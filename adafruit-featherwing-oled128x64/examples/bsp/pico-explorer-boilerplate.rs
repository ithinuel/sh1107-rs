use fugit::RateExtU32;
use panic_probe as _;
use pimoroni_pico_explorer::{all_pins::Pins, hal};

use hal::{
    gpio::{bank0, FunctionI2C, Pin, PullUp},
    i2c::I2C,
    pac::{self, interrupt},
    sio::Sio,
    watchdog::Watchdog,
    Clock,
};

pub use embedded_hal_async::i2c::SevenBitAddress;
pub use pimoroni_pico_explorer::entry;

pub type I2CPeriph = I2C<
    pac::I2C0,
    (
        Pin<bank0::Gpio20, FunctionI2C, PullUp>,
        Pin<bank0::Gpio21, FunctionI2C, PullUp>,
    ),
>;

mod timer;
pub use timer::*;

#[interrupt]
#[allow(non_snake_case)]
fn I2C0_IRQ() {
    use hal::async_utils::AsyncPeripheral;
    I2CPeriph::on_interrupt();
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

    let mut timer = Timer::new(pac.TIMER, &mut pac.RESETS, &clocks);
    let alarm = timer.alarm_0().unwrap();

    let sio = Sio::new(pac.SIO);
    let pins = Pins::new(
        pac.IO_BANK0,
        pac.PADS_BANK0,
        sio.gpio_bank0,
        &mut pac.RESETS,
    );

    let i2c_ctrl = I2C::new_controller(
        pac.I2C0,
        pins.i2c_sda.reconfigure(),
        pins.i2c_scl.reconfigure(),
        400_000.Hz(),
        &mut pac.RESETS,
        clocks.system_clock.freq(),
    );

    critical_section::with(move |cs| unsafe {
        pac::NVIC::unpend(pac::Interrupt::I2C0_IRQ);
        pac::NVIC::unmask(pac::Interrupt::I2C0_IRQ);
        pac::NVIC::unpend(pac::Interrupt::TIMER_IRQ_0);
        pac::NVIC::unmask(pac::Interrupt::TIMER_IRQ_0);
        *ALARM0_WAKER.borrow_ref_mut(cs) = Some((alarm, None));
    });

    (timer, i2c_ctrl)
}
