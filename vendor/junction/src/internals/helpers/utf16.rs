/// Convert ASCII bytes to UTF-16 sequences.
pub const fn utf16s<const N: usize>(src: &'static [u8; N]) -> [u16; N] {
    let mut dst = [0u16; N];
    let mut i = 0;
    while i < N {
        dst[i] = src[i] as u16;
        i += 1;
    }
    dst
}
