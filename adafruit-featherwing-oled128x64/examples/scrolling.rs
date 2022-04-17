#![no_std]
#![no_main]

use adafruit_featherwing_oled128x64::{Display, DisplayState};
use promicro_rp2040_boilerplate as bsp;

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

async fn demo(timer: bsp::Timer, i2c_bus: bsp::I2CPeriph) {
    use bsp::wait_for;

    wait_for(&timer, 100_000).await;

    let mut display: Display<_, ADDRESS> = Display::new(i2c_bus)
        .await
        .ok()
        .expect("Failed to initialized display");

    wait_for(&timer, 200_000).await;
    while display.is_busy().await.unwrap_or(true) {}

    display
        .write_frame_by_page(GLYPHS.iter().cloned())
        .await
        .ok()
        .expect("failed to write frame data");

    let mut n = 0u8;

    loop {
        n = n.wrapping_add(1);
        n %= 8;
        wait_for(&timer, 200_000).await;

        // Scrolling screen
        display
            .set_line_and_offset(n, 0)
            .await
            .expect("Failed to set line and offset");
        //sh1107
        //    .run([Command::SetStartLine(n)])
        //    //.run([Command::SetDisplayOffset(n.wrapping_add(96))])
        //    .await
        //    .expect("woops");
        wait_for(&timer, 5_000).await;

        // Clear screen
        display
            .write_frame_by_column(core::iter::repeat(0).take(128 * 128))
            .await
            .expect("failed to write data by column");

        display
            .set_state(DisplayState::On)
            .await
            .expect("Failed to turn the display On");

        //write_frame(&mut sh1107).await.expect("failed");

        display
            .set_state(DisplayState::Off)
            .await
            .expect("Failed to turn the display Off");

        display
            .write_frame_by_column(core::iter::repeat(0).take(128 * 16))
            .await
            .expect("Failed to partially clear");
    }
}

#[cortex_m_rt::entry]
fn main() -> ! {
    let (timer, i2c) = bsp::init();

    let runtime = nostd_async::Runtime::new();
    let mut task = nostd_async::Task::new(demo(timer, i2c));
    let handle = task.spawn(&runtime);
    handle.join();
    unreachable!()
}
