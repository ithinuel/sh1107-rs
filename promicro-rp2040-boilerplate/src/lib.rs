#![no_std]
#![feature(type_alias_impl_trait)]
#![feature(generic_associated_types)]

use core::cell::RefCell;
use core::ops::Deref;
use core::ops::DerefMut;
use core::task::Poll;
use core::task::Waker;

use critical_section::Mutex;
use embedded_hal_async::i2c::ErrorType;
use fugit::MicrosDurationU32;
use fugit::RateExtU32;
use futures::Future;
use panic_probe as _;
use rp2040_async_i2c::AsyncI2C;

use sparkfun_pro_micro_rp2040::hal::pac::interrupt;
use sparkfun_pro_micro_rp2040::hal::timer::Alarm;
use sparkfun_pro_micro_rp2040::hal::timer::Alarm0;
use sparkfun_pro_micro_rp2040::hal::{
    self,
    gpio::{self, bank0, FunctionI2C, Pin},
    pac,
    prelude::_rphal_clocks_Clock,
    sio::Sio,
    watchdog::Watchdog,
};

pub use embedded_hal_async::i2c::SevenBitAddress;
pub use sparkfun_pro_micro_rp2040::entry;
pub use sparkfun_pro_micro_rp2040::hal::timer::Timer;

type I2CPeriphInner = AsyncI2C<
    pac::I2C0,
    (
        Pin<bank0::Gpio16, FunctionI2C>,
        Pin<bank0::Gpio17, FunctionI2C>,
    ),
>;
pub struct I2CPeriph(I2CPeriphInner);
impl Deref for I2CPeriph {
    type Target = I2CPeriphInner;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl DerefMut for I2CPeriph {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
impl embedded_hal_async::i2c::ErrorType for I2CPeriph {
    type Error = <I2CPeriphInner as ErrorType>::Error;
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

type WakerCTX = (Alarm0, Option<Waker>);
static ALARM0_WAKER: Mutex<RefCell<Option<WakerCTX>>> = Mutex::new(RefCell::new(None));
pub async fn wait_for(timer: &mut Timer, delay: u32) {
    if delay < 20 {
        let start = timer.get_counter_low();
        futures::future::poll_fn(|cx| {
            cx.waker().wake_by_ref();
            if timer.get_counter_low().wrapping_sub(start) < delay {
                Poll::Pending
            } else {
                Poll::Ready(())
            }
        })
        .await;
    } else {
        let mut started = false;
        futures::future::poll_fn(move |cx| {
            critical_section::with(|cs| {
                if let Some((alarm, waker)) = ALARM0_WAKER.borrow_ref_mut(cs).as_mut() {
                    if !started {
                        alarm.clear_interrupt();
                        alarm.enable_interrupt();
                        alarm.schedule(MicrosDurationU32::micros(delay)).unwrap();
                        started = true;
                        *waker = Some(cx.waker().clone());
                        Poll::Pending
                    } else if alarm.finished() {
                        Poll::Ready(())
                    } else {
                        *waker = Some(cx.waker().clone());
                        Poll::Pending
                    }
                } else {
                    unreachable!()
                }
            })
        })
        .await;
    }
}

#[interrupt]
#[allow(non_snake_case)]
fn TIMER_IRQ_0() {
    critical_section::with(|cs| {
        ALARM0_WAKER
            .borrow_ref_mut(cs)
            .as_mut()
            .and_then(|(alarm, waker)| {
                alarm.disable_interrupt();
                waker.take()
            })
            .map(|waker| waker.wake())
    });
}

static WAKER: Mutex<RefCell<Option<Waker>>> = Mutex::new(RefCell::new(None));
pub fn waker_setter(waker: Waker) {
    critical_section::with(|cs| {
        *WAKER.borrow_ref_mut(cs) = Some(waker);
    });
}

#[interrupt]
#[allow(non_snake_case)]
fn I2C0_IRQ() {
    critical_section::with(|cs| {
        let i2c0 = unsafe { &pac::Peripherals::steal().I2C0 };
        let stat = i2c0.ic_intr_stat.read();
        if stat.r_tx_abrt().bit() {
            defmt::trace!("i2c0: {:x}", stat.bits());
            i2c0.ic_intr_mask.modify(|_, w| w.m_tx_abrt().enabled());
        }
        i2c0.ic_intr_mask
            .modify(|_, w| w.m_tx_empty().enabled().m_rx_full().enabled());
        WAKER.borrow_ref_mut(cs).take().map(|waker| waker.wake())
    });
}

pub fn init() -> (Timer, I2CPeriph) {
    let mut pac = pac::Peripherals::take().unwrap();
    let _core = pac::CorePeripherals::take().unwrap();
    let mut watchdog = Watchdog::new(pac.WATCHDOG);
    let sio = Sio::new(pac.SIO);

    // External high-speed crystal on the pico board is 12Mhz
    let external_xtal_freq_hz = 12_000_000u32;
    let clocks = hal::clocks::init_clocks_and_plls(
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

    let mut timer = Timer::new(pac.TIMER, &mut pac.RESETS);
    let alarm = timer.alarm_0().unwrap();

    let pins = gpio::Pins::new(
        pac.IO_BANK0,
        pac.PADS_BANK0,
        sio.gpio_bank0,
        &mut pac.RESETS,
    );

    let mut i2c_ctrl = AsyncI2C::new(
        pac.I2C0,
        pins.gpio16.into_mode(),
        pins.gpio17.into_mode(),
        400_000.Hz(),
        &mut pac.RESETS,
        clocks.system_clock.freq(),
    );
    i2c_ctrl.set_waker_setter(waker_setter);

    critical_section::with(move |cs| unsafe {
        pac::NVIC::unpend(pac::Interrupt::I2C0_IRQ);
        pac::NVIC::unmask(pac::Interrupt::I2C0_IRQ);
        pac::NVIC::unpend(pac::Interrupt::TIMER_IRQ_0);
        pac::NVIC::unmask(pac::Interrupt::TIMER_IRQ_0);
        *ALARM0_WAKER.borrow_ref_mut(cs) = Some((alarm, None));
    });

    (timer, I2CPeriph(i2c_ctrl))
}
