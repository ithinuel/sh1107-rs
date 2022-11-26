#![no_std]
#![allow(incomplete_features)]
#![feature(async_fn_in_trait)]
//! See the [datasheet](https://www.displayfuture.com/Display/datasheet/controller/SH1107.pdf) for
//! further details

use core::{
    iter::once,
    ops::{Deref, DerefMut},
};

use embedded_hal_async::i2c::{AddressMode as I2CAddressMode, ErrorType, I2c, SevenBitAddress};
use itertools::Itertools;

pub trait WriteIter<A: I2CAddressMode>: ErrorType {
    /// Writes bytes obtained form the iterator.
    async fn write_iter<'a, U>(&'a mut self, address: A, bytes: U) -> Result<(), Self::Error>
    where
        U: IntoIterator<Item = u8> + 'a;
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum DisplayState {
    Off,
    On,
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Direction {
    Normal,
    Inverted,
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum AddressMode {
    Page,
    Column,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum DisplayMode {
    BlackOnWhite,
    WhiteOnBlack,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Command {
    SetColumnAddress(u8),
    SetAddressMode(AddressMode),
    SetDisplayMode(DisplayMode),
    ForceEntireDisplay(bool),
    SetClkDividerOscFrequency {
        divider: u8,
        osc_freq_ratio: i8,
    },
    SetMultiplexRatio(u8),
    SetStartLine(u8),
    SetSegmentReMap(bool),
    SetCOMScanDirection(Direction),
    SetDisplayOffset(u8),
    SetContrastControl(u8),
    /// Set Charge & Discharge period
    SetChargePeriods {
        precharge: Option<u8>,
        discharge: u8,
    },
    SetVCOMHDeselectLevel(u8),
    SetDCDCSettings(u8),
    DisplayOnOff(DisplayState),
    SetPageAddress(u8),
    StartReadModifyWrite,
    EndReadModifyWrite,
    Nop,
}

impl Command {
    fn encode(self) -> impl Iterator<Item = u8> {
        use either::Either::*;
        match self {
            Self::SetColumnAddress(addr) => {
                assert!(addr < 128);
                Right([addr & 0xF, 0x10 | ((addr & 0x70) >> 4)])
            }
            Self::SetAddressMode(mode) => {
                Left(0x20 | if let AddressMode::Page = mode { 0 } else { 1 })
            }
            Self::SetContrastControl(contrast) => Right([0x81, contrast]),
            Self::SetSegmentReMap(is_remapped) => Left(0xA0 | if is_remapped { 1 } else { 0 }),
            Self::SetMultiplexRatio(ratio) => {
                assert!((1..=128).contains(&ratio));
                Right([0xA8, ratio - 1])
            }
            Self::ForceEntireDisplay(state) => Left(0xA4 | if state { 1 } else { 0 }),
            Self::SetDisplayMode(mode) => Left(
                0xA6 | if let DisplayMode::WhiteOnBlack = mode {
                    1
                } else {
                    0
                },
            ),
            Self::SetDisplayOffset(offset) => Right([0xD3, offset & 0x7F]),
            Self::SetDCDCSettings(cfg) => Right([0xAD, 0x80 | (cfg & 0x0F)]),
            Self::DisplayOnOff(state) => {
                Left(0xAE | if let DisplayState::On = state { 1 } else { 0 })
            }
            Self::SetPageAddress(addr) => {
                assert!(addr < 16);
                Left(0xB0 | (addr & 0x0F))
            }
            Self::SetCOMScanDirection(dir) => {
                Left(0xC0 | if let Direction::Normal = dir { 0 } else { 0x08 })
            }
            Self::SetClkDividerOscFrequency {
                divider,
                osc_freq_ratio,
            } => {
                assert!(
                    osc_freq_ratio % 5 == 0,
                    "osc_freq_ratio must be a multiple of 5."
                );
                assert!(
                    (-25..=50).contains(&osc_freq_ratio),
                    "osc_freq_ratio must be within [-25; 50]"
                );
                assert!((1..=16).contains(&divider), "divider must be in [1; 16]");

                let osc_freq_ratio = osc_freq_ratio / 5 + 5;
                Right([0xD5, ((osc_freq_ratio & 0xF) << 4) as u8 | (divider - 1)])
            }
            Self::SetChargePeriods {
                precharge,
                discharge,
            } => {
                let precharge = if let Some(v) = precharge {
                    assert!((1..=15).contains(&v));
                    v
                } else {
                    0
                };
                assert!((1..=15).contains(&discharge));
                let arg = discharge << 4 | precharge;

                Right([0xD9, arg])
            }
            Self::SetVCOMHDeselectLevel(arg) => Right([0xDB, arg]),
            Self::SetStartLine(line) => {
                assert!(line < 128);

                Right([0xDC, line & 0x7F])
            }
            Self::StartReadModifyWrite => Left(0xE0),
            Self::EndReadModifyWrite => Left(0xEE),
            Self::Nop => Left(0xE3),
        }
        .map_left(|v| [v])
        .into_iter()
    }
}

pub struct Sh1107<T, const ADDRESS: SevenBitAddress>(T);

impl<T, U, V, const ADDRESS: SevenBitAddress> Sh1107<T, ADDRESS>
where
    T: WriteIter<SevenBitAddress, Error = V> + Deref<Target = U> + DerefMut,
    U: I2c<SevenBitAddress, Error = V>,
{
    pub fn new(i2c: T) -> Self {
        Self(i2c)
    }

    pub async fn run(&mut self, commands: impl IntoIterator<Item = Command>) -> Result<(), V> {
        self.0
            .write_iter(
                ADDRESS,
                Iterator::chain(once(0x00), commands.into_iter().flat_map(Command::encode)),
            )
            .await
    }

    pub async fn write_to_ram(
        &mut self,
        buf: impl IntoIterator<Item = u8>,
    ) -> Result<(), <T as ErrorType>::Error> {
        self.0
            .write_iter(
                ADDRESS, // Write data, no other control byte
                once(0x40).chain(buf),
            )
            .await
    }

    pub async fn run_then_write_to_ram(
        &mut self,
        commands: impl IntoIterator<Item = Command>,
        data: impl IntoIterator<Item = u8>,
    ) -> Result<(), V> {
        self.0
            .write_iter(
                ADDRESS,
                // command phase
                Iterator::chain(
                    once(0x80),
                    Itertools::intersperse(commands.into_iter().flat_map(Command::encode), 0x80),
                )
                // transition to data phase
                .chain(once(0x40))
                .chain(data.into_iter()),
            )
            .await
    }

    pub async fn read_from_ram(&mut self, buf: &mut [u8]) -> Result<(), V> {
        self.0.write_read(ADDRESS, &[0x40], buf).await
    }

    pub async fn is_busy(&mut self) -> Result<bool, V> {
        let mut res = 0u8;
        self.0
            .write_read(ADDRESS, &[0x80], core::slice::from_mut(&mut res))
            .await?;
        Ok((res & 0x80) != 0)
    }

    pub fn release(self) -> T {
        self.0
    }
}

/// Helper macro to implement Deref, DerefMut and sh1107::WriteIter on a new type.
#[macro_export]
macro_rules! impl_write_iter {
    ($outer:ty => $inner:ty : $method:ident) => {
        impl Deref for $outer {
            type Target = $inner;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }
        impl DerefMut for $outer {
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.0
            }
        }
        impl ErrorType for $outer {
            type Error = <$inner as ErrorType>::Error;
        }
        impl sh1107::WriteIter<SevenBitAddress> for I2CPeriph {
            async fn write_iter<'a, U>(
                &'a mut self,
                address: SevenBitAddress,
                bytes: U,
            ) -> Result<(), Self::Error>
            where
                U: IntoIterator<Item = u8> + 'a,
            {
                self.0.$method(address, bytes).await
            }
        }
    };
}
