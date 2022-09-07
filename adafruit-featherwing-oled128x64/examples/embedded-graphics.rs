#![no_std]
#![no_main]


use core::convert::Infallible;

use embedded_graphics::{
    mono_font::{ascii::FONT_4X6, MonoTextStyle},
    pixelcolor::BinaryColor,
    prelude::Point,
    primitives::{Circle, Line, Primitive, PrimitiveStyle},
    text::Text,
    Drawable,
};

cfg_if::cfg_if! {
    if #[cfg(feature = "pico-explorer")] {
        use pico_explorer_boilerplate as bsp;
    } else if #[cfg(feature = "pico-explorer-pio")] {
        use pico_explorer_pio_boilerplate as bsp;
    } else if #[cfg(feature = "promicro")] {
        use promicro_rp2040_boilerplate as bsp;
    } else if #[cfg(feature = "rpi-pico")] {
        use rpi_pico_boilerplate as bsp;
    } else {
        compile_error!("One platform feature must be selected");
    }
}

use adafruit_featherwing_oled128x64::{BufferedDisplay, DisplayState, PAGE};
use defmt_rtt as _;

const ADDRESS: bsp::SevenBitAddress = 0x3C;

async fn demo(
    timer: &bsp::Timer,
    i2c_bus: bsp::I2CPeriph,
) -> Result<(), <bsp::I2CPeriph as embedded_hal::i2c::ErrorType>::Error> {
    use bsp::{timed, wait_for};

    wait_for(timer, 1_000_000).await;

    let mut display: BufferedDisplay<_, ADDRESS> =
        timed("Init", timer, async { BufferedDisplay::new(i2c_bus).await })
            .await
            .map_err(|(_, e)| e)?;

    timed("Turn on Display", timer, async {
        display.set_contrast(0).await?;
        display.wait_while_busy().await?;
        display.set_state(DisplayState::On).await
    })
    .await?;

    //timed("Flush to display", timer, display.flush()).await?;
    wait_for(timer, 2_000_000).await;

    Line::new(Point::new(4, 4), Point::new(19, 4))
        .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 4))
        .draw(&mut display)
        .unwrap();

    // Create a new character style
    let txt_style = MonoTextStyle::new(&FONT_4X6, BinaryColor::On);
    let line_style = PrimitiveStyle::with_stroke(BinaryColor::On, 1);

    // Drawing to this display is Infallible
    let _: Result<(), Infallible> = timed("Drawing in buffer", timer, async {
        for n in 1..i32::from(PAGE) {
            use core::fmt::Write;
            // may contain up to 2 base 10 digits.
            let mut string: arrayvec::ArrayString<2> = Default::default();
            write!(&mut string, "{}", n).unwrap_or_else(|_| unreachable!());

            Text::new(&string, Point::new(10, (n + 1) * 8 - 2), txt_style).draw(&mut display)?;
            Line::new(Point::new(4, n * 8), Point::new(19, n * 8))
                .into_styled(line_style)
                .draw(&mut display)?;
            Line::new(Point::new(44, n * 8), Point::new(59, n * 8))
                .into_styled(line_style)
                .draw(&mut display)?;
            Text::new(&string, Point::new(48, (n + 1) * 8 - 2), txt_style).draw(&mut display)?;
        }
        Ok(())
    })
    .await;

    timed("Flush to display", timer, display.flush()).await?;

    let range = 26..(128 - 26); // boundary of the movement
    let range = range.clone().chain(range.rev()); // going back and forth
    for n in core::iter::repeat(range).flatten() {
        let shape = Circle::with_center(Point::new(31, n), 20);
        let mut shape = shape.into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1));

        let _ = shape.draw(&mut display);
        display.flush().await?;

        // clear the circle
        shape.style.stroke_color = Some(BinaryColor::Off);
        let _ = shape.draw(&mut display);
    }
    Ok(())
}

#[bsp::entry]
fn main() -> ! {
    let (timer, i2c) = bsp::init();

    let runtime = nostd_async::Runtime::new();
    let mut task = nostd_async::Task::new(demo(&timer, i2c));
    let handle = task.spawn(&runtime);
    handle.join().expect("Some error occured");
    unreachable!()
}
