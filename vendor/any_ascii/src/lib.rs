//! Unicode to ASCII transliteration.
//!
//! Converts Unicode text to a reasonable representation using only ASCII.
//!
//! For most characters in Unicode, AnyAscii provides an ASCII-only replacement string.
//! Text is converted character-by-character without considering the context.
//! The mappings for each script are based on popular existing romanization schemes.
//! Symbolic characters are converted based on their meaning or appearance.
//! All ASCII characters in the input are left unchanged,
//! every other character is replaced with printable ASCII characters.
//! Unknown characters are removed.
//!
//! This crate supports `no_std` + `alloc`.

#![no_std]

extern crate alloc;
use alloc::string::String;

mod block;

/// Transliterates a Unicode String into ASCII.
///
/// ```
/// # use any_ascii::any_ascii;
/// assert_eq!("anthropoi", any_ascii("άνθρωποι"));
/// assert_eq!("sample", any_ascii("sample"));
/// assert_eq!("ShenZhen", any_ascii("深圳"));
/// assert_eq!("Boris", any_ascii("Борис"));
/// assert_eq!("toyota", any_ascii("トヨタ"));
/// ```
pub fn any_ascii(s: &str) -> String {
    let mut r = String::with_capacity(s.len() / 2);
    for c in s.chars() {
        if c.is_ascii() {
            r.push(c);
        } else {
            r.push_str(any_ascii_char(c));
        }
    }
    r
}

/// Transliterates a Unicode char into ASCII.
///
/// ```
/// # use any_ascii::any_ascii_char;
/// assert_eq!("ae", any_ascii_char('æ'));
/// assert_eq!("e", any_ascii_char('é'));
/// assert_eq!("k", any_ascii_char('k'));
/// assert_eq!("ss", any_ascii_char('ß'));
/// assert_eq!("Shen", any_ascii_char('深'));
/// assert_eq!("c", any_ascii_char('ç'));
/// assert_eq!("l", any_ascii_char('λ'));
/// assert_eq!("zh", any_ascii_char('ж'));
/// assert_eq!(":crown:", any_ascii_char('👑'));
/// assert_eq!("#", any_ascii_char('♯'));
/// ```
pub fn any_ascii_char(c: char) -> &'static str {
    let block_num = ((c as u32) >> 8) as u16;
    let block_bytes = block::block(block_num);
    let block: &'static [[u8; 3]] = unsafe {
        core::slice::from_raw_parts(
            block_bytes.as_ptr() as *const [u8; 3],
            block_bytes.len() / 3
        )
    };
    let lo = (c as u8) as usize;
    if let Some(ptr) = block.get(lo) {
        let l = ptr[2];
        let len = if (l & 0x80) == 0 { 3 } else { (l & 0x7f) as usize };
        if len <= 3 {
            let ascii_bytes = &ptr[..len];
            unsafe { core::str::from_utf8_unchecked(ascii_bytes) }
        } else {
            let i = ((u16::from(ptr[0]) << 8) | u16::from(ptr[1])) as usize;
            let bank = include_str!("bank.txt");
            unsafe { bank.get_unchecked(i..i + len) }
        }
    } else {
        ""
    }
}

#[test]
fn test() {
    fn check(s: &str, expected: &str) {
        assert_eq!(any_ascii(s), expected);
    }

    check("", "");
    check("\x00\x01\t\n\x1f ~\x7f", "\x00\x01\t\n\x1f ~\x7f");
    check("sample", "sample");
    check("\u{e000}", "");
    check("\u{fdff}", "");
    check("\u{0080}", "");
    check("\u{00ff}", "y");

    check("René François Lacôte", "Rene Francois Lacote");
    check("Blöße", "Blosse");
    check("Trần Hưng Đạo", "Tran Hung Dao");
    check("Nærøy", "Naeroy");
    check("Φειδιππίδης", "Feidippidis");
    check("Δημήτρης Φωτόπουλος", "Dimitris Fotopoylos");
    check("Борис Николаевич Ельцин", "Boris Nikolaevich El'tsin");
    check("Володимир Горбулін", "Volodimir Gorbulin");
    check("Търговище", "T'rgovishche");
    check("深圳", "ShenZhen");
    check("深水埗", "ShenShuiBu");
    check("화성시", "HwaSeongSi");
    check("華城市", "HuaChengShi");
    check("さいたま", "saitama");
    check("埼玉県", "QiYuXian");
    check("ደብረ ዘይት", "debre zeyt");
    check("ደቀምሓረ", "dek'emhare");
    check("دمنهور", "dmnhwr");
    check("Աբովյան", "Abovyan");
    check("სამტრედია", "samt'redia");
    check("אברהם הלוי פרנקל", "'vrhm hlvy frnkl");
    check("⠠⠎⠁⠽⠀⠭⠀⠁⠛", "+say x ag");
    check("ময়মনসিংহ", "mymnsimh");
    check("ထန်တလန်", "thntln");
    check("પોરબંદર", "porbmdr");
    check("महासमुंद", "mhasmumd");
    check("ಬೆಂಗಳೂರು", "bemgluru");
    check("សៀមរាប", "siemrab");
    check("ສະຫວັນນະເຂດ", "sahvannaekhd");
    check("കളമശ്ശേരി", "klmsseri");
    check("ଗଜପତି", "gjpti");
    check("ਜਲੰਧਰ", "jlmdhr");
    check("රත්නපුර", "rtnpur");
    check("கன்னியாகுமரி", "knniyakumri");
    check("శ్రీకాకుళం", "srikakulm");
    check("สงขลา", "sngkhla");
    check("😎 👑 🍎", ":sunglasses: :crown: :apple:");
    check("☆ ♯ ♰ ⚄ ⛌", "* # + 5 X");
    check("№ ℳ ⅋ ⅍", "No M & A/S");

    check("トヨタ", "toyota");
    check("ߞߐߣߊߞߙߌ߫", "konakri");
}
