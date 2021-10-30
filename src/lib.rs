#![no_std]
//! See the [datasheet](https://www.displayfuture.com/Display/datasheet/controller/SH1107.pdf) for
//! further details

use embassy_traits::i2c::{I2c, SevenBitAddress, WriteIter};
use embedded_graphics_core::{
    draw_target::DrawTarget, geometry::OriginDimensions, pixelcolor::BinaryColor, prelude::Size,
    Pixel,
};
use itertools::Itertools;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DisplayState {
    Off,
    On,
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Direction {
    Normal,
    Inverted,
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AddressMode {
    Page,
    Vertical,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Command {
    SetColumnAddress(u8),
    SetMemAddressMode(AddressMode),
    SetDisplay(bool),
    SetEntireDisplay(DisplayState),
    SetClkDividerOscFrequency(u8),
    SetMultiplexRatio(u8),
    SetStartLine(u8),
    SetSegmentReMap(bool),
    SetCOMScanDirection(Direction),
    SetDisplayOffset(u8),
    SetContrastControl(u8),
    /// Set Charge & Discharge period
    SetChargePeriods(u8),
    SetVCOMHDeselectLevel(u8),
    SetDCDCSettings(u8),
    DisplayOnOff(DisplayState),
    SetPageAddress(u8),
}

impl Command {
    fn into_array(self) -> impl Iterator<Item = u8> {
        use either::Either::*;
        match self {
            Self::SetColumnAddress(addr) => Right([addr & 0xF, (addr & 0x70) >> 4]),
            Self::SetMemAddressMode(mode) => {
                Left([0x20 | if mode == AddressMode::Page { 0 } else { 1 }])
            }
            Self::SetContrastControl(contrast) => Right([0x81, contrast]),
            Self::SetSegmentReMap(is_remapped) => Left([0xA0 | if is_remapped { 1 } else { 0 }]),
            Self::SetMultiplexRatio(ratio) => Right([0xA8, ratio & 0x7F]),
            Self::SetEntireDisplay(state) => {
                Left([0xA4 | if state == DisplayState::On { 1 } else { 0 }])
            }
            Self::SetDisplayOffset(offset) => Right([0xD3, offset & 0x7F]),
            Self::SetDCDCSettings(cfg) => Right([0xAD, 0x80 | (cfg & 0x0F)]),
            Self::DisplayOnOff(state) => {
                Left([0xAE | if state == DisplayState::On { 1 } else { 0 }])
            }
            Self::SetPageAddress(addr) => Left([0xB0 | (addr & 0x0F)]),
            Self::SetCOMScanDirection(dir) => {
                Left([0xC0 | if dir == Direction::Normal { 0 } else { 0x08 }])
            }
            Self::SetClkDividerOscFrequency(args) => Right([0xD5, args]),
            Self::SetChargePeriods(arg) => Right([0xD9, arg]),
            Self::SetVCOMHDeselectLevel(arg) => Right([0xDB, arg]),
            Self::SetStartLine(line) => Right([0xDC, line & 0x7F]),

            _ => todo!(),
        }
        .into_iter()
    }
}

pub struct Sh1107<T, const COL: u32, const ROW: u32, const ADDRESS: SevenBitAddress>(T);

impl<T, const COL: u32, const ROW: u32, const ADDRESS: SevenBitAddress> Sh1107<T, COL, ROW, ADDRESS>
where
    T: I2c<SevenBitAddress> + WriteIter<SevenBitAddress>,
{
    pub fn new(i2c: T) -> Self {
        Self(i2c)
    }

    pub async fn run(
        &mut self,
        commands: impl IntoIterator<Item = Command>,
    ) -> Result<(), <T as WriteIter>::Error> {
        self.0
            .write_iter(
                ADDRESS,
                Iterator::chain(
                    core::iter::once(0x80),
                    Itertools::intersperse(
                        commands.into_iter().flat_map(Command::into_array),
                        0x80,
                    ),
                ),
            )
            .await
    }

    // Set col address hi: 0b0001_0xxx
    // Set col address lo: 0b0000_xxxx
    // defaults to 0 on POR
    //
    // Set Mem address mode: 0b0010_000x
    // 0 = Page address mode (default)
    // 1 = Vertical address mode
    //
    // Set Contrast Control Register: 0x81
    // Expects 1 data byte to set the contrast. (0-255, defaults to 128 on POR)
    //
    // Set Segment re-map: 0b1010_000x (horizontal flip)
    // 0 = normal direction (default)
    // 1 = reverse direction
    //
    // Set Multiplex Ratio: 0xA8
    // Expects 1 data byte to set the multiplex ratio. (0-127, defaults to 127 on POR)
    //
    // Set Entire display OFF/ON: 0b1010_010x
    // 0 = Off (default)
    // 1 = On
    //
    // Set Normal/Reverse display: 0b1010011x
    // 0 = Normal (white on black, default)
    // 1 = Reverse (black on white)
    //
    // Set Display Offset: 0xD3
    // Expects 1 data byte to set the offset. (0-127, defaults to 0 on POR)
    //
    // Set DC-DC settings: (I don't understand this command, don't use)
    //
    // Display OFF/ON: 0b1010_111x
    // 0 = Off (default)
    // 1 = On
    //
    // Set Page Address: 0b1011_xxxx
    // xxxx = page address (0-15, defaults to 0 on POR)
    //
    // Set Common Output Scan Direction: 0b1100_xyyy
    // x:0 = Scan from COM0 to COM[n-1]
    // x:1 = Scan from COM[n-1] to COM0
    //
    // yyy = ignored, set to 0
    //
    //
    // Set Display Clock Device Ratio/Osc Frequency: 0xD5
    // Expects 1 data byte built as follow:
    // 4 least significant/lower bits: Divider as (divider - 1)
    // 4 most significant/upper bits: ((1+val*0.05-0.25) * fosc)
    // defaults to 0b0101_0000 on POR (ratio = 0, val = 5)
}

impl<T, const COL: u32, const ROW: u32, const ADDRESS: SevenBitAddress> OriginDimensions
    for Sh1107<T, COL, ROW, ADDRESS>
{
    fn size(&self) -> Size {
        Size::new(COL, ROW)
    }
}
impl<T, const COL: u32, const ROW: u32, const ADDRESS: SevenBitAddress> DrawTarget
    for Sh1107<T, COL, ROW, ADDRESS>
{
    type Color = BinaryColor;

    type Error = ();

    fn draw_iter<I>(&mut self, _pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        todo!()
    }
}
