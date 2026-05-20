use alloc::string::String;
use alloc::vec::Vec;

use crate::{CompleteCp, IncompleteCp, TryFromU8Error};

use super::code_table_type::TableType;
use super::OEMCPHashMap;

use TableType::*;

pub trait StrExt {
    /// ```
    /// use oem_cp::{Cp437, Cp737, StrExt};
    ///
    /// assert_eq!("π≈22/7".to_cp::<Cp437>().unwrap(), vec![0xE3, 0xF7, 0x32, 0x32, 0x2F, 0x37]);
    /// // Archimedes in Greek
    /// assert_eq!("Αρχιμήδης".to_cp::<Cp737>().unwrap(), vec![0x80, 0xA8, 0xAE, 0xA0, 0xA3, 0xE3, 0x9B, 0x9E, 0xAA]);
    /// // Japanese characters are not defined in CP437
    /// assert!("日本語ja_jp".to_cp::<Cp437>().is_err());
    /// ```
    fn to_cp<T: IncompleteCp>(&self) -> Result<Vec<T>, <T as TryFrom<char>>::Error>
    where
        u8: From<T>,
        char: From<T>;

    /// ```
    /// use oem_cp::{Cp437, Cp737, StrExt};
    ///
    /// assert_eq!("π≈22/7".to_cp_lossy::<Cp437>(), vec![0xE3, 0xF7, 0x32, 0x32, 0x2F, 0x37]);
    /// // Archimedes in Greek
    /// assert_eq!("Αρχιμήδης".to_cp_lossy::<Cp737>(), vec![0x80, 0xA8, 0xAE, 0xA0, 0xA3, 0xE3, 0x9B, 0x9E, 0xAA]);
    /// // Japanese characters are not defined in CP437 and replaced with `?` (0x3F)
    /// // "日本語ja_jp" => "???ja_jp"
    /// assert_eq!("日本語ja_jp".to_cp_lossy::<Cp437>(), vec![0x3F, 0x3F, 0x3F, 0x6A, 0x61, 0x5F, 0x6A, 0x70]);
    /// ```
    fn to_cp_lossy<T: IncompleteCp>(&self) -> Vec<T>
    where
        u8: From<T>,
        char: From<T>;
}

impl StrExt for str {
    fn to_cp<T: IncompleteCp>(&self) -> Result<Vec<T>, <T as TryFrom<char>>::Error>
    where
        u8: From<T>,
        char: From<T>,
    {
        self.chars().map(T::try_from).collect()
    }

    fn to_cp_lossy<T: IncompleteCp>(&self) -> Vec<T>
    where
        u8: From<T>,
        char: From<T>,
    {
        self.chars().map(T::from_char_lossy).collect()
    }
}

pub trait StringExt: Sized {
    /// ```
    /// use oem_cp::{Cp874, StringExt};
    ///
    /// // means shrimp in Thai (U+E49 => 0xE9)
    /// assert_eq!(String::try_from_cp::<Cp874>(&[0xA1, 0xD8, 0xE9, 0xA7]).unwrap(), "กุ้ง");
    /// // 0xDB-0xDE,0xFC-0xFF is invalid in CP874 in Windows
    /// assert!(String::try_from_cp::<Cp874>(&[0x30, 0xDB]).is_err());
    /// ```
    fn try_from_cp<T: IncompleteCp>(v: &[u8]) -> Result<Self, TryFromU8Error>
    where
        u8: From<T>,
        char: From<T>,
        TryFromU8Error: From<<T as TryFrom<u8>>::Error>;

    /// ```
    /// use oem_cp::{Cp874, StringExt};
    ///
    /// // means shrimp in Thai (U+E49 => 0xE9)
    /// assert_eq!(String::from_cp_lossy::<Cp874>(&[0xA1, 0xD8, 0xE9, 0xA7]), "กุ้ง");
    /// // 0xDB-0xDE,0xFC-0xFF is invalid in CP874 in Windows
    /// assert_eq!(String::from_cp_lossy::<Cp874>(&[0x30, 0xDB]), "0\u{FFFD}");
    /// ```
    fn from_cp_lossy<T: IncompleteCp>(v: &[u8]) -> Self
    where
        u8: From<T>,
        char: From<T>;

    /// ```
    /// use oem_cp::{Cp437, StringExt};
    ///
    /// assert_eq!(String::from_cp::<Cp437>(&[0xFB, 0xAC, 0x3D, 0xAB]), "√¼=½");
    /// ```
    fn from_cp<T: CompleteCp>(v: &[u8]) -> Self
    where
        u8: From<T>,
        char: From<T>;
}

impl StringExt for String {
    fn from_cp_lossy<T: IncompleteCp>(v: &[u8]) -> Self
    where
        u8: From<T>,
        char: From<T>,
    {
        const REPLACEMENT: char = '\u{FFFD}';
        v.iter()
            .copied()
            .map(|cp| T::try_from(cp).map(char::from).unwrap_or(REPLACEMENT))
            .collect()
    }

    fn from_cp<T: CompleteCp>(v: &[u8]) -> Self
    where
        u8: From<T>,
        char: From<T>,
    {
        v.iter().copied().map(T::from).map(char::from).collect()
    }

    fn try_from_cp<T: IncompleteCp>(v: &[u8]) -> Result<Self, TryFromU8Error>
    where
        u8: From<T>,
        char: From<T>,
        TryFromU8Error: From<<T as TryFrom<u8>>::Error>,
    {
        v.iter()
            .copied()
            .map(|cp| {
                T::try_from(cp)
                    .map(char::from)
                    .map_err(TryFromU8Error::from)
            })
            .collect()
    }
}

impl TableType {
    /// Wrapper function for decoding bytes encoded in SBCSs
    ///
    /// This function returns `None` if any bytes bumps into undefined codepoints
    ///
    /// # Arguments
    ///
    /// * `src` - bytes encoded in SBCS
    ///
    /// # Examples
    ///
    /// ```
    /// use oem_cp::code_table::{DECODING_TABLE_CP437, DECODING_TABLE_CP874};
    /// use oem_cp::code_table_type::TableType;
    /// use TableType::{Complete,Incomplete};
    ///
    /// assert_eq!(Complete(&DECODING_TABLE_CP437).decode_string_checked(&[0xFB, 0xAC, 0x3D, 0xAB]), Some("√¼=½".to_string()));
    /// // means shrimp in Thai (U+E49 => 0xE9)
    /// assert_eq!(Incomplete(&DECODING_TABLE_CP874).decode_string_checked(&[0xA1, 0xD8, 0xE9, 0xA7]), Some("กุ้ง".to_string()));
    /// // 0xDB-0xDE,0xFC-0xFF is invalid in CP874 in Windows (strict mode)
    /// assert_eq!(Incomplete(&DECODING_TABLE_CP874).decode_string_checked(&[0x30, 0xDB]), None);
    /// ```
    pub fn decode_string_checked(&self, src: &[u8]) -> Option<String> {
        match self {
            Complete(table_ref) => Some(decode_string_complete_table(src, table_ref)),
            Incomplete(table_ref) => decode_string_incomplete_table_checked(src, table_ref),
        }
    }
    /// Wrapper function for decoding bytes encoded in SBCSs
    ///
    /// Undefined codepoints are replaced with U+FFFD.
    ///
    /// # Arguments
    ///
    /// * `src` - bytes encoded in SBCS
    ///
    /// # Examples
    ///
    /// ```
    /// use oem_cp::code_table::{DECODING_TABLE_CP437, DECODING_TABLE_CP874};
    /// use oem_cp::code_table_type::TableType;
    /// use TableType::{Complete,Incomplete};
    ///
    /// assert_eq!(Complete(&DECODING_TABLE_CP437).decode_string_lossy(&[0xFB, 0xAC, 0x3D, 0xAB]), "√¼=½".to_string());
    /// // means shrimp in Thai (U+E49 => 0xE9)
    /// assert_eq!(Incomplete(&DECODING_TABLE_CP874).decode_string_lossy(&[0xA1, 0xD8, 0xE9, 0xA7]), "กุ้ง".to_string());
    /// // 0xDB-0xDE,0xFC-0xFF is invalid in CP874 in Windows (strict mode)
    /// assert_eq!(Incomplete(&DECODING_TABLE_CP874).decode_string_lossy(&[0x30, 0xDB]), "0\u{FFFD}".to_string());
    /// ```
    pub fn decode_string_lossy(&self, src: &[u8]) -> String {
        match self {
            Complete(table_ref) => decode_string_complete_table(src, table_ref),
            Incomplete(table_ref) => decode_string_incomplete_table_lossy(src, table_ref),
        }
    }

    pub fn decode_char_checked(&self, byte: u8) -> Option<char> {
        match self {
            Complete(table_ref) => Some(decode_char_complete_table(byte, table_ref)),
            Incomplete(table_ref) => decode_char_incomplete_table_checked(byte, table_ref),
        }
    }
}

/// Decode SBCS (single byte character set) bytes (no undefined codepoints)
///
/// # Arguments
///
/// * `src` - bytes encoded in SBCS
/// * `decoding_table` - table for decoding SBCS (with**out** undefined codepoints)
///
/// # Examples
///
/// ```
/// use oem_cp::decode_string_complete_table;
/// use oem_cp::code_table::DECODING_TABLE_CP437;
///
/// assert_eq!(&decode_string_complete_table(&[0xFB, 0xAC, 0x3D, 0xAB], &DECODING_TABLE_CP437), "√¼=½");
/// ```
pub fn decode_string_complete_table(src: &[u8], decoding_table: &[char; 128]) -> String {
    src.iter()
        .map(|byte| {
            if *byte < 128 {
                *byte as char
            } else {
                decoding_table[(*byte & 127) as usize]
            }
        })
        .collect()
}

/// Decode single SBCS (single byte character set) byte (no undefined codepoints)
///
/// # Arguments
///
/// * `src` - single byte encoded in SBCS
/// * `decoding_table` - table for decoding SBCS (**with** undefined codepoints)
///
/// # Examples
///
/// ```
/// use oem_cp::decode_char_complete_table;
/// use oem_cp::code_table::DECODING_TABLE_CP437;
///
/// assert_eq!(decode_char_complete_table(0xFB, &DECODING_TABLE_CP437), '√');
/// ```
pub fn decode_char_complete_table(src: u8, decoding_table: &[char; 128]) -> char {
    if src < 128 {
        src as char
    } else {
        decoding_table[(src & 127) as usize]
    }
}

/// Decode SBCS (single byte character set) bytes (with undefined codepoints)
///
/// If some undefined codepoints are found, returns `None`.
///
/// # Arguments
///
/// * `src` - bytes encoded in SBCS
/// * `decoding_table` - table for decoding SBCS (**with** undefined codepoints)
///
/// # Examples
///
/// ```
/// use oem_cp::decode_string_incomplete_table_checked;
/// use oem_cp::code_table::DECODING_TABLE_CP874;
///
/// // means shrimp in Thai (U+E49 => 0xE9)
/// assert_eq!(decode_string_incomplete_table_checked(&[0xA1, 0xD8, 0xE9, 0xA7], &DECODING_TABLE_CP874), Some("กุ้ง".to_string()));
/// // 0xDB-0xDE,0xFC-0xFF is invalid in CP874 in Windows
/// assert_eq!(decode_string_incomplete_table_checked(&[0x30, 0xDB], &DECODING_TABLE_CP874), None);
/// ```
pub fn decode_string_incomplete_table_checked(
    src: &[u8],
    decoding_table: &[Option<char>; 128],
) -> Option<String> {
    let mut ret = String::new();
    for byte in src.iter() {
        ret.push(if *byte < 128 {
            *byte as char
        } else {
            decoding_table[(*byte & 127) as usize]?
        });
    }
    Some(ret)
}

/// Decode SBCS (single byte character set) bytes (with undefined codepoints)
///
/// Undefined codepoints are replaced with `U+FFFD` (replacement character).
///
/// # Arguments
///
/// * `src` - bytes encoded in SBCS
/// * `decoding_table` - table for decoding SBCS (**with** undefined codepoints)
///
/// # Examples
///
/// ```
/// use oem_cp::decode_string_incomplete_table_lossy;
/// use oem_cp::code_table::DECODING_TABLE_CP874;
///
/// // means shrimp in Thai (U+E49 => 0xE9)
/// assert_eq!(&decode_string_incomplete_table_lossy(&[0xA1, 0xD8, 0xE9, 0xA7], &DECODING_TABLE_CP874), "กุ้ง");
/// // 0xDB-0xDE,0xFC-0xFF is invalid in CP874 in Windows
/// assert_eq!(&decode_string_incomplete_table_lossy(&[0x30, 0xDB], &DECODING_TABLE_CP874), "0\u{FFFD}");
/// ```
pub fn decode_string_incomplete_table_lossy(
    src: &[u8],
    decoding_table: &[Option<char>; 128],
) -> String {
    src.iter()
        .map(|byte| {
            if *byte < 128 {
                *byte as char
            } else {
                decoding_table[(*byte & 127) as usize].unwrap_or('\u{FFFD}')
            }
        })
        .collect()
}

/// Decode single SBCS (single byte character set) byte (with undefined codepoints)
///
/// If some undefined codepoints are found, returns `None`.
///
/// # Arguments
///
/// * `src` - single byte encoded in SBCS
/// * `decoding_table` - table for decoding SBCS (**with** undefined codepoints)
///
/// # Examples
///
/// ```
/// use oem_cp::decode_char_incomplete_table_checked;
/// use oem_cp::code_table::DECODING_TABLE_CP874;
///
/// assert_eq!(decode_char_incomplete_table_checked(0x85, &DECODING_TABLE_CP874), Some('…'));
/// assert_eq!(decode_char_incomplete_table_checked(0xFC, &DECODING_TABLE_CP874), None);
/// ```
pub fn decode_char_incomplete_table_checked(
    src: u8,
    decoding_table: &[Option<char>; 128],
) -> Option<char> {
    if src < 128 {
        Some(src as char)
    } else {
        decoding_table[(src & 127) as usize]
    }
}

/// Decode single SBCS (single byte character set) byte (with undefined codepoints)
///
/// Undefined codepoints are replaced with `U+FFFD` (replacement character).
///
/// # Arguments
///
/// * `src` - single byte encoded in SBCS
/// * `decoding_table` - table for decoding SBCS (**with** undefined codepoints)
///
/// # Examples
///
/// ```
/// use oem_cp::decode_char_incomplete_table_lossy;
/// use oem_cp::code_table::DECODING_TABLE_CP874;
///
/// assert_eq!(decode_char_incomplete_table_lossy(0x85, &DECODING_TABLE_CP874), '…');
/// assert_eq!(decode_char_incomplete_table_lossy(0xFC, &DECODING_TABLE_CP874), '\u{FFFD}');
/// ```
pub fn decode_char_incomplete_table_lossy(src: u8, decoding_table: &[Option<char>; 128]) -> char {
    if src < 128 {
        src as char
    } else {
        decoding_table[(src & 127) as usize].unwrap_or('\u{FFFD}')
    }
}

/// Encode Unicode string in SBCS (single byte character set)
///
/// If some undefined codepoints are found, returns `None`.
///
/// # Arguments
///
/// * `src` - Unicode string
/// * `encoding_table` - table for encoding in SBCS
///
/// # Examples
///
/// ```
/// use oem_cp::encode_string_checked;
/// use oem_cp::code_table::{ENCODING_TABLE_CP437, ENCODING_TABLE_CP737};
/// assert_eq!(encode_string_checked("π≈22/7", &ENCODING_TABLE_CP437), Some(vec![0xE3, 0xF7, 0x32, 0x32, 0x2F, 0x37]));
/// // Archimedes in Greek
/// assert_eq!(encode_string_checked("Αρχιμήδης", &ENCODING_TABLE_CP737), Some(vec![0x80, 0xA8, 0xAE, 0xA0, 0xA3, 0xE3, 0x9B, 0x9E, 0xAA]));
/// // Japanese characters are not defined in CP437
/// assert_eq!(encode_string_checked("日本語ja_jp", &ENCODING_TABLE_CP437), None);
/// ```
pub fn encode_string_checked(
    src: &str,
    encoding_table: &OEMCPHashMap<char, u8>,
) -> Option<Vec<u8>> {
    let mut ret = Vec::new();
    for c in src.chars() {
        ret.push(if (c as u32) < 128 {
            c as u8
        } else {
            *encoding_table.get(&c)?
        });
    }
    Some(ret)
}

/// Encode Unicode string in SBCS (single byte character set)
///
/// Undefined codepoints are replaced with `0x3F` (`?`).
///
/// # Arguments
///
/// * `src` - Unicode string
/// * `encoding_table` - table for encoding in SBCS
///
/// # Examples
///
/// ```
/// use oem_cp::encode_string_lossy;
/// use oem_cp::code_table::{ENCODING_TABLE_CP437, ENCODING_TABLE_CP737};
/// assert_eq!(encode_string_lossy("π≈22/7", &ENCODING_TABLE_CP437), vec![0xE3, 0xF7, 0x32, 0x32, 0x2F, 0x37]);
/// // Archimedes in Greek
/// assert_eq!(encode_string_lossy("Αρχιμήδης", &ENCODING_TABLE_CP737), vec![0x80, 0xA8, 0xAE, 0xA0, 0xA3, 0xE3, 0x9B, 0x9E, 0xAA]);
/// // Japanese characters are not defined in CP437 and replaced with `?` (0x3F)
/// // "日本語ja_jp" => "???ja_jp"
/// assert_eq!(encode_string_lossy("日本語ja_jp", &ENCODING_TABLE_CP437), vec![0x3F, 0x3F, 0x3F, 0x6A, 0x61, 0x5F, 0x6A, 0x70]);
/// ```
pub fn encode_string_lossy(src: &str, encoding_table: &OEMCPHashMap<char, u8>) -> Vec<u8> {
    src.chars()
        .map(|c| {
            if (c as u32) < 128 {
                c as u8
            } else {
                encoding_table.get(&c).copied().unwrap_or(b'?')
            }
        })
        .collect()
}

/// Encode Unicode char in SBCS (single byte character set)
///
/// If undefined codepoint is found, returns `None`.
///
/// # Arguments
///
/// * `src` - Unicode char
/// * `encoding_table` - table for encoding in SBCS
///
/// # Examples
///
/// ```
/// use oem_cp::encode_char_checked;
/// use oem_cp::code_table::{ENCODING_TABLE_CP437, ENCODING_TABLE_CP737};
/// assert_eq!(encode_char_checked('π', &ENCODING_TABLE_CP437), Some(0xE3));
/// // Archimedes in Greek
/// assert_eq!(encode_char_checked('Α', &ENCODING_TABLE_CP737), Some(0x80));
/// // Japanese characters are not defined in CP437
/// assert_eq!(encode_char_checked('日', &ENCODING_TABLE_CP437), None);
/// ```
pub fn encode_char_checked(src: char, encoding_table: &OEMCPHashMap<char, u8>) -> Option<u8> {
    if (src as u32) < 128 {
        Some(src as u8)
    } else {
        encoding_table.get(&src).copied()
    }
}

/// Encode Unicode char in SBCS (single byte character set)
///
/// Undefined codepoints are replaced with `0x3F` (`?`).
///
/// # Arguments
///
/// * `src` - Unicode char
/// * `encoding_table` - table for encoding in SBCS
///
/// # Examples
///
/// ```
/// use oem_cp::encode_char_lossy;
/// use oem_cp::code_table::{ENCODING_TABLE_CP437, ENCODING_TABLE_CP737};
/// assert_eq!(encode_char_lossy('π', &ENCODING_TABLE_CP437), 0xE3);
/// // Archimedes in Greek
/// assert_eq!(encode_char_lossy('Α', &ENCODING_TABLE_CP737), 0x80);
/// // Japanese characters are not defined in CP437 and replaced with `?` (0x3F)
/// assert_eq!(encode_char_lossy('日', &ENCODING_TABLE_CP437), 0x3F);
/// ```
pub fn encode_char_lossy(src: char, encoding_table: &OEMCPHashMap<char, u8>) -> u8 {
    if (src as u32) < 128 {
        src as u8
    } else {
        encoding_table.get(&src).copied().unwrap_or(b'?')
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::code_table::*;

    static CP437_VALID_PAIRS: &[(&str, &[u8])] = &[
        ("√α²±ß²", &[0xFB, 0xE0, 0xFD, 0xF1, 0xE1, 0xFD]),
        ("és", &[0x82, 0x73]),
        ("più", &[0x70, 0x69, 0x97]),
        ("½÷¼=2", &[0xAB, 0xF6, 0xAC, 0x3D, 0x32]),
    ];
    static CP874_VALID_PAIRS: &[(&str, &[u8])] = &[
        // cspell: disable
        (
            "ราชอาณาจักรไท",
            &[
                0xC3, 0xD2, 0xAA, 0xCD, 0xD2, 0xB3, 0xD2, 0xA8, 0xD1, 0xA1, 0xC3, 0xE4, 0xB7,
            ],
        ),
        (
            "ต้มยำกุ้ง",
            &[0xB5, 0xE9, 0xC1, 0xC2, 0xD3, 0xA1, 0xD8, 0xE9, 0xA7],
        ),
        // cspell: enable
    ];
    static CP857_VALID_PAIRS: &[(&str, &[u8])] = &[
        // cspell: disable
        ("½÷¼=2", &[0xAB, 0xF6, 0xAC, 0x3D, 0x32]),
        ("¼×3=¾", &[0xAC, 0xE8, 0x33, 0x3D, 0xF3]),
        ("İran", &[0x98, 0x72, 0x61, 0x6E]),
        ("ırmak", &[0x8D, 0x72, 0x6D, 0x61, 0x6B]),
        ("iş", &[0x69, 0x9F]),
        // cspell: enable
    ];
    /// OEM SBCSs used in some languages (locales)
    static WINDOWS_USED_CODEPAGES: &[u16] = &[
        437, // 720, // TODO: implement for locales using Arabic alphabets
        737, 775, 850, 852, 855, 857, 862, 866, 874,
    ];
    #[test]
    fn cp437_encoding_test() {
        for &(utf8_ref, cp437_ref) in CP437_VALID_PAIRS {
            assert_eq!(
                &encode_string_lossy(utf8_ref, &ENCODING_TABLE_CP437),
                cp437_ref
            );
            assert_eq!(
                &(encode_string_checked(utf8_ref, &ENCODING_TABLE_CP437).unwrap()),
                cp437_ref
            );
        }
    }
    #[test]
    fn cp437_decoding_test() {
        for &(utf8_ref, cp437_ref) in CP437_VALID_PAIRS {
            assert_eq!(
                &decode_string_complete_table(cp437_ref, &DECODING_TABLE_CP437),
                utf8_ref
            );
        }
    }
    #[test]
    fn cp874_encoding_test() {
        for &(utf8_ref, cp874_ref) in CP874_VALID_PAIRS {
            assert_eq!(
                &encode_string_lossy(utf8_ref, &ENCODING_TABLE_CP874),
                cp874_ref
            );
            assert_eq!(
                &(encode_string_checked(utf8_ref, &ENCODING_TABLE_CP874).unwrap()),
                cp874_ref
            );
        }
    }
    #[test]
    fn cp874_decoding_test() {
        for &(utf8_ref, cp874_ref) in CP874_VALID_PAIRS {
            assert_eq!(
                &decode_string_incomplete_table_lossy(cp874_ref, &DECODING_TABLE_CP874),
                utf8_ref
            );
            assert_eq!(
                &*(decode_string_incomplete_table_checked(cp874_ref, &DECODING_TABLE_CP874)
                    .unwrap_or_else(|| panic!(
                        "{cp874_ref:?} (intended for {utf8_ref:?}) is not a valid cp874 bytes."
                    ))),
                utf8_ref
            );
        }
    }
    #[test]
    fn cp857_encoding_test() {
        for &(utf8_ref, cp857_ref) in CP857_VALID_PAIRS {
            assert_eq!(
                &encode_string_lossy(utf8_ref, &ENCODING_TABLE_CP857),
                cp857_ref
            );
            assert_eq!(
                &(encode_string_checked(utf8_ref, &ENCODING_TABLE_CP857).unwrap()),
                cp857_ref
            );
        }
    }
    #[test]
    fn cp857_decoding_test() {
        for &(utf8_ref, cp857_ref) in CP857_VALID_PAIRS {
            assert_eq!(
                &decode_string_incomplete_table_lossy(cp857_ref, &DECODING_TABLE_CP857),
                utf8_ref
            );
            assert_eq!(
                &*(decode_string_incomplete_table_checked(cp857_ref, &DECODING_TABLE_CP857)
                    .unwrap_or_else(|| panic!(
                        "{cp857_ref:?} (intended for {utf8_ref:?}) is not a valid cp857 bytes."
                    ))),
                utf8_ref
            );
        }
    }

    #[test]
    fn windows_codepages_coverage_test() {
        for cp in WINDOWS_USED_CODEPAGES {
            assert!(
                ENCODING_TABLE_CP_MAP.get(cp).is_some(),
                "Encoding table for cp{cp} is not defined",
            );
            assert!(
                DECODING_TABLE_CP_MAP.get(cp).is_some(),
                "Decoding table for cp{cp} is not defined",
            );
        }
    }

    /// Convert codepoint to Unicode via WindowsAPI
    ///
    /// # Arguments
    ///
    /// * `byte` - code point to convert to Unicode
    /// * `codepage` - code page
    #[cfg(windows)]
    fn windows_to_unicode_char(byte: u8, codepage: u16) -> Option<char> {
        let input_buf = [byte];
        let mut win_decode_buf: Vec<u16>;
        unsafe {
            use std::ptr::null_mut;
            use winapi::shared::winerror::ERROR_NO_UNICODE_TRANSLATION;
            use winapi::um::errhandlingapi::GetLastError;
            use winapi::um::stringapiset::MultiByteToWideChar;
            use winapi::um::winnls::MB_ERR_INVALID_CHARS;
            let win_decode_len = MultiByteToWideChar(
                codepage as u32,
                MB_ERR_INVALID_CHARS,
                input_buf.as_ptr() as *const i8,
                1,
                null_mut(),
                0,
            );
            if win_decode_len <= 0 {
                if GetLastError() == ERROR_NO_UNICODE_TRANSLATION {
                    return None;
                }
                panic!("MultiByteToWideChar (size checking) for 0x{byte:X} failed in cp{codepage}");
            }
            win_decode_buf = vec![0; win_decode_len as usize];
            let win_decode_status = MultiByteToWideChar(
                codepage as u32,
                MB_ERR_INVALID_CHARS,
                input_buf.as_ptr() as *const i8,
                1,
                win_decode_buf.as_mut_ptr(),
                win_decode_len,
            );
            assert_eq!(
                win_decode_status, win_decode_len,
                "MultiByteToWideChar (writing) failed for 0x{byte:X} in cp{codepage} (size checking returned {win_decode_len} / writing returned {win_decode_status})"
            );
        }
        let string_buf = String::from_utf16(&win_decode_buf).unwrap();
        if string_buf.chars().count() != 1 {
            return None;
        }
        return Some(string_buf.chars().next().unwrap());
    }

    #[cfg(windows)]
    fn get_formatted_error_message(error_code: u32) -> String {
        use core::ptr::null_mut;

        use winapi::um::winbase::{
            FormatMessageW, FORMAT_MESSAGE_FROM_SYSTEM, FORMAT_MESSAGE_IGNORE_INSERTS,
            FORMAT_MESSAGE_MAX_WIDTH_MASK,
        };
        use winapi::um::winnt::{LANG_ENGLISH, MAKELANGID, SUBLANG_ENGLISH_US};

        let mut local_error_message_buf = [0u16; 1024];
        let mut english_error_message_buf = [0u16; 1024];
        let local_error_message_len = unsafe {
            FormatMessageW(
                FORMAT_MESSAGE_FROM_SYSTEM
                    | FORMAT_MESSAGE_IGNORE_INSERTS
                    | FORMAT_MESSAGE_MAX_WIDTH_MASK,
                null_mut(),
                error_code,
                0,
                local_error_message_buf.as_mut_ptr(),
                local_error_message_buf.len() as u32,
                null_mut(),
            )
        };
        let english_error_message_len = unsafe {
            FormatMessageW(
                FORMAT_MESSAGE_FROM_SYSTEM
                    | FORMAT_MESSAGE_IGNORE_INSERTS
                    | FORMAT_MESSAGE_MAX_WIDTH_MASK,
                null_mut(),
                error_code,
                MAKELANGID(LANG_ENGLISH, SUBLANG_ENGLISH_US) as u32,
                english_error_message_buf.as_mut_ptr(),
                english_error_message_buf.len() as u32,
                null_mut(),
            )
        };
        assert!(local_error_message_len > 0);
        assert!(english_error_message_len > 0);
        let local_string =
            String::from_utf16_lossy(&local_error_message_buf[..local_error_message_len as usize])
                .trim_end()
                .to_string();
        let english_string = String::from_utf16_lossy(
            &english_error_message_buf[..english_error_message_len as usize],
        )
        .trim_end()
        .to_string();
        if local_string == english_string {
            format!("{local_string} [{error_code} (0x{error_code:X})]")
        } else {
            format!("{local_string} ({english_string}) [{error_code} (0x{error_code:X})]")
        }
    }

    /// Convert an Unicode character to codepoint via WindowsAPI
    ///
    /// # Arguments
    ///
    /// * `unicode` - Unicode character to convert to codepoint
    /// * `codepage` - code page
    /// * `strict` - whether to use WC_NO_BEST_FIT_CHARS or not.
    #[cfg(windows)]
    fn windows_to_codepage_char(unicode: char, codepage: u16, strict: bool) -> Option<Vec<u8>> {
        use alloc::borrow::Cow;
        use winapi::shared::minwindef::DWORD;

        let mut unicode_buf = [0u16; 2];
        let unicode_buf_slice = unicode.encode_utf16(&mut unicode_buf);
        unsafe {
            use std::ptr::null_mut;
            use winapi::um::errhandlingapi::GetLastError;
            use winapi::um::stringapiset::WideCharToMultiByte;
            use winapi::um::winnls::WC_NO_BEST_FIT_CHARS;

            let strict_flag: DWORD = if strict { WC_NO_BEST_FIT_CHARS } else { 0 };

            let mut has_invalid_chars = 0i32;
            let bytes_len = WideCharToMultiByte(
                codepage as u32,
                strict_flag, // We can't use WC_ERR_INVALID_CHARS here because it's dedicated to UTF-8 and GB18030
                unicode_buf_slice.as_ptr(),
                unicode_buf_slice.len() as i32,
                null_mut(),
                0,
                null_mut(),
                &mut has_invalid_chars,
            );
            if has_invalid_chars != 0 {
                return None;
            }
            if bytes_len <= 0 {
                let error_code = GetLastError();
                let error_message = get_formatted_error_message(error_code);
                panic!("WideCharToMultiByte (size checking) failed for {unicode} (U+{:04X}) in cp{codepage} (error: {error_message})", unicode as u32);
            }
            let mut bytes_buf = vec![0u8; bytes_len as usize];
            let written_bytes = WideCharToMultiByte(
                codepage as u32,
                strict_flag,
                unicode_buf_slice.as_ptr(),
                unicode_buf_slice.len() as i32,
                bytes_buf.as_mut_ptr() as *mut i8,
                bytes_len,
                null_mut(),
                null_mut(),
            );
            if written_bytes != bytes_len {
                let error_message: Cow<str> = if written_bytes == 0 {
                    Cow::from(format!(
                        " (error: {})",
                        get_formatted_error_message(GetLastError())
                    ))
                } else {
                    Cow::from("")
                };
                panic!("WideCharToMultiByte (writing) failed for {unicode} (U+{:04X}) in cp{codepage} (size checking returned {bytes_len} / writing returned {written_bytes}){error_message}", unicode as u32);
            }
            Some(bytes_buf)
        }
    }

    #[cfg(windows)]
    #[test]
    fn windows_to_unicode_char_test() {
        static WINDOWS_CONVERSION_VALID_TESTCASES: &[(u16, &[(u8, char)])] = &[
            (437, &[(0x82, 'é'), (0x9D, '¥'), (0xFB, '√')]),
            (850, &[(0xD0, 'ð'), (0xF3, '¾'), (0x9E, '×')]),
            (874, &[(0x80, '€'), (0xDF, '฿'), (0xA1, 'ก')]),
        ];
        static WINDOWS_CONVERSION_INVALID_TESTCASES: &[(u16, &[u8])] = &[
            (857, &[0xE7, 0xF2]),
            (874, &[0xDB, 0xDC, 0xDD, 0xDE, 0xFC, 0xFD, 0xFE, 0xFF]),
        ];
        use itertools::join;
        for &(codepage, testcases) in WINDOWS_CONVERSION_VALID_TESTCASES {
            let result = testcases
                .iter()
                .map(|(source, _)| windows_to_unicode_char(*source, codepage))
                .collect::<Vec<Option<char>>>();
            assert!(
                testcases
                    .iter()
                    .zip(result.iter())
                    .all(|((_, target), converted)| converted
                        .map(|c| c == *target)
                        .unwrap_or(false)),
                "failed in cp{}:\n{}",
                codepage,
                join(
                    testcases
                        .iter()
                        .zip(result.iter())
                        .filter(|((_, target), converted)| converted
                            .map(|c| c != *target)
                            .unwrap_or(true))
                        .map(|((from, target), converted)| format!(
                            "0x{from:X} => {target:?} (target) / {converted:?} (Windows)"
                        )),
                    ", "
                )
            );
        }
        for &(codepage, testcases) in WINDOWS_CONVERSION_INVALID_TESTCASES {
            let result = testcases
                .iter()
                .map(|source| windows_to_unicode_char(*source, codepage))
                .collect::<Vec<Option<char>>>();
            assert!(
                result.iter().all(|r| r.is_none()),
                "Some codepoints in cp{} weren't None: {}",
                codepage,
                join(
                    testcases
                        .iter()
                        .zip(result.iter())
                        .filter(|(_, r)| r.is_some())
                        .map(|(t, r)| format!("0x{:X} => {:?}", t, r.unwrap())),
                    ", "
                )
            );
        }
    }

    #[cfg(windows)]
    #[test]
    fn compare_to_winapi_decoding_test() {
        static WINDOWS_TESTING_CODEPAGES: &[(u16, Option<&[std::ops::Range<u8>]>)] = &[
            // Windows is the absolute reference because Unix-like OSes has already been migrated to UTF-8
            (437, None),
            (720, None),
            (737, None),
            (775, None),
            (850, None),
            (852, None),
            (855, None),
            (857, None),
            (858, None),
            (860, None),
            (861, None),
            (862, None),
            (863, None),
            (864, None),
            (865, None),
            (866, None),
            (869, None),
            (874, None),
        ];
        use std::borrow::Cow;
        let default_range = vec![(128..255).collect::<Vec<u8>>()];
        use itertools::join;
        for (codepage, testing_ranges) in WINDOWS_TESTING_CODEPAGES {
            let testing_ranges = testing_ranges
                .as_ref()
                .map(|v| {
                    Cow::from(
                        v.iter()
                            .map(|r| r.clone().collect::<Vec<u8>>())
                            .collect::<Vec<Vec<u8>>>(),
                    )
                })
                .unwrap_or(Cow::from(&default_range));
            for testing in testing_ranges.into_iter() {
                let msg = format!("Decoding table for cp{codepage} is not defined");
                let library_result = DECODING_TABLE_CP_MAP
                    .get(codepage)
                    .expect(&msg)
                    .decode_string_lossy(testing);
                let windows_result = testing
                    .iter()
                    .map(|codepoint| {
                        windows_to_unicode_char(*codepoint, *codepage)
                            .and_then(|ch| {
                                if 0xE000 <= ch as u32 && ch as u32 <= 0xF8FF {
                                    None
                                } else {
                                    Some(ch)
                                }
                            })
                            .unwrap_or('\u{FFFD}')
                    })
                    .collect::<String>();
                assert_eq!(
                    library_result,
                    windows_result,
                    "Different in cp{}:\n {}",
                    codepage,
                    join(
                        testing
                            .iter()
                            .zip(library_result.chars().zip(windows_result.chars()))
                            .filter(|(_, (l, w))| l != w)
                            .map(|(from, (lib, win))| format!(
                                "0x{:X} => {:?} (U+{:04X}) (library) != {:?} (U+{:04X}) (Windows)",
                                from, lib, lib as u32, win, win as u32
                            )),
                        "\n"
                    ),
                );
            }
        }
    }

    #[cfg(windows)]
    #[test]
    fn compare_to_winapi_encoding_test() {
        static WINDOWS_TESTING_CODEPAGES: &[u16] = &[
            437, 720, 737, 775, 850, 852, 855, 857, 858, 860, 861, 862, 863, 864, 865, 866, 869,
            874,
        ];

        use itertools::Itertools;
        for codepage in WINDOWS_TESTING_CODEPAGES {
            let table = ENCODING_TABLE_CP_MAP.get(codepage).unwrap();
            assert!(
                table.entries().all(|(unicode, table_result)| {
                    let windows_result = windows_to_codepage_char(*unicode, *codepage, true);
                    windows_result.is_some_and(|result| &result == &[*table_result])
                }),
                "Encoding result for cp{codepage} is incorrect:\n\n{}",
                table
                    .entries()
                    .filter_map(|(unicode, table_result)| {
                        let windows_result = windows_to_codepage_char(*unicode, *codepage, true);
                        if windows_result
                            .as_ref()
                            .is_some_and(|result| result == &[*table_result])
                        {
                            None
                        } else {
                            Some(format!(
                                "U+{:04X} => {:?} (Windows) != {:?} (library)",
                                *unicode as u32, &windows_result, table_result
                            ))
                        }
                    })
                    .join("\n")
            );
        }
    }
}
