#![no_std]

use embedded_hal_async::i2c::I2c;
use embedded_hal_async::i2c::SevenBitAddress;
use sh1107::Direction;
use sh1107::WriteIter;
use sh1107::{AddressMode, Sh1107};
use sh1107::{Command, DisplayMode};

pub use sh1107::DisplayState;

pub struct Display<T, const ADDRESS: SevenBitAddress>(Sh1107<T, ADDRESS>);

impl<T: WriteIter<SevenBitAddress> + I2c, const ADDRESS: SevenBitAddress> Display<T, ADDRESS> {
    pub async fn new(i2c_bus: T) -> Result<Self, (T, T::Error)> {
        let mut sh1107 = Sh1107::new(i2c_bus);

        use Command::*;
        const INIT_SEQUENCE: [Command; 14] = [
            DisplayOnOff(DisplayState::Off),
            SetClkDividerOscFrequency {
                divider: 2,        // divide by 2
                osc_freq_ratio: 0, // +0%
            },
            SetMultiplexRatio(128),
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
            // intensity
            SetContrastControl(110), // 110 / 256
            ForceEntireDisplay(false),
            // display & addressing mode
            SetDisplayMode(DisplayMode::BlackOnWhite),
            Command::SetAddressMode(AddressMode::Page),
            // power up VDD
            DisplayOnOff(DisplayState::On),
        ];

        match sh1107.run(INIT_SEQUENCE).await {
            Ok(_) => {}
            Err(e) => return Err((sh1107.release(), e)),
        }

        Ok(Display(sh1107))
    }
    pub async fn set_state(&mut self, state: DisplayState) -> Result<(), T::Error> {
        self.0.run([Command::DisplayOnOff(state)]).await
    }
    pub async fn set_line_and_offset(&mut self, line: u8, offset: u8) -> Result<(), T::Error> {
        self.0
            .run([
                Command::SetStartLine(line),
                Command::SetDisplayOffset(offset),
            ])
            .await
    }

    pub async fn write_frame_by_column(
        &mut self,
        mut buf: impl Iterator<Item = u8>,
    ) -> Result<(), T::Error> {
        self.0
            .run([Command::SetAddressMode(AddressMode::Column)])
            .await?;

        let buf = &mut buf;
        // 128 col of 16 pages
        for col in 0..64 {
            self.0
                .run_then_write_to_ram(
                    [
                        Command::SetColumnAddress(col as u8),
                        Command::SetPageAddress(0),
                    ],
                    buf.take(16),
                )
                .await?;
        }
        Ok(())
    }
    pub async fn write_frame_by_page(
        &mut self,
        mut buf: impl Iterator<Item = u8>,
    ) -> Result<(), T::Error> {
        self.0
            .run([Command::SetAddressMode(AddressMode::Page)])
            .await?;

        let buf = &mut buf;
        // 16 pages of 128 COL
        for page in 0..16 {
            self.0
                .run_then_write_to_ram(
                    [
                        Command::SetColumnAddress(0),
                        Command::SetPageAddress(page as u8),
                    ],
                    buf.take(64),
                )
                .await?;
        }
        Ok(())
    }
    pub async fn read_frame(&mut self, buf: &mut [u8]) -> Result<(), T::Error> {
        self.0
            .run([Command::SetAddressMode(AddressMode::Page)])
            .await?;
        for page in 0..16 {
            self.0
                .run([Command::SetColumnAddress(0), Command::SetPageAddress(page)])
                .await?;

            let start = usize::from(page) * 128;
            let end = start + 127;
            self.0.read_from_ram(&mut buf[start..=end]).await?;
        }
        Ok(())
    }

    pub async fn is_busy(&mut self) -> Result<bool, T::Error> {
        self.0.is_busy().await
    }

    pub fn release(self) -> T {
        self.0.release()
    }
}
//sh1107
//    .run([Command::DisplayOnOff(DisplayState::On)])
//    .await
//    .expect("Woops");
//write_frame_by_page(&mut sh1107, GLYPHS.iter().cloned())
//    .await
//    .expect("Woops you failed");
