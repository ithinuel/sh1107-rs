#![no_std]
#![feature(type_alias_impl_trait)]
#![feature(generic_associated_types)]

use core::{
    ops::{Deref, DerefMut},
    task::Poll,
};

use embedded_hal_async::i2c::ErrorType;
use fugit::{ExtU32, RateExtU32};
use futures::{future, Future};
use panic_probe as _;
use pimoroni_pico_explorer::{all_pins::Pins, hal};

use hal::{
    gpio::{bank0, FunctionI2C, Pin},
    pac::{self},
    sio::Sio,
    watchdog::Watchdog,
    Clock,
};

use rp2040_async_i2c::i2c::I2C;

pub use embedded_hal_async::i2c::SevenBitAddress;
pub use hal::timer::Timer;
pub use pimoroni_pico_explorer::entry;

type I2CPeriphInner = I2C<
    pac::I2C0,
    (
        Pin<bank0::Gpio20, FunctionI2C>,
        Pin<bank0::Gpio21, FunctionI2C>,
    ),
>;

pub struct I2CPeriph(I2CPeriphInner);
sh1107::impl_write_iter!(I2CPeriph => I2CPeriphInner: write_iter);

pub async fn wait_for(timer: &Timer, delay: u32) {
    let target = timer.get_counter() + delay.micros();
    future::poll_fn(|cx| {
        if timer.get_counter() < target {
            cx.waker().wake_by_ref();
            Poll::Pending
        } else {
            Poll::Ready(())
        }
    })
    .await;
}

pub async fn timed<T>(_op: &str, _timer: &Timer, fut: impl Future<Output = T>) -> T {
    fut.await
}

pub fn init() -> (Timer, I2CPeriph) {
    let mut pac = pac::Peripherals::take().unwrap();
    let _core = pac::CorePeripherals::take().unwrap();

    let mut watchdog = Watchdog::new(pac.WATCHDOG);
    let clocks = hal::clocks::init_clocks_and_plls(
        pimoroni_pico_explorer::XOSC_CRYSTAL_FREQ,
        pac.XOSC,
        pac.CLOCKS,
        pac.PLL_SYS,
        pac.PLL_USB,
        &mut pac.RESETS,
        &mut watchdog,
    )
    .ok()
    .unwrap();

    let timer = Timer::new(pac.TIMER, &mut pac.RESETS);

    let sio = Sio::new(pac.SIO);
    let pins = Pins::new(
        pac.IO_BANK0,
        pac.PADS_BANK0,
        sio.gpio_bank0,
        &mut pac.RESETS,
    );

    let i2c_ctrl = I2C::new(
        pac.I2C0,
        pins.i2c_sda.into_mode(),
        pins.i2c_scl.into_mode(),
        400_000.Hz(),
        &mut pac.RESETS,
        clocks.system_clock.freq(),
    );

    (timer, I2CPeriph(i2c_ctrl))
}
