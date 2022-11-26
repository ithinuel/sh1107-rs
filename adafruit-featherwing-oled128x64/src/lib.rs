#![no_std]
#![allow(incomplete_features)]
#![feature(async_fn_in_trait)]

use core::ops::Deref;
use core::ops::DerefMut;

use embedded_hal_async::i2c::ErrorType;
use embedded_hal_async::i2c::I2c;
use embedded_hal_async::i2c::SevenBitAddress;
use sh1107::Direction;
use sh1107::{AddressMode, Sh1107};
use sh1107::{Command, DisplayMode};

pub use sh1107::DisplayState;

pub const COLUMN: u8 = 64;
pub const ROW: u8 = 128;
pub const PAGE: u8 = ROW / 8;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
#[cfg_attr(feature = "defmt", defmt::Format)]
pub enum Destination {
    Frame1,
    Frame2,
}

pub trait ValidBus:
    sh1107::WriteIter<SevenBitAddress, Error = <<Self as ValidBus>::I2c as ErrorType>::Error>
    + Deref<Target = Self::I2c>
    + DerefMut
{
    type I2c: I2c<SevenBitAddress>;
}
impl<T, U> ValidBus for T
where
    T: sh1107::WriteIter<SevenBitAddress, Error = U::Error> + Deref<Target = U> + DerefMut,
    U: I2c<SevenBitAddress>,
{
    type I2c = U;
}

pub struct Display<T, const ADDRESS: SevenBitAddress>(Sh1107<T, ADDRESS>);

impl<T, const ADDRESS: SevenBitAddress> Display<T, ADDRESS>
where
    T: ValidBus,
{
    pub async fn new(i2c_bus: T) -> Result<Self, (T, T::Error)> {
        let mut sh1107 = Sh1107::new(i2c_bus);

        use Command::*;
        let init_sequence = [
            DisplayOnOff(DisplayState::Off),
            SetClkDividerOscFrequency {
                divider: 2,        // divide by 2
                osc_freq_ratio: 0, // +0%
            },
            SetMultiplexRatio(COLUMN),
            // rendering alignment
            SetDisplayOffset(96),
            SetStartLine(0),
            // display orientation
            SetSegmentReMap(false),
            SetCOMScanDirection(Direction::Normal),
            // electrical configuration
            SetChargePeriods {
                precharge: Some(2),
                discharge: 2,
            },
            SetVCOMHDeselectLevel(0x35),
            SetDCDCSettings(0xF),
            // intensity
            SetContrastControl(128), // 110 / 256
            ForceEntireDisplay(false),
            // display & addressing mode
            SetDisplayMode(DisplayMode::BlackOnWhite),
        ];

        match sh1107.run(init_sequence).await {
            Ok(_) => {}
            Err(e) => return Err((sh1107.release(), e)),
        }

        Ok(Display(sh1107))
    }
    pub async fn set_state(&mut self, state: DisplayState) -> Result<(), T::Error> {
        self.0.run([Command::DisplayOnOff(state)]).await
    }
    pub async fn set_start_line(&mut self, line: u8) -> Result<(), T::Error> {
        self.0.run([Command::SetStartLine(line)]).await
    }
    pub async fn set_contrast(&mut self, contrast: u8) -> Result<(), T::Error> {
        self.0.run([Command::SetContrastControl(contrast)]).await
    }
    pub async fn flip_horizontal(&mut self, flip: bool) -> Result<(), T::Error> {
        let commands = if flip {
            [
                Command::SetCOMScanDirection(Direction::Inverted),
                Command::SetDisplayOffset(32),
            ]
        } else {
            [
                Command::SetCOMScanDirection(Direction::Normal),
                Command::SetDisplayOffset(96),
            ]
        };
        self.0.run(commands).await
    }
    pub async fn flip_vertical(&mut self, flip: bool) -> Result<(), T::Error> {
        self.0.run([Command::SetSegmentReMap(flip)]).await
    }

    pub async fn write_frame_by_column(
        &mut self,
        dest: Destination,
        mut buf: impl Iterator<Item = u8>,
    ) -> Result<(), T::Error> {
        self.0
            .run([Command::SetAddressMode(AddressMode::Column)])
            .await?;

        let buf = &mut buf;

        for col in 0..COLUMN {
            self.0
                .run_then_write_to_ram(
                    [
                        Command::SetColumnAddress(
                            match dest {
                                Destination::Frame1 => 0,
                                Destination::Frame2 => 64,
                            } + col,
                        ),
                        Command::SetPageAddress(0),
                    ],
                    buf.take(PAGE.into()),
                )
                .await?;
        }
        Ok(())
    }
    pub async fn write_frame_by_page(
        &mut self,
        dest: Destination,
        mut buf: impl Iterator<Item = u8>,
    ) -> Result<(), T::Error> {
        self.0
            .run([Command::SetAddressMode(AddressMode::Page)])
            .await?;

        let buf = &mut buf;
        for page in 0..PAGE {
            self.0
                .run_then_write_to_ram(
                    [
                        Command::SetColumnAddress(match dest {
                            Destination::Frame1 => 0,
                            Destination::Frame2 => 64,
                        }),
                        Command::SetPageAddress(page),
                    ],
                    buf.take(COLUMN.into()),
                )
                .await?;
        }
        Ok(())
    }
    pub async fn read_frame(&mut self, buf: &mut [u8]) -> Result<(), T::Error> {
        self.0
            .run([Command::SetAddressMode(AddressMode::Page)])
            .await?;
        for page in 0..PAGE {
            self.0
                .run([Command::SetColumnAddress(0), Command::SetPageAddress(page)])
                .await?;

            let col = usize::from(COLUMN);
            let start = usize::from(page) * col;
            let end = start + col - 1;
            self.0.read_from_ram(&mut buf[start..=end]).await?;
        }
        Ok(())
    }

    pub async fn is_busy(&mut self) -> Result<bool, T::Error> {
        self.0.is_busy().await
    }

    pub async fn wait_while_busy(&mut self) -> Result<(), T::Error> {
        while self.is_busy().await? {}
        Ok(())
    }

    pub fn release(self) -> T {
        self.0.release()
    }
}

#[cfg(feature = "embedded-graphics")]
pub use self::embedded_graphics::BufferedDisplay;

#[cfg(feature = "embedded-graphics")]
mod embedded_graphics {
    use core::ops::Deref;
    use core::ops::DerefMut;

    use crate::ValidBus;
    use crate::COLUMN;
    use crate::PAGE;
    use crate::ROW;

    use super::Destination;
    use super::Display;
    use super::SevenBitAddress;
    use embedded_graphics::pixelcolor::BinaryColor;
    use embedded_graphics::prelude::*;
    use embedded_graphics::primitives::Rectangle;
    use itertools::Itertools;
    use sh1107::AddressMode;
    use sh1107::Command;

    pub struct BufferedDisplay<T, const ADDRESS: SevenBitAddress> {
        display: Display<T, ADDRESS>,
        bitmask: [u8; 128 * 64],
        bitmap: [u8; 128 * 64],
    }
    impl<T, const ADDRESS: SevenBitAddress> Deref for BufferedDisplay<T, ADDRESS> {
        type Target = Display<T, ADDRESS>;

        fn deref(&self) -> &Self::Target {
            &self.display
        }
    }
    impl<T, const ADDRESS: SevenBitAddress> DerefMut for BufferedDisplay<T, ADDRESS> {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.display
        }
    }
    impl<T: ValidBus, const ADDRESS: SevenBitAddress> BufferedDisplay<T, ADDRESS> {
        pub async fn new(i2c_bus: T) -> Result<Self, (T, T::Error)> {
            // on startup the whole display is considered dirty
            Ok(Self {
                display: Display::new(i2c_bus).await?,
                bitmask: [0xFF; 128 * 64],
                bitmap: [0; 128 * 64],
            })
        }
        pub async fn flush(&mut self) -> Result<(), <T as embedded_hal::i2c::ErrorType>::Error> {
            self.flush_to(Destination::Frame1).await
        }
        pub async fn flush_to(
            &mut self,
            destination: Destination,
        ) -> Result<(), <T as embedded_hal::i2c::ErrorType>::Error> {
            self.display
                .0
                .run([Command::SetAddressMode(AddressMode::Page)])
                .await?;

            // set_position overhead = 6
            //

            use itertools::{iproduct, izip};
            let mut idx = 0;
            // worst case scenario is each page is processed as (1word + 8 skip after)*7 + 1word
            let pages = itertools::put_back(izip!(
                iproduct!(0..PAGE, 0..COLUMN),
                self.bitmask.iter().cloned()
            ))
            .batching(|it| {
                // count the number of skip
                // count the number of word to send
                // count the number to skip after

                let mut skip_before = 0;
                let mut take_count = 0;
                let mut skip_after = 0;
                let mut first = None;

                while let Some((coords, mask)) = it.next() {
                    if first.is_none() && mask == 0 {
                        skip_before += 1;
                        continue;
                    }
                    if let Some((first_page, _)) = first {
                        if first_page != coords.0 {
                            it.put_back((coords, mask));
                            break;
                        }
                    } else {
                        first = Some(coords);
                    }
                    if mask == 0 {
                        skip_after += 1;
                    } else {
                        take_count += 1 + skip_after;
                        skip_after = 0;
                    }
                    if skip_after == 8 || (take_count == COLUMN.into()) {
                        break;
                    }
                }
                let start = idx + skip_before;
                let end = start + take_count;
                idx = end + skip_after;

                first.map(move |coord| (coord, start..end))
            });

            for ((page, col), range) in pages {
                self.display
                    .0
                    .run_then_write_to_ram(
                        [
                            Command::SetColumnAddress(
                                match destination {
                                    Destination::Frame1 => 0,
                                    Destination::Frame2 => 64,
                                } + col,
                            ), // 2bytes + intersperse
                            Command::SetPageAddress(page), // 1 byte + intersperse
                        ],
                        self.bitmap[range].iter().cloned(),
                    )
                    .await?;
            }
            self.bitmask.fill(0);

            Ok(())
        }
    }

    impl<T, const ADDRESS: SevenBitAddress> Dimensions for BufferedDisplay<T, ADDRESS> {
        fn bounding_box(&self) -> Rectangle {
            Rectangle::new(Point::new(0, 0), Size::new(64, 128))
        }
    }

    impl<T, const ADDRESS: SevenBitAddress> DrawTarget for BufferedDisplay<T, ADDRESS> {
        type Color = BinaryColor;

        type Error = core::convert::Infallible;

        fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
        where
            I: IntoIterator<Item = Pixel<Self::Color>>,
        {
            for Pixel(coord, color) in pixels
                .into_iter()
                .filter(|Pixel(coord, _)| coord.x < COLUMN.into() && coord.y < ROW.into())
            {
                let page = coord.y / 8;
                let lsh = coord.y % 8;
                let column = coord.x;

                let idx = page * i32::from(COLUMN) + column;

                let pixel = (matches!(color, BinaryColor::On) as u8) << lsh;
                let mask = 1 << lsh;

                self.bitmask[idx as usize] |= mask;
                self.bitmap[idx as usize] = (self.bitmap[idx as usize] & !mask) | pixel;
            }
            Ok(())
        }
    }
}
