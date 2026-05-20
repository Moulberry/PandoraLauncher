use crate::LinkFlags;
use byteorder::{ByteOrder, LE};
use log::debug;

pub fn parse_string(data: &[u8], flags: LinkFlags) -> (usize, String) {
    let result = if !flags.contains(LinkFlags::IS_UNICODE) {
        let char_bytes = LE::read_u16(data) as usize;
        let total_bytes = 2 + char_bytes;
        let char_data = &data[2..total_bytes];
        // FIXME: Should be decoding with the system default encoding.
        //        This is effectively Latin-1, as that is the first 256 code points
        //        in Unicode.
        let mut s = String::new();
        s.reserve(char_bytes);
        for char in char_data {
            s.push(*char as char);
        }
        (total_bytes, s)
    } else {
        let char_count = LE::read_u16(data) as usize;
        let total_bytes = 2 + char_count * 2;
        let char_data = &data[2..total_bytes];
        let mut u16_chars = vec![0u16; char_count];
        LE::read_u16_into(char_data, &mut u16_chars);
        (total_bytes, String::from_utf16_lossy(&u16_chars))
    };
    debug!("Parsed string: {:?}", &result);
    result
}

pub fn to_data<S: Into<String>>(str_data: S, flags: LinkFlags) -> Vec<u8> {
    let s = str_data.into();
    if !flags.contains(LinkFlags::IS_UNICODE) {
        let mut bytes = vec![0u8; 2];
        for c in s.chars() {
            bytes.push(c as u8); // FIXME: clips non-Latin-1 characters!
        }
        let len = bytes.len() - 2;
        LE::write_u16(&mut bytes, len as u16); // writes u16 len at the start
        bytes
    } else {
        let utf16: Vec<u16> = s.encode_utf16().collect();
        let mut bytes = vec![0u8; 2 + utf16.len() * 2];
        LE::write_u16(&mut bytes, utf16.len() as u16);
        LE::write_u16_into(&utf16, &mut bytes[2..]);
        bytes
    }
}
