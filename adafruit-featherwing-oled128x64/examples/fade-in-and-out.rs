#![no_std]
#![no_main]

mod bsp;

use adafruit_featherwing_oled128x64::{Destination, Display, DisplayState};
use defmt_rtt as _;

const ADDRESS: bsp::SevenBitAddress = 0x3C;

const GLYPHS: [u8; 1024] = {
    let bmp = include_bytes!("../../assets/glyphs.bmp");

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

async fn demo(
    timer: &bsp::Timer,
    i2c_bus: bsp::I2CPeriph,
) -> Result<(), <bsp::I2CPeriph as embedded_hal::i2c::ErrorType>::Error> {
    use bsp::{timed, wait_for};

    wait_for(timer, 1_000_000).await;

    let mut display: Display<_, ADDRESS> = timed("Init", timer, async {
        Display::new(i2c_bus).await.map_err(|(_, e)| e)
    })
    .await?;

    timed("Write Frame", timer, async {
        display.set_start_line(0).await?;
        display
            .write_frame_by_page(Destination::Frame1, GLYPHS.into_iter())
            .await?;
        display.set_contrast(0).await?;
        display.set_state(DisplayState::On).await
    })
    .await?;

    for c in core::iter::repeat((0..=255).chain((1..=254).rev())).flatten() {
        display.set_contrast(c).await?;

        // brightest stays on the shortest: 5ms
        // darkest stays off the longest: 40ms

        wait_for(timer, 1_000 + ((14_000 * u32::from(255 - c)) / 255)).await;
    }
    Ok(())
}

#[bsp::entry]
fn main() -> ! {
    let (timer, i2c) = bsp::init();

    let runtime = nostd_async::Runtime::new();
    let mut task = nostd_async::Task::new(demo(&timer, i2c));
    let handle = task.spawn(&runtime);
    handle.join().expect("Something went wrong");
    unreachable!()
}
