#![no_std]
#![feature(type_alias_impl_trait)]
#![feature(generic_associated_types)]

use embedded_hal_async::i2c::ErrorType;
use embedded_hal_async::i2c::I2c;
use futures::Future;
use panic_rtt_target as _;

use embedded_time::rate::Extensions as _;
use rp2040_async_i2c::AsyncI2C;
use rp2040_hal::{
    gpio::{self, bank0, FunctionI2C, Pin},
    pac,
    prelude::_rphal_clocks_Clock,
    sio::Sio,
    watchdog::Watchdog,
};

pub use embedded_hal_async::i2c::SevenBitAddress;
pub use rp2040_hal::timer::Timer;

#[used]
#[link_section = ".boot2"]
static BOOT2: [u8; 256] = rp2040_boot2::BOOT_LOADER_W25Q080;
type I2CPeriphInner = AsyncI2C<
    pac::I2C0,
    (
        Pin<bank0::Gpio16, FunctionI2C>,
        Pin<bank0::Gpio17, FunctionI2C>,
    ),
>;

pub struct I2CPeriph(I2CPeriphInner);

impl embedded_hal_async::i2c::ErrorType for I2CPeriph {
    type Error = <I2CPeriphInner as ErrorType>::Error;
}
impl I2c<SevenBitAddress> for I2CPeriph {
    type ReadFuture<'a> = <I2CPeriphInner as I2c<SevenBitAddress>>::ReadFuture<'a>
    where
        Self: 'a;

    fn read<'a>(
        &'a mut self,
        address: SevenBitAddress,
        read: &'a mut [u8],
    ) -> Self::ReadFuture<'a> {
        self.0.read(address, read)
    }

    type WriteFuture<'a> = <I2CPeriphInner as I2c<SevenBitAddress>>::WriteFuture<'a>
    where
        Self: 'a;

    fn write<'a>(&'a mut self, address: SevenBitAddress, write: &'a [u8]) -> Self::WriteFuture<'a> {
        self.0.write(address, write)
    }

    type WriteReadFuture<'a> = <I2CPeriphInner as I2c<SevenBitAddress>>::WriteReadFuture<'a>
    where
        Self: 'a;

    fn write_read<'a>(
        &'a mut self,
        address: SevenBitAddress,
        write: &'a [u8],
        read: &'a mut [u8],
    ) -> Self::WriteReadFuture<'a> {
        self.0.write_read(address, write, read)
    }

    type TransactionFuture<'a, 'b> = <I2CPeriphInner as I2c<SevenBitAddress>>::TransactionFuture<'a, 'b>
    where
        Self: 'a,
        'b: 'a;

    fn transaction<'a, 'b>(
        &'a mut self,
        address: SevenBitAddress,
        operations: &'a mut [embedded_hal_async::i2c::Operation<'b>],
    ) -> Self::TransactionFuture<'a, 'b> {
        self.0.transaction(address, operations)
    }
}
impl sh1107::WriteIter<embedded_hal_async::i2c::SevenBitAddress> for I2CPeriph {
    type WriteIterFuture<'a, U>
    = impl Future<Output = Result<(), Self::Error>> + 'a
    where
        Self: 'a,
        U: 'a;

    fn write_iter<'a, U>(
        &'a mut self,
        address: SevenBitAddress,
        bytes: U,
    ) -> Self::WriteIterFuture<'a, U>
    where
        U: IntoIterator<Item = u8> + 'a,
    {
        self.0.write_iter(address, bytes)
    }
}

async fn _yield() {
    let mut once = true;
    futures::future::poll_fn(move |cx| {
        cx.waker().wake_by_ref();
        if once {
            once = false;
            core::task::Poll::Pending
        } else {
            core::task::Poll::Ready(())
        }
    })
    .await
}

pub async fn wait_for(timer: &rp2040_hal::timer::Timer, delay: u32) {
    let start = timer.get_counter_low();
    while timer.get_counter_low().wrapping_sub(start) < delay {
        _yield().await
    }
}

pub fn init() -> (rp2040_hal::timer::Timer, I2CPeriph) {
    rtt_target::rtt_init_print!();

    let mut pac = pac::Peripherals::take().unwrap();
    let _core = pac::CorePeripherals::take().unwrap();
    let mut watchdog = Watchdog::new(pac.WATCHDOG);
    let sio = Sio::new(pac.SIO);

    // External high-speed crystal on the pico board is 12Mhz
    let external_xtal_freq_hz = 12_000_000u32;
    let clocks = rp2040_hal::clocks::init_clocks_and_plls(
        external_xtal_freq_hz,
        pac.XOSC,
        pac.CLOCKS,
        pac.PLL_SYS,
        pac.PLL_USB,
        &mut pac.RESETS,
        &mut watchdog,
    )
    .ok()
    .unwrap();

    let timer = rp2040_hal::timer::Timer::new(pac.TIMER, &mut pac.RESETS);

    let pins = gpio::Pins::new(
        pac.IO_BANK0,
        pac.PADS_BANK0,
        sio.gpio_bank0,
        &mut pac.RESETS,
    );

    let i2c_ctrl = AsyncI2C::new(
        pac.I2C0,
        pins.gpio16.into_mode(),
        pins.gpio17.into_mode(),
        400_000.Hz(),
        &mut pac.RESETS,
        clocks.system_clock.freq(),
    );

    (timer, I2CPeriph(i2c_ctrl))
}
