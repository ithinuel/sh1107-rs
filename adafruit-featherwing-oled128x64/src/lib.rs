#![no_std]
use embassy_traits::i2c::I2c;
use embassy_traits::i2c::{SevenBitAddress, WriteIter};
use sh1107::Command;
use sh1107::Direction;
use sh1107::{AddressMode, Sh1107};

pub use sh1107::DisplayState;

pub struct Display<T, const ADDRESS: SevenBitAddress>(Sh1107<T, ADDRESS>);

impl<U, T: WriteIter<Error = U> + I2c<Error = U>, const ADDRESS: SevenBitAddress>
    Display<T, ADDRESS>
{
    pub async fn new(i2c_bus: T) -> Result<Self, U> {
        let mut sh1107 = Sh1107::new(i2c_bus);

        use Command::*;
        const INIT_SEQUENCE: [Command; 11] = [
            DisplayOnOff(DisplayState::Off),
            SetClkDividerOscFrequency {
                divider: 1,
                osc_freq_ratio: 0,
            },
            SetMultiplexRatio(127),
            SetDisplayOffset(96),
            SetSegmentReMap(false),
            SetCOMScanDirection(Direction::Normal),
            SetContrastControl(110), // 110 / 256
            SetChargePeriods {
                precharge: Some(2),
                discharge: 2,
            },
            SetVCOMHDeselectLevel(0x35),
            ForceEntireDisplay(false),
            // power up VDD
            DisplayOnOff(DisplayState::On),
        ];
        sh1107.run(INIT_SEQUENCE).await?;
        Ok(Display(sh1107))
    }
    pub async fn set_state(&mut self, state: DisplayState) -> Result<(), U> {
        self.0.run([Command::DisplayOnOff(state)]).await
    }

    pub async fn write_frame_by_column(
        &mut self,
        mut buf: impl Iterator<Item = u8>,
    ) -> Result<(), U> {
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
    ) -> Result<(), U> {
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
    pub async fn read_frame(&mut self, buf: &mut [u8]) -> Result<(), U> {
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
}
//sh1107
//    .run([Command::DisplayOnOff(DisplayState::On)])
//    .await
//    .expect("Woops");
//write_frame_by_page(&mut sh1107, GLYPHS.iter().cloned())
//    .await
//    .expect("Woops you failed");
