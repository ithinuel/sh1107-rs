#![no_std]
#![allow(incomplete_features)]
#![feature(async_fn_in_trait)]

use core::{
    cell::RefCell,
    ops::{Deref, DerefMut},
    task::{Poll, Waker},
};

use critical_section::Mutex;
use embedded_hal_async::i2c::ErrorType;
use fugit::{MicrosDurationU32, RateExtU32};
use futures::{future, Future};
use panic_probe as _;
use pimoroni_pico_explorer::{
    all_pins::Pins,
    hal::{self, pio::SM0, prelude::_rphal_pio_PIOExt},
    pac::PIO0,
};

use hal::{
    gpio::bank0,
    pac::{self, interrupt},
    sio::Sio,
    timer::{Alarm, Alarm0},
    watchdog::Watchdog,
    Clock,
};

use rp2040_async_i2c::pio::I2C;

pub use embedded_hal_async::i2c::SevenBitAddress;
pub use hal::timer::Timer;
pub use pimoroni_pico_explorer::entry;

type I2CPeriphInner = I2C<'static, PIO0, SM0, bank0::Gpio20, bank0::Gpio21>;

pub struct I2CPeriph(I2CPeriphInner);
sh1107::impl_write_iter!(I2CPeriph => I2CPeriphInner: write_iter);

type Alarm0WakerCTX = (Alarm0, Option<Waker>);
static ALARM0_WAKER: Mutex<RefCell<Option<Alarm0WakerCTX>>> = Mutex::new(RefCell::new(None));
pub async fn wait_for(timer: &Timer, delay: u32) {
    if delay < 20 {
        let start = timer.get_counter_low();
        future::poll_fn(|cx| {
            if timer.get_counter_low().wrapping_sub(start) < delay {
                cx.waker().wake_by_ref();
                Poll::Pending
            } else {
                Poll::Ready(())
            }
        })
        .await;
    } else {
        let mut started = false;
        future::poll_fn(move |cx| {
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

pub async fn timed<T>(op: &str, timer: &Timer, fut: impl Future<Output = T>) -> T {
    let start = timer.get_counter_low();
    let res = fut.await;
    let diff = timer.get_counter_low().wrapping_sub(start);
    defmt::info!("{} took {}us", op, diff);
    res
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

static mut PIO: Option<hal::pio::PIO<PIO0>> = None;
static PIO_WAKER: Mutex<RefCell<Option<core::task::Waker>>> = Mutex::new(RefCell::new(None));
fn waker_setter(waker: core::task::Waker) {
    critical_section::with(|cs| {
        PIO_WAKER.borrow_ref_mut(cs).replace(waker);
    });
}
#[interrupt]
#[allow(non_snake_case)]
fn PIO0_IRQ_0() {
    critical_section::with(|cs| {
        let pio = unsafe { &*pac::PIO0::ptr() };
        pio.sm_irq[0].irq_inte.modify(|_, w| {
            w.sm0()
                .clear_bit()
                .sm0_txnfull()
                .clear_bit()
                .sm0_rxnempty()
                .clear_bit()
        });
        if let Some(waker) = PIO_WAKER.borrow_ref_mut(cs).take() {
            waker.wake();
        }
    });
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

    let mut timer = Timer::new(pac.TIMER, &mut pac.RESETS);
    let alarm = timer.alarm_0().unwrap();

    let sio = Sio::new(pac.SIO);
    let pins = Pins::new(
        pac.IO_BANK0,
        pac.PADS_BANK0,
        sio.gpio_bank0,
        &mut pac.RESETS,
    );

    let (pio0, pio0sm0, ..) = pac.PIO0.split(&mut pac.RESETS);
    unsafe { PIO = Some(pio0) };

    let mut i2c_ctrl = I2C::new(
        unsafe { PIO.as_mut().unwrap() },
        pins.i2c_sda.into_pull_up_disabled(),
        pins.i2c_scl.into_pull_up_disabled(),
        pio0sm0,
        400_000.Hz(),
        clocks.system_clock.freq(),
    );
    i2c_ctrl.set_waker_setter(waker_setter);

    critical_section::with(move |cs| unsafe {
        pac::NVIC::unpend(pac::Interrupt::TIMER_IRQ_0);
        pac::NVIC::unmask(pac::Interrupt::TIMER_IRQ_0);
        pac::NVIC::unpend(pac::Interrupt::PIO0_IRQ_0);
        pac::NVIC::unmask(pac::Interrupt::PIO0_IRQ_0);
        *ALARM0_WAKER.borrow_ref_mut(cs) = Some((alarm, None));
    });

    (timer, I2CPeriph(i2c_ctrl))
}
