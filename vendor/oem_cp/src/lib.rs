#![cfg_attr(not(test), no_std)]

#[cfg(feature = "alloc")]
extern crate alloc;

#[path = "code_table.generated.rs"]
pub mod code_table;

#[cfg(feature = "alloc")]
mod string;

use core::fmt;

#[cfg(feature = "alloc")]
pub use string::*;

/// The type of hashmap used in this crate.
///
/// The hash library may be changed in the future release.
/// Make sure to use only APIs compatible with `std::collections::HashMap`.
pub type OEMCPHashMap<K, V> = phf::Map<K, V>;

pub mod code_table_type {
    /// Wrapper enumerate for decoding tables
    ///
    /// It has 2 types: `Complete`, complete tables (it doesn't have undefined codepoints) / `Incomplete`, incomplete tables (does have ones)
    #[derive(Debug, Clone)]
    pub enum TableType {
        /// complete table, which doesn't have any undefined codepoints
        Complete(&'static [char; 128]),
        /// incomplete table, which has some undefined codepoints
        Incomplete(&'static [Option<char>; 128]),
    }
}

#[derive(Debug)]
pub struct TryFromCharError;

impl fmt::Display for TryFromCharError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("unicode code point out of range")
    }
}

#[derive(Debug)]
pub struct TryFromU8Error;

impl fmt::Display for TryFromU8Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("code point out of range")
    }
}

pub trait IncompleteCp:
    Clone
    + Copy
    + fmt::Debug
    + fmt::Display
    + TryFrom<char>
    + TryFrom<u8>
    + Into<char>
    + Into<u8>
    + PartialEq<u8>
{
    fn from_char_lossy(c: char) -> Self;
    fn from_u8_lossy(cp: u8) -> Self;
}

pub trait CompleteCp: IncompleteCp + From<u8> {}

const REPLACEMENT: u8 = b'?';

macro_rules! cp_impl {
    ($Cp:ident(Common, $ENCODING_TABLE:ident, $DECODING_TABLE:ident)) => {
        #[derive(Clone, Copy, PartialEq, Eq, Debug)]
        #[repr(transparent)]
        pub struct $Cp(pub u8);

        impl IncompleteCp for $Cp {
            fn from_char_lossy(c: char) -> Self {
                Self::try_from(c).unwrap_or(Self(REPLACEMENT))
            }

            fn from_u8_lossy(cp: u8) -> Self {
                Self::try_from(cp).unwrap_or(Self(REPLACEMENT))
            }
        }

        impl PartialEq<u8> for $Cp {
            fn eq(&self, other: &u8) -> bool {
                self.0.eq(other)
            }
        }

        impl TryFrom<char> for $Cp {
            type Error = TryFromCharError;

            fn try_from(value: char) -> Result<Self, Self::Error> {
                if (value as u32) < 128 {
                    Ok(Self(value as u8))
                } else {
                    code_table::$ENCODING_TABLE
                        .get(&value)
                        .copied()
                        .ok_or(TryFromCharError)
                        .map(Self)
                }
            }
        }

        impl From<$Cp> for u8 {
            fn from(value: $Cp) -> Self {
                value.0
            }
        }

        impl fmt::Display for $Cp {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                char::from(*self).fmt(f)
            }
        }
    };
    ($Cp:ident(Complete, $ENCODING_TABLE:ident, $DECODING_TABLE:ident)) => {
        cp_impl! { $Cp(Common, $ENCODING_TABLE, $DECODING_TABLE) }

        impl CompleteCp for $Cp {}

        impl From<u8> for $Cp {
            fn from(value: u8) -> Self {
                Self(value)
            }
        }

        impl From<$Cp> for char {
            fn from(value: $Cp) -> Self {
                if value.0 < 128 {
                    value.0 as char
                } else {
                    code_table::$DECODING_TABLE[usize::from(value.0 - 128)]
                }
            }
        }
    };
    ($Cp:ident(Incomplete, $ENCODING_TABLE:ident, $DECODING_TABLE:ident)) => {
        cp_impl! { $Cp(Common, $ENCODING_TABLE, $DECODING_TABLE) }

        impl TryFrom<u8> for $Cp {
            type Error = TryFromU8Error;

            fn try_from(value: u8) -> Result<Self, Self::Error> {
                if value < 128 || code_table::$DECODING_TABLE[usize::from(value - 128)].is_some() {
                    Ok(Self(value))
                } else {
                    Err(TryFromU8Error)
                }
            }
        }

        impl From<$Cp> for char {
            fn from(value: $Cp) -> Self {
                if value.0 < 128 {
                    value.0 as char
                } else {
                    code_table::$DECODING_TABLE[usize::from(value.0 - 128)].unwrap()
                }
            }
        }
    };
    ($($Cp:ident($Type:ident, $ENCODING_TABLE:ident, $DECODING_TABLE:ident),)*) => {
        $(cp_impl! { $Cp($Type, $ENCODING_TABLE, $DECODING_TABLE) })*
    };
}

cp_impl! {
    Cp437(Complete, ENCODING_TABLE_CP437, DECODING_TABLE_CP437),
    Cp720(Complete, ENCODING_TABLE_CP720, DECODING_TABLE_CP720),
    Cp737(Complete, ENCODING_TABLE_CP737, DECODING_TABLE_CP737),
    Cp775(Complete, ENCODING_TABLE_CP775, DECODING_TABLE_CP775),
    Cp850(Complete, ENCODING_TABLE_CP850, DECODING_TABLE_CP850),
    Cp852(Complete, ENCODING_TABLE_CP852, DECODING_TABLE_CP852),
    Cp855(Complete, ENCODING_TABLE_CP855, DECODING_TABLE_CP855),
    Cp857(Incomplete, ENCODING_TABLE_CP857, DECODING_TABLE_CP857),
    Cp858(Complete, ENCODING_TABLE_CP858, DECODING_TABLE_CP858),
    Cp860(Complete, ENCODING_TABLE_CP860, DECODING_TABLE_CP860),
    Cp861(Complete, ENCODING_TABLE_CP861, DECODING_TABLE_CP861),
    Cp862(Complete, ENCODING_TABLE_CP862, DECODING_TABLE_CP862),
    Cp863(Complete, ENCODING_TABLE_CP863, DECODING_TABLE_CP863),
    Cp864(Incomplete, ENCODING_TABLE_CP864, DECODING_TABLE_CP864),
    Cp865(Complete, ENCODING_TABLE_CP865, DECODING_TABLE_CP865),
    Cp866(Complete, ENCODING_TABLE_CP866, DECODING_TABLE_CP866),
    Cp869(Complete, ENCODING_TABLE_CP869, DECODING_TABLE_CP869),
    Cp874(Incomplete, ENCODING_TABLE_CP874, DECODING_TABLE_CP874),
}
