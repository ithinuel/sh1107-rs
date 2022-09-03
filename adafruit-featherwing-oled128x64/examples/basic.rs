#![no_std]
#![no_main]

use adafruit_featherwing_oled128x64::Display;
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

async fn demo(timer: bsp::Timer, mut i2c_bus: bsp::I2CPeriph) {
    use bsp::wait_for;

    loop {
        wait_for(&timer, 500_000).await;

        let start = timer.get_counter_low();
        let mut display: Display<_, ADDRESS> = Display::new(i2c_bus)
            .await
            .ok()
            .expect("Failed to initialized display");
        let diff = timer.get_counter_low().wrapping_sub(start);
        rtt_target::rprintln!("Init took {}us", diff);

        let start = timer.get_counter_low();
        wait_for(&timer, 250_000).await;
        while display.is_busy().await.unwrap_or(true) {}
        let diff = timer.get_counter_low().wrapping_sub(start);
        rtt_target::rprintln!("Init took {}us", diff);

        let start = timer.get_counter_low();
        display
            .write_frame_by_page(GLYPHS.iter().cloned())
            .await
            .ok()
            .expect("failed to write frame data");
        let diff = timer.get_counter_low().wrapping_sub(start);
        rtt_target::rprintln!("Init took {}us", diff);

        i2c_bus = display.release();
        wait_for(&timer, 250_000).await;
    }

    //loop {
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
    //}

    //sh1107.run([DisplayOnOff(State::On)]).await.expect("failed");
    //wait_for(&timer, 2_000_000).await;
    //sh1107
    //    .run([DisplayOnOff(State::Off)])
    //    .await
    //    .expect("failed");
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
