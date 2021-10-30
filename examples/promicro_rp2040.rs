#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

use panic_halt as _;

use embedded_time::rate::Extensions as _;

use embassy::{executor::Executor, util::Forever};

use rp2040_hal::{
    gpio::{self, bank0, FunctionI2C, Pin},
    i2c::I2C,
    pac,
    prelude::_rphal_clocks_Clock,
    sio::Sio,
    watchdog::Watchdog,
};
use sh1107_async::Sh1107;

const ADDRESS: embassy_traits::i2c::SevenBitAddress = 0x3C;

const GLYPHS: &'static [u8; 1024] = {
    let g = include_bytes!("glyphs.bmp");
    unsafe { &*(g.as_ptr().offset(130) as *const [u8; 1024]) }
};

#[used]
#[link_section = ".boot2"]
static BOOT2: [u8; 256] = rp2040_boot2::BOOT_LOADER_W25Q080;
static EXECUTOR: Forever<Executor> = Forever::new();

#[embassy::task]
async fn demo(
    i2c_bus: I2C<
        pac::I2C0,
        (
            Pin<bank0::Gpio16, FunctionI2C>,
            Pin<bank0::Gpio17, FunctionI2C>,
        ),
    >,
) {
    let mut sh1107: Sh1107<_, 128, 64, ADDRESS> = Sh1107::new(i2c_bus);

    use sh1107_async::Command::*;
    use sh1107_async::Direction;
    use sh1107_async::DisplayState;
    sh1107
        .run([
            SetDisplay(false),
            SetClkDividerOscFrequency(0x41), // divide clk / 2, fosc - 5%
            SetMultiplexRatio(127),
            SetDisplayOffset(96),
            SetStartLine(0),
            SetSegmentReMap(false),
            SetCOMScanDirection(Direction::Normal),
            SetContrastControl(110), // 110 / 256
            SetChargePeriods(0x22),  // precharge 2 DCLK, discharge 2DCLK
            SetVCOMHDeselectLevel(0x35),
            SetEntireDisplay(DisplayState::On),
            // power up VDD
            SetDisplay(true),
        ])
        .await
        .ok()
        .expect("Failed to run init command chain");

    // draw some stuff to the frame buffer with embedded_graphics
    // flush to the target
}

#[cortex_m_rt::entry]
fn main() -> ! {
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

    let pins = gpio::Pins::new(
        pac.IO_BANK0,
        pac.PADS_BANK0,
        sio.gpio_bank0,
        &mut pac.RESETS,
    );

    let i2c_ctrl = I2C::new_controller(
        pac.I2C0,
        pins.gpio16.into_mode(),
        pins.gpio17.into_mode(),
        400_000.Hz(),
        &mut pac.RESETS,
        clocks.system_clock.freq(),
    );

    let executor = EXECUTOR.put(Executor::new());
    executor.run(|spawner| {
        spawner
            .spawn(demo(i2c_ctrl))
            .expect("Failed to setup i2c task");
    });
}
