use super::curve25519::{is_identity, sc_reduce, GeP2, GeP3};
use super::sha512;

static L: [u8; 32] = [
    0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x14, 0xde, 0xf9, 0xde, 0xa2, 0xf7, 0x9c, 0xd6, 0x58, 0x12, 0x63, 0x1a, 0x5c, 0xf5, 0xd3, 0xed,
];

fn check_s_lt_l(s: &[u8]) -> bool {
    let mut c: u8 = 0;
    let mut n: u8 = 1;

    let mut i = 31;
    loop {
        c |= ((((s[i] as i32) - (L[i] as i32)) >> 8) as u8) & n;
        n &= ((((s[i] ^ L[i]) as i32) - 1) >> 8) as u8;
        if i == 0 {
            break;
        } else {
            i -= 1;
        }
    }

    c == 0
}

pub fn verify(message: &[u8], public_key: &[u8], signature: &[u8]) -> bool {
    if check_s_lt_l(&signature[32..64]) || is_identity(public_key) {
        return false;
    }

    let a = match GeP3::from_bytes_negate_vartime(public_key) {
        Some(g) => g,
        None => {
            return false;
        }
    };
    if public_key.iter().fold(0, |acc, x| acc | x) == 0 {
        return false;
    }

    let mut hasher = sha512::Hash::new();
    hasher.update(&signature[0..32]);
    hasher.update(public_key);
    hasher.update(message);
    let mut hash = hasher.finalize();
    sc_reduce(&mut hash);

    let r = GeP2::double_scalarmult_vartime(hash.as_ref(), a, &signature[32..64]);
    r.to_bytes()
        .as_ref()
        .iter()
        .zip(signature.iter())
        .fold(0, |acc, (x, y)| acc | (x ^ y))
        == 0
}
