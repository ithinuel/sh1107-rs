use core::cell::RefCell;

use critical_section::Mutex;
use fugit::RateExtU32;
use panic_probe as _;
use rp2040_async_i2c::I2C;
use sparkfun_pro_micro_rp2040::{hal, Pins};

use hal::{
    gpio::{bank0, FunctionI2C, FunctionNull, Pin, PullUp},
    pac::{self, interrupt, PIO0},
    pio::{PIOExt, SM0},
    sio::Sio,
    watchdog::Watchdog,
    Clock,
};

pub use embedded_hal_async::i2c::SevenBitAddress;
pub use sparkfun_pro_micro_rp2040::entry;

mod timer;
pub use timer::*;

pub type I2CPeriph = I2C<
    'static,
    PIO0,
    SM0,
    Pin<bank0::Gpio16, FunctionNull, PullUp>,
    Pin<bank0::Gpio17, FunctionNull, PullUp>,
>;

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
        pio.sm_irq(0).irq_inte().modify(|_, w| {
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
        sparkfun_pro_micro_rp2040::XOSC_CRYSTAL_FREQ,
        pac.XOSC,
        pac.CLOCKS,
        pac.PLL_SYS,
        pac.PLL_USB,
        &mut pac.RESETS,
        &mut watchdog,
    )
    .ok()
    .unwrap();

    let mut timer = Timer::new(pac.TIMER, &mut pac.RESETS, &clocks);
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
        pins.sda.into_pull_up_disabled(),
        pins.scl.into_pull_up_disabled(),
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

    (timer, i2c_ctrl)
}
