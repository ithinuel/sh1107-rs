#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

use panic_rtt_target as _;

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

use adafruit_featherwing_oled128x64::{Display, DisplayState};
const ADDRESS: embassy_traits::i2c::SevenBitAddress = 0x3C;

const GLYPHS: [u8; 1024] = {
    let bmp = include_bytes!("glyphs.bmp");

    // Eliminate bmp header
    // Transpose & flip image

    let mut g = [0u8; 1024];
    let mut page = 0;
    while page < 16 {
        let mut col = 0;
        while col < 64 {
            g[page * 64 + col] = bmp[130 + (63 - col) * 16 + (15 - page)];
            col += 1;
        }
        page += 1;
    }
    g
};

#[used]
#[link_section = ".boot2"]
static BOOT2: [u8; 256] = rp2040_boot2::BOOT_LOADER_W25Q080;
static EXECUTOR: Forever<Executor> = Forever::new();
type I2CPeriph = I2C<
    pac::I2C0,
    (
        Pin<bank0::Gpio16, FunctionI2C>,
        Pin<bank0::Gpio17, FunctionI2C>,
    ),
>;

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

async fn wait_for(timer: &rp2040_hal::timer::Timer, delay: u32) {
    let start = timer.get_counter_low();
    while timer.get_counter_low().wrapping_sub(start) < delay {
        _yield().await
    }
}

#[embassy::task]
async fn demo(timer: rp2040_hal::timer::Timer, i2c_bus: I2CPeriph) {
    wait_for(&timer, 1_000_000).await;
    //wait_for(&timer, 250_000).await;

    let mut display: Display<_, ADDRESS> = Display::new(i2c_bus)
        .await
        .expect("Failed to initialize display");

    wait_for(&timer, 100_000).await;

    let start = timer.get_counter_low();
    display
        .write_frame_by_page(GLYPHS.iter().cloned())
        .await
        .expect("Woops you failed");
    let diff = timer.get_counter_low().wrapping_sub(start);
    rtt_target::rprintln!("Full frame took: {}us", diff);

    display.set_state(DisplayState::On).await.expect("Woops");

    loop {
        // Scrolling screen
        //sh1107
        //    .run([Command::SetStartLine(n)])
        //    //.run([Command::SetDisplayOffset(n.wrapping_add(96))])
        //    .await
        //    .expect("woops");
        //wait_for(&timer, 5_000).await;

        // Clear screen
        //write_frame_by_column(&mut sh1107, core::iter::repeat(0).take(128 * 128))
        //    .await
        //    .expect("failed");

        //sh1107
        //    .run([
        //        // power up VDD
        //        DisplayOnOff(State::On),
        //    ])
        //    .await
        //    .expect("failed");
        //read_frame(&mut sh1107, &mut rx).await.expect("failed");
        //rtt_target::rprintln!("{:x?}", &rx);

        //write_frame(&mut sh1107).await.expect("failed");

        //sh1107.run([DisplayOnOff(State::Off)]).await;
        //read_frame(&mut sh1107, &mut rx).await.expect("failed");
        //rtt_target::rprintln!("{:x?}", &rx);

        //write_frame_by_column(&mut sh1107, core::iter::repeat(0).take(128 * 16)).await;
    }

    //sh1107.run([DisplayOnOff(State::On)]).await.expect("failed");
    //wait_for(&timer, 2_000_000).await;
    //sh1107
    //    .run([DisplayOnOff(State::Off)])
    //    .await
    //    .expect("failed");
}

#[cortex_m_rt::entry]
fn main() -> ! {
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
            .spawn(demo(timer, i2c_ctrl))
            .expect("Failed to setup i2c task");
    });
}
