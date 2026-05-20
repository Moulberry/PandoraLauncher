#![cfg(not(miri))]

use core::mem::size_of_val;
use core::slice::from_raw_parts_mut;

/// Misaligns the slice by one byte, to ensure no SIMD load instructions require alignment.
fn misalign_slice(buf: &mut [u128]) -> &mut [u8] {
    let ptr = buf.as_mut_ptr() as *mut u8;
    let len = size_of_val(buf);
    unsafe { from_raw_parts_mut(ptr.add(1), len - 1) }
}

/// Confirms that all bit positions are being used for comparison, for a given length.
fn test_one_length<CTEQ>(a: &mut [u8], b: &mut [u8], n: usize, cteq: &CTEQ)
where
    CTEQ: Fn(&[u8], &[u8]) -> bool,
{
    let a = &mut a[..n];
    let b = &mut b[..n];

    assert!(cteq(a, b));
    for i in 0..n {
        for m in [0x01, 0x02, 0x04, 0x08, 0x10, 0x20, 0x40, 0x80] {
            a[i] ^= m;
            assert!(!cteq(a, b), "len={} a[{}] mask 0x{:02x}", n, i, m);
            a[i] ^= m;

            b[i] ^= m;
            assert!(!cteq(a, b), "len={} b[{}] mask 0x{:02x}", n, i, m);
            b[i] ^= m;
        }
    }
    assert!(cteq(a, b));
}

/// Confirms that all bit positions are being used for comparison, for all lengths up to 1024 bits.
fn test_all_lengths<F: FnOnce(&mut [u8]), CTEQ>(fill: F, cteq: &CTEQ)
where
    CTEQ: Fn(&[u8], &[u8]) -> bool,
{
    let mut a = [0u128; 9];
    let mut b = [0u128; 9];

    let a = misalign_slice(&mut a);
    let b = misalign_slice(&mut b);

    fill(a);
    b.copy_from_slice(a);

    // Note: this is quadratic; do not increase the maximum length too much.
    for n in 0..=128 {
        test_one_length(a, b, n, cteq);
    }
}

fn exhaustive_test_zeros<CTEQ>(cteq: &CTEQ)
where
    CTEQ: Fn(&[u8], &[u8]) -> bool,
{
    test_all_lengths(|buf| buf.fill(0), cteq);
}

fn exhaustive_test_ones<CTEQ>(cteq: &CTEQ)
where
    CTEQ: Fn(&[u8], &[u8]) -> bool,
{
    test_all_lengths(|buf| buf.fill(!0), cteq);
}

fn exhaustive_test_random<CTEQ>(cteq: &CTEQ)
where
    CTEQ: Fn(&[u8], &[u8]) -> bool,
{
    // Simple xorshift PRNG, from https://www.jstatsoft.org/article/view/v008i14
    let mut state: u32 = 2463534242;
    let xorshift32 = || {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        state as u8
    };

    test_all_lengths(|buf| buf.fill_with(xorshift32), cteq);
}

#[test]
fn exhaustive_test_zeros_simd() {
    use constant_time_eq::constant_time_eq;
    exhaustive_test_zeros(&constant_time_eq);
}

#[test]
fn exhaustive_test_ones_simd() {
    use constant_time_eq::constant_time_eq;
    exhaustive_test_ones(&constant_time_eq);
}

#[test]
fn exhaustive_test_random_simd() {
    use constant_time_eq::constant_time_eq;
    exhaustive_test_random(&constant_time_eq);
}

#[test]
fn exhaustive_test_zeros_generic() {
    use constant_time_eq::generic::constant_time_eq;
    exhaustive_test_zeros(&constant_time_eq);
}

#[test]
fn exhaustive_test_ones_generic() {
    use constant_time_eq::generic::constant_time_eq;
    exhaustive_test_ones(&constant_time_eq);
}

#[test]
fn exhaustive_test_random_generic() {
    use constant_time_eq::generic::constant_time_eq;
    exhaustive_test_random(&constant_time_eq);
}
