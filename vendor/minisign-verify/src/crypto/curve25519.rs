#![allow(unused_parens)]
#![allow(non_camel_case_types)]

use std::cmp::{min, Eq, PartialEq};
use std::ops::{Add, Mul, Sub};

pub type fiat_25519_u1 = u8;
pub type fiat_25519_i1 = i8;
pub type fiat_25519_i2 = i8;

#[inline]
pub fn fiat_25519_addcarryx_u51(
    out1: &mut u64,
    out2: &mut fiat_25519_u1,
    arg1: fiat_25519_u1,
    arg2: u64,
    arg3: u64,
) {
    let x1: u64 = (((arg1 as u64) + arg2) + arg3);
    let x2: u64 = (x1 & 0x7ffffffffffff);
    let x3: fiat_25519_u1 = ((x1 >> 51) as fiat_25519_u1);
    *out1 = x2;
    *out2 = x3;
}

#[inline]
pub fn fiat_25519_subborrowx_u51(
    out1: &mut u64,
    out2: &mut fiat_25519_u1,
    arg1: fiat_25519_u1,
    arg2: u64,
    arg3: u64,
) {
    let x1: i64 = ((((((arg2 as i128) - (arg1 as i128)) as i64) as i128) - (arg3 as i128)) as i64);
    let x2: fiat_25519_i1 = ((x1 >> 51) as fiat_25519_i1);
    let x3: u64 = (((x1 as i128) & 0x7ffffffffffff_i128) as u64);
    *out1 = x3;
    *out2 = ((0x0_i8 - (x2 as fiat_25519_i2)) as fiat_25519_u1);
}

#[inline]
pub fn fiat_25519_cmovznz_u64(out1: &mut u64, arg1: fiat_25519_u1, arg2: u64, arg3: u64) {
    let x1: fiat_25519_u1 = (!(!arg1));
    let x2: u64 = (((((0x0_i8 - (x1 as fiat_25519_i2)) as fiat_25519_i1) as i128)
        & 0xffffffffffffffff_i128) as u64);
    let x3: u64 = ((x2 & arg3) | ((!x2) & arg2));
    *out1 = x3;
}

#[inline]
pub fn fiat_25519_carry_mul(out1: &mut [u64; 5], arg1: &[u64; 5], arg2: &[u64; 5]) {
    let x1: u128 = (((arg1[4]) as u128) * (((arg2[4]) * 0x13) as u128));
    let x2: u128 = (((arg1[4]) as u128) * (((arg2[3]) * 0x13) as u128));
    let x3: u128 = (((arg1[4]) as u128) * (((arg2[2]) * 0x13) as u128));
    let x4: u128 = (((arg1[4]) as u128) * (((arg2[1]) * 0x13) as u128));
    let x5: u128 = (((arg1[3]) as u128) * (((arg2[4]) * 0x13) as u128));
    let x6: u128 = (((arg1[3]) as u128) * (((arg2[3]) * 0x13) as u128));
    let x7: u128 = (((arg1[3]) as u128) * (((arg2[2]) * 0x13) as u128));
    let x8: u128 = (((arg1[2]) as u128) * (((arg2[4]) * 0x13) as u128));
    let x9: u128 = (((arg1[2]) as u128) * (((arg2[3]) * 0x13) as u128));
    let x10: u128 = (((arg1[1]) as u128) * (((arg2[4]) * 0x13) as u128));
    let x11: u128 = (((arg1[4]) as u128) * ((arg2[0]) as u128));
    let x12: u128 = (((arg1[3]) as u128) * ((arg2[1]) as u128));
    let x13: u128 = (((arg1[3]) as u128) * ((arg2[0]) as u128));
    let x14: u128 = (((arg1[2]) as u128) * ((arg2[2]) as u128));
    let x15: u128 = (((arg1[2]) as u128) * ((arg2[1]) as u128));
    let x16: u128 = (((arg1[2]) as u128) * ((arg2[0]) as u128));
    let x17: u128 = (((arg1[1]) as u128) * ((arg2[3]) as u128));
    let x18: u128 = (((arg1[1]) as u128) * ((arg2[2]) as u128));
    let x19: u128 = (((arg1[1]) as u128) * ((arg2[1]) as u128));
    let x20: u128 = (((arg1[1]) as u128) * ((arg2[0]) as u128));
    let x21: u128 = (((arg1[0]) as u128) * ((arg2[4]) as u128));
    let x22: u128 = (((arg1[0]) as u128) * ((arg2[3]) as u128));
    let x23: u128 = (((arg1[0]) as u128) * ((arg2[2]) as u128));
    let x24: u128 = (((arg1[0]) as u128) * ((arg2[1]) as u128));
    let x25: u128 = (((arg1[0]) as u128) * ((arg2[0]) as u128));
    let x26: u128 = (x25 + (x10 + (x9 + (x7 + x4))));
    let x27: u64 = ((x26 >> 51) as u64);
    let x28: u64 = ((x26 & 0x7ffffffffffff_u128) as u64);
    let x29: u128 = (x21 + (x17 + (x14 + (x12 + x11))));
    let x30: u128 = (x22 + (x18 + (x15 + (x13 + x1))));
    let x31: u128 = (x23 + (x19 + (x16 + (x5 + x2))));
    let x32: u128 = (x24 + (x20 + (x8 + (x6 + x3))));
    let x33: u128 = ((x27 as u128) + x32);
    let x34: u64 = ((x33 >> 51) as u64);
    let x35: u64 = ((x33 & 0x7ffffffffffff_u128) as u64);
    let x36: u128 = ((x34 as u128) + x31);
    let x37: u64 = ((x36 >> 51) as u64);
    let x38: u64 = ((x36 & 0x7ffffffffffff_u128) as u64);
    let x39: u128 = ((x37 as u128) + x30);
    let x40: u64 = ((x39 >> 51) as u64);
    let x41: u64 = ((x39 & 0x7ffffffffffff_u128) as u64);
    let x42: u128 = ((x40 as u128) + x29);
    let x43: u64 = ((x42 >> 51) as u64);
    let x44: u64 = ((x42 & 0x7ffffffffffff_u128) as u64);
    let x45: u64 = (x43 * 0x13);
    let x46: u64 = (x28 + x45);
    let x47: u64 = (x46 >> 51);
    let x48: u64 = (x46 & 0x7ffffffffffff);
    let x49: u64 = (x47 + x35);
    let x50: fiat_25519_u1 = ((x49 >> 51) as fiat_25519_u1);
    let x51: u64 = (x49 & 0x7ffffffffffff);
    let x52: u64 = ((x50 as u64) + x38);
    out1[0] = x48;
    out1[1] = x51;
    out1[2] = x52;
    out1[3] = x41;
    out1[4] = x44;
}

#[inline]
pub fn fiat_25519_carry_square(out1: &mut [u64; 5], arg1: &[u64; 5]) {
    let x1: u64 = ((arg1[4]) * 0x13);
    let x2: u64 = (x1 * 0x2);
    let x3: u64 = ((arg1[4]) * 0x2);
    let x4: u64 = ((arg1[3]) * 0x13);
    let x5: u64 = (x4 * 0x2);
    let x6: u64 = ((arg1[3]) * 0x2);
    let x7: u64 = ((arg1[2]) * 0x2);
    let x8: u64 = ((arg1[1]) * 0x2);
    let x9: u128 = (((arg1[4]) as u128) * (x1 as u128));
    let x10: u128 = (((arg1[3]) as u128) * (x2 as u128));
    let x11: u128 = (((arg1[3]) as u128) * (x4 as u128));
    let x12: u128 = (((arg1[2]) as u128) * (x2 as u128));
    let x13: u128 = (((arg1[2]) as u128) * (x5 as u128));
    let x14: u128 = (((arg1[2]) as u128) * ((arg1[2]) as u128));
    let x15: u128 = (((arg1[1]) as u128) * (x2 as u128));
    let x16: u128 = (((arg1[1]) as u128) * (x6 as u128));
    let x17: u128 = (((arg1[1]) as u128) * (x7 as u128));
    let x18: u128 = (((arg1[1]) as u128) * ((arg1[1]) as u128));
    let x19: u128 = (((arg1[0]) as u128) * (x3 as u128));
    let x20: u128 = (((arg1[0]) as u128) * (x6 as u128));
    let x21: u128 = (((arg1[0]) as u128) * (x7 as u128));
    let x22: u128 = (((arg1[0]) as u128) * (x8 as u128));
    let x23: u128 = (((arg1[0]) as u128) * ((arg1[0]) as u128));
    let x24: u128 = (x23 + (x15 + x13));
    let x25: u64 = ((x24 >> 51) as u64);
    let x26: u64 = ((x24 & 0x7ffffffffffff_u128) as u64);
    let x27: u128 = (x19 + (x16 + x14));
    let x28: u128 = (x20 + (x17 + x9));
    let x29: u128 = (x21 + (x18 + x10));
    let x30: u128 = (x22 + (x12 + x11));
    let x31: u128 = ((x25 as u128) + x30);
    let x32: u64 = ((x31 >> 51) as u64);
    let x33: u64 = ((x31 & 0x7ffffffffffff_u128) as u64);
    let x34: u128 = ((x32 as u128) + x29);
    let x35: u64 = ((x34 >> 51) as u64);
    let x36: u64 = ((x34 & 0x7ffffffffffff_u128) as u64);
    let x37: u128 = ((x35 as u128) + x28);
    let x38: u64 = ((x37 >> 51) as u64);
    let x39: u64 = ((x37 & 0x7ffffffffffff_u128) as u64);
    let x40: u128 = ((x38 as u128) + x27);
    let x41: u64 = ((x40 >> 51) as u64);
    let x42: u64 = ((x40 & 0x7ffffffffffff_u128) as u64);
    let x43: u64 = (x41 * 0x13);
    let x44: u64 = (x26 + x43);
    let x45: u64 = (x44 >> 51);
    let x46: u64 = (x44 & 0x7ffffffffffff);
    let x47: u64 = (x45 + x33);
    let x48: fiat_25519_u1 = ((x47 >> 51) as fiat_25519_u1);
    let x49: u64 = (x47 & 0x7ffffffffffff);
    let x50: u64 = ((x48 as u64) + x36);
    out1[0] = x46;
    out1[1] = x49;
    out1[2] = x50;
    out1[3] = x39;
    out1[4] = x42;
}

#[inline]
pub fn fiat_25519_carry(out1: &mut [u64; 5], arg1: &[u64; 5]) {
    let x1: u64 = (arg1[0]);
    let x2: u64 = ((x1 >> 51) + (arg1[1]));
    let x3: u64 = ((x2 >> 51) + (arg1[2]));
    let x4: u64 = ((x3 >> 51) + (arg1[3]));
    let x5: u64 = ((x4 >> 51) + (arg1[4]));
    let x6: u64 = ((x1 & 0x7ffffffffffff) + ((x5 >> 51) * 0x13));
    let x7: u64 = ((((x6 >> 51) as fiat_25519_u1) as u64) + (x2 & 0x7ffffffffffff));
    let x8: u64 = (x6 & 0x7ffffffffffff);
    let x9: u64 = (x7 & 0x7ffffffffffff);
    let x10: u64 = ((((x7 >> 51) as fiat_25519_u1) as u64) + (x3 & 0x7ffffffffffff));
    let x11: u64 = (x4 & 0x7ffffffffffff);
    let x12: u64 = (x5 & 0x7ffffffffffff);
    out1[0] = x8;
    out1[1] = x9;
    out1[2] = x10;
    out1[3] = x11;
    out1[4] = x12;
}

#[inline]
pub fn fiat_25519_add(out1: &mut [u64; 5], arg1: &[u64; 5], arg2: &[u64; 5]) {
    let x1: u64 = ((arg1[0]) + (arg2[0]));
    let x2: u64 = ((arg1[1]) + (arg2[1]));
    let x3: u64 = ((arg1[2]) + (arg2[2]));
    let x4: u64 = ((arg1[3]) + (arg2[3]));
    let x5: u64 = ((arg1[4]) + (arg2[4]));
    out1[0] = x1;
    out1[1] = x2;
    out1[2] = x3;
    out1[3] = x4;
    out1[4] = x5;
}

#[inline]
pub fn fiat_25519_sub(out1: &mut [u64; 5], arg1: &[u64; 5], arg2: &[u64; 5]) {
    let x1: u64 = ((0xfffffffffffda + (arg1[0])) - (arg2[0]));
    let x2: u64 = ((0xffffffffffffe + (arg1[1])) - (arg2[1]));
    let x3: u64 = ((0xffffffffffffe + (arg1[2])) - (arg2[2]));
    let x4: u64 = ((0xffffffffffffe + (arg1[3])) - (arg2[3]));
    let x5: u64 = ((0xffffffffffffe + (arg1[4])) - (arg2[4]));
    out1[0] = x1;
    out1[1] = x2;
    out1[2] = x3;
    out1[3] = x4;
    out1[4] = x5;
}

#[inline]
pub fn fiat_25519_opp(out1: &mut [u64; 5], arg1: &[u64; 5]) {
    let x1: u64 = (0xfffffffffffda - (arg1[0]));
    let x2: u64 = (0xffffffffffffe - (arg1[1]));
    let x3: u64 = (0xffffffffffffe - (arg1[2]));
    let x4: u64 = (0xffffffffffffe - (arg1[3]));
    let x5: u64 = (0xffffffffffffe - (arg1[4]));
    out1[0] = x1;
    out1[1] = x2;
    out1[2] = x3;
    out1[3] = x4;
    out1[4] = x5;
}

#[inline]
pub fn fiat_25519_to_bytes(out1: &mut [u8; 32], arg1: &[u64; 5]) {
    let mut x1: u64 = 0;
    let mut x2: fiat_25519_u1 = 0;
    fiat_25519_subborrowx_u51(&mut x1, &mut x2, 0x0, (arg1[0]), 0x7ffffffffffed);
    let mut x3: u64 = 0;
    let mut x4: fiat_25519_u1 = 0;
    fiat_25519_subborrowx_u51(&mut x3, &mut x4, x2, (arg1[1]), 0x7ffffffffffff);
    let mut x5: u64 = 0;
    let mut x6: fiat_25519_u1 = 0;
    fiat_25519_subborrowx_u51(&mut x5, &mut x6, x4, (arg1[2]), 0x7ffffffffffff);
    let mut x7: u64 = 0;
    let mut x8: fiat_25519_u1 = 0;
    fiat_25519_subborrowx_u51(&mut x7, &mut x8, x6, (arg1[3]), 0x7ffffffffffff);
    let mut x9: u64 = 0;
    let mut x10: fiat_25519_u1 = 0;
    fiat_25519_subborrowx_u51(&mut x9, &mut x10, x8, (arg1[4]), 0x7ffffffffffff);
    let mut x11: u64 = 0;
    fiat_25519_cmovznz_u64(&mut x11, x10, 0x0_u64, 0xffffffffffffffff);
    let mut x12: u64 = 0;
    let mut x13: fiat_25519_u1 = 0;
    fiat_25519_addcarryx_u51(&mut x12, &mut x13, 0x0, x1, (x11 & 0x7ffffffffffed));
    let mut x14: u64 = 0;
    let mut x15: fiat_25519_u1 = 0;
    fiat_25519_addcarryx_u51(&mut x14, &mut x15, x13, x3, (x11 & 0x7ffffffffffff));
    let mut x16: u64 = 0;
    let mut x17: fiat_25519_u1 = 0;
    fiat_25519_addcarryx_u51(&mut x16, &mut x17, x15, x5, (x11 & 0x7ffffffffffff));
    let mut x18: u64 = 0;
    let mut x19: fiat_25519_u1 = 0;
    fiat_25519_addcarryx_u51(&mut x18, &mut x19, x17, x7, (x11 & 0x7ffffffffffff));
    let mut x20: u64 = 0;
    let mut x21: fiat_25519_u1 = 0;
    fiat_25519_addcarryx_u51(&mut x20, &mut x21, x19, x9, (x11 & 0x7ffffffffffff));
    let x22: u64 = (x20 << 4);
    let x23: u64 = (x18 * 0x2_u64);
    let x24: u64 = (x16 << 6);
    let x25: u64 = (x14 << 3);
    let x26: u64 = (x12 >> 8);
    let x27: u8 = ((x12 & 0xff_u64) as u8);
    let x28: u64 = (x26 >> 8);
    let x29: u8 = ((x26 & 0xff_u64) as u8);
    let x30: u64 = (x28 >> 8);
    let x31: u8 = ((x28 & 0xff_u64) as u8);
    let x32: u64 = (x30 >> 8);
    let x33: u8 = ((x30 & 0xff_u64) as u8);
    let x34: u64 = (x32 >> 8);
    let x35: u8 = ((x32 & 0xff_u64) as u8);
    let x36: u8 = ((x34 >> 8) as u8);
    let x37: u8 = ((x34 & 0xff_u64) as u8);
    let x38: u64 = ((x36 as u64) + x25);
    let x39: u64 = (x38 >> 8);
    let x40: u8 = ((x38 & 0xff_u64) as u8);
    let x41: u64 = (x39 >> 8);
    let x42: u8 = ((x39 & 0xff_u64) as u8);
    let x43: u64 = (x41 >> 8);
    let x44: u8 = ((x41 & 0xff_u64) as u8);
    let x45: u64 = (x43 >> 8);
    let x46: u8 = ((x43 & 0xff_u64) as u8);
    let x47: u64 = (x45 >> 8);
    let x48: u8 = ((x45 & 0xff_u64) as u8);
    let x49: u8 = ((x47 >> 8) as u8);
    let x50: u8 = ((x47 & 0xff_u64) as u8);
    let x51: u64 = ((x49 as u64) + x24);
    let x52: u64 = (x51 >> 8);
    let x53: u8 = ((x51 & 0xff_u64) as u8);
    let x54: u64 = (x52 >> 8);
    let x55: u8 = ((x52 & 0xff_u64) as u8);
    let x56: u64 = (x54 >> 8);
    let x57: u8 = ((x54 & 0xff_u64) as u8);
    let x58: u64 = (x56 >> 8);
    let x59: u8 = ((x56 & 0xff_u64) as u8);
    let x60: u64 = (x58 >> 8);
    let x61: u8 = ((x58 & 0xff_u64) as u8);
    let x62: u64 = (x60 >> 8);
    let x63: u8 = ((x60 & 0xff_u64) as u8);
    let x64: fiat_25519_u1 = ((x62 >> 8) as fiat_25519_u1);
    let x65: u8 = ((x62 & 0xff_u64) as u8);
    let x66: u64 = ((x64 as u64) + x23);
    let x67: u64 = (x66 >> 8);
    let x68: u8 = ((x66 & 0xff_u64) as u8);
    let x69: u64 = (x67 >> 8);
    let x70: u8 = ((x67 & 0xff_u64) as u8);
    let x71: u64 = (x69 >> 8);
    let x72: u8 = ((x69 & 0xff_u64) as u8);
    let x73: u64 = (x71 >> 8);
    let x74: u8 = ((x71 & 0xff_u64) as u8);
    let x75: u64 = (x73 >> 8);
    let x76: u8 = ((x73 & 0xff_u64) as u8);
    let x77: u8 = ((x75 >> 8) as u8);
    let x78: u8 = ((x75 & 0xff_u64) as u8);
    let x79: u64 = ((x77 as u64) + x22);
    let x80: u64 = (x79 >> 8);
    let x81: u8 = ((x79 & 0xff_u64) as u8);
    let x82: u64 = (x80 >> 8);
    let x83: u8 = ((x80 & 0xff_u64) as u8);
    let x84: u64 = (x82 >> 8);
    let x85: u8 = ((x82 & 0xff_u64) as u8);
    let x86: u64 = (x84 >> 8);
    let x87: u8 = ((x84 & 0xff_u64) as u8);
    let x88: u64 = (x86 >> 8);
    let x89: u8 = ((x86 & 0xff_u64) as u8);
    let x90: u8 = ((x88 >> 8) as u8);
    let x91: u8 = ((x88 & 0xff_u64) as u8);
    out1[0] = x27;
    out1[1] = x29;
    out1[2] = x31;
    out1[3] = x33;
    out1[4] = x35;
    out1[5] = x37;
    out1[6] = x40;
    out1[7] = x42;
    out1[8] = x44;
    out1[9] = x46;
    out1[10] = x48;
    out1[11] = x50;
    out1[12] = x53;
    out1[13] = x55;
    out1[14] = x57;
    out1[15] = x59;
    out1[16] = x61;
    out1[17] = x63;
    out1[18] = x65;
    out1[19] = x68;
    out1[20] = x70;
    out1[21] = x72;
    out1[22] = x74;
    out1[23] = x76;
    out1[24] = x78;
    out1[25] = x81;
    out1[26] = x83;
    out1[27] = x85;
    out1[28] = x87;
    out1[29] = x89;
    out1[30] = x91;
    out1[31] = x90;
}

#[derive(Clone, Default, Copy)]
pub struct Fe(pub [u64; 5]);

impl PartialEq for Fe {
    fn eq(&self, other: &Fe) -> bool {
        let &Fe(self_elems) = self;
        let &Fe(other_elems) = other;
        self_elems == other_elems
    }
}
impl Eq for Fe {}

static FE_ZERO: Fe = Fe([0, 0, 0, 0, 0]);
static FE_ONE: Fe = Fe([1, 0, 0, 0, 0]);
static FE_SQRTM1: Fe = Fe([
    1718705420411056,
    234908883556509,
    2233514472574048,
    2117202627021982,
    765476049583133,
]);
static FE_D: Fe = Fe([
    929955233495203,
    466365720129213,
    1662059464998953,
    2033849074728123,
    1442794654840575,
]);
static FE_D2: Fe = Fe([
    1859910466990425,
    932731440258426,
    1072319116312658,
    1815898335770999,
    633789495995903,
]);

#[inline]
fn load_8u(s: &[u8]) -> u64 {
    (s[0] as u64)
        | ((s[1] as u64) << 8)
        | ((s[2] as u64) << 16)
        | ((s[3] as u64) << 24)
        | ((s[4] as u64) << 32)
        | ((s[5] as u64) << 40)
        | ((s[6] as u64) << 48)
        | ((s[7] as u64) << 56)
}

#[inline]
fn load_4u(s: &[u8]) -> u64 {
    (s[0] as u64) | ((s[1] as u64) << 8) | ((s[2] as u64) << 16) | ((s[3] as u64) << 24)
}

#[inline]
fn load_4i(s: &[u8]) -> i64 {
    load_4u(s) as i64
}

#[inline]
fn load_3u(s: &[u8]) -> u64 {
    (s[0] as u64) | ((s[1] as u64) << 8) | ((s[2] as u64) << 16)
}

#[inline]
fn load_3i(s: &[u8]) -> i64 {
    load_3u(s) as i64
}

impl Add for Fe {
    type Output = Fe;

    fn add(self, _rhs: Fe) -> Fe {
        let Fe(f) = self;
        let Fe(g) = _rhs;
        let mut h = Fe::default();
        fiat_25519_add(&mut h.0, &f, &g);
        h
    }
}

impl Sub for Fe {
    type Output = Fe;

    fn sub(self, _rhs: Fe) -> Fe {
        let Fe(f) = self;
        let Fe(g) = _rhs;
        let mut h = Fe::default();
        fiat_25519_sub(&mut h.0, &f, &g);
        h.carry()
    }
}

impl Mul for Fe {
    type Output = Fe;

    fn mul(self, _rhs: Fe) -> Fe {
        let Fe(f) = self;
        let Fe(g) = _rhs;
        let mut h = Fe::default();
        fiat_25519_carry_mul(&mut h.0, &f, &g);
        h
    }
}

impl Fe {
    pub fn from_bytes(s: &[u8]) -> Fe {
        if s.len() != 32 {
            panic!("Invalid compressed length")
        }
        let mut h = Fe::default();
        let mask = 0x7ffffffffffff;
        h.0[0] = load_8u(&s[0..]) & mask;
        h.0[1] = (load_8u(&s[6..]) >> 3) & mask;
        h.0[2] = (load_8u(&s[12..]) >> 6) & mask;
        h.0[3] = (load_8u(&s[19..]) >> 1) & mask;
        h.0[4] = (load_8u(&s[24..]) >> 12) & mask;
        h
    }

    pub fn to_bytes(&self) -> [u8; 32] {
        let &Fe(es) = &self.carry();
        let mut s_ = [0u8; 32];
        fiat_25519_to_bytes(&mut s_, &es);
        s_
    }

    pub fn carry(&self) -> Fe {
        let mut h = Fe::default();
        fiat_25519_carry(&mut h.0, &self.0);
        h
    }

    fn square(&self) -> Fe {
        let &Fe(f) = &self;
        let mut h = Fe::default();
        fiat_25519_carry_square(&mut h.0, f);
        h
    }

    fn square_and_double(&self) -> Fe {
        let h = self.square();
        (h + h)
    }

    pub fn invert(&self) -> Fe {
        let z1 = *self;
        let z2 = z1.square();
        let z8 = z2.square().square();
        let z9 = z1 * z8;
        let z11 = z2 * z9;
        let z22 = z11.square();
        let z_5_0 = z9 * z22;
        let z_10_5 = (0..5).fold(z_5_0, |z_5_n, _| z_5_n.square());
        let z_10_0 = z_10_5 * z_5_0;
        let z_20_10 = (0..10).fold(z_10_0, |x, _| x.square());
        let z_20_0 = z_20_10 * z_10_0;
        let z_40_20 = (0..20).fold(z_20_0, |x, _| x.square());
        let z_40_0 = z_40_20 * z_20_0;
        let z_50_10 = (0..10).fold(z_40_0, |x, _| x.square());
        let z_50_0 = z_50_10 * z_10_0;
        let z_100_50 = (0..50).fold(z_50_0, |x, _| x.square());
        let z_100_0 = z_100_50 * z_50_0;
        let z_200_100 = (0..100).fold(z_100_0, |x, _| x.square());
        let z_200_0 = z_200_100 * z_100_0;
        let z_250_50 = (0..50).fold(z_200_0, |x, _| x.square());
        let z_250_0 = z_250_50 * z_50_0;
        let z_255_5 = (0..5).fold(z_250_0, |x, _| x.square());

        z_255_5 * z11
    }

    fn is_nonzero(&self) -> bool {
        self.to_bytes().iter().fold(0, |acc, x| acc | x) != 0
    }

    fn is_negative(&self) -> bool {
        (self.to_bytes()[0] & 1) != 0
    }

    fn neg(&self) -> Fe {
        let &Fe(f) = &self;
        let mut h = Fe::default();
        fiat_25519_opp(&mut h.0, f);
        h
    }

    fn pow25523(&self) -> Fe {
        let z2 = self.square();
        let z8 = (0..2).fold(z2, |x, _| x.square());
        let z9 = *self * z8;
        let z11 = z2 * z9;
        let z22 = z11.square();
        let z_5_0 = z9 * z22;
        let z_10_5 = (0..5).fold(z_5_0, |x, _| x.square());
        let z_10_0 = z_10_5 * z_5_0;
        let z_20_10 = (0..10).fold(z_10_0, |x, _| x.square());
        let z_20_0 = z_20_10 * z_10_0;
        let z_40_20 = (0..20).fold(z_20_0, |x, _| x.square());
        let z_40_0 = z_40_20 * z_20_0;
        let z_50_10 = (0..10).fold(z_40_0, |x, _| x.square());
        let z_50_0 = z_50_10 * z_10_0;
        let z_100_50 = (0..50).fold(z_50_0, |x, _| x.square());
        let z_100_0 = z_100_50 * z_50_0;
        let z_200_100 = (0..100).fold(z_100_0, |x, _| x.square());
        let z_200_0 = z_200_100 * z_100_0;
        let z_250_50 = (0..50).fold(z_200_0, |x, _| x.square());
        let z_250_0 = z_250_50 * z_50_0;
        let z_252_2 = (0..2).fold(z_250_0, |x, _| x.square());

        z_252_2 * *self
    }
}

#[derive(Clone, Copy)]
pub struct GeP2 {
    x: Fe,
    y: Fe,
    z: Fe,
}

#[derive(Clone, Copy)]
pub struct GeP3 {
    x: Fe,
    y: Fe,
    z: Fe,
    t: Fe,
}

#[derive(Clone, Copy)]
pub struct GeP1P1 {
    x: Fe,
    y: Fe,
    z: Fe,
    t: Fe,
}

#[derive(Clone, Copy)]
pub struct GePrecomp {
    y_plus_x: Fe,
    y_minus_x: Fe,
    xy2d: Fe,
}

#[derive(Clone, Copy)]
pub struct GeCached {
    y_plus_x: Fe,
    y_minus_x: Fe,
    z: Fe,
    t2d: Fe,
}

impl GeP1P1 {
    fn to_p2(&self) -> GeP2 {
        GeP2 {
            x: self.x * self.t,
            y: self.y * self.z,
            z: self.z * self.t,
        }
    }

    fn to_p3(&self) -> GeP3 {
        GeP3 {
            x: self.x * self.t,
            y: self.y * self.z,
            z: self.z * self.t,
            t: self.x * self.y,
        }
    }
}

impl GeP2 {
    fn zero() -> GeP2 {
        GeP2 {
            x: FE_ZERO,
            y: FE_ONE,
            z: FE_ONE,
        }
    }

    pub fn to_bytes(&self) -> [u8; 32] {
        let recip = self.z.invert();
        let x = self.x * recip;
        let y = self.y * recip;
        let mut bs = y.to_bytes();
        bs[31] ^= (if x.is_negative() { 1 } else { 0 }) << 7;
        bs
    }

    fn dbl(&self) -> GeP1P1 {
        let xx = self.x.square();
        let yy = self.y.square();
        let b = self.z.square_and_double();
        let a = self.x + self.y;
        let aa = a.square();
        let y3 = yy + xx;
        let z3 = yy - xx;
        let x3 = aa - y3;
        let t3 = b - z3;

        GeP1P1 {
            x: x3,
            y: y3,
            z: z3,
            t: t3,
        }
    }

    fn slide(a: &[u8]) -> [i8; 256] {
        let mut r = [0i8; 256];
        for i in 0..256 {
            r[i] = (1 & (a[i >> 3] >> (i & 7))) as i8;
        }
        for i in 0..256 {
            if r[i] != 0 {
                for b in 1..min(7, 256 - i) {
                    if r[i + b] != 0 {
                        if r[i] + (r[i + b] << b) <= 15 {
                            r[i] += r[i + b] << b;
                            r[i + b] = 0;
                        } else if r[i] - (r[i + b] << b) >= -15 {
                            r[i] -= r[i + b] << b;
                            for k in i + b..256 {
                                if r[k] == 0 {
                                    r[k] = 1;
                                    break;
                                }
                                r[k] = 0;
                            }
                        } else {
                            break;
                        }
                    }
                }
            }
        }

        r
    }

    pub fn double_scalarmult_vartime(a_scalar: &[u8], a_point: GeP3, b_scalar: &[u8]) -> GeP2 {
        let aslide = GeP2::slide(a_scalar);
        let bslide = GeP2::slide(b_scalar);

        let mut ai = [GeCached {
            y_plus_x: FE_ZERO,
            y_minus_x: FE_ZERO,
            z: FE_ZERO,
            t2d: FE_ZERO,
        }; 8]; // A,3A,5A,7A,9A,11A,13A,15A
        ai[0] = a_point.to_cached();
        let a2 = a_point.dbl().to_p3();
        ai[1] = (a2 + ai[0]).to_p3().to_cached();
        ai[2] = (a2 + ai[1]).to_p3().to_cached();
        ai[3] = (a2 + ai[2]).to_p3().to_cached();
        ai[4] = (a2 + ai[3]).to_p3().to_cached();
        ai[5] = (a2 + ai[4]).to_p3().to_cached();
        ai[6] = (a2 + ai[5]).to_p3().to_cached();
        ai[7] = (a2 + ai[6]).to_p3().to_cached();

        let mut r = GeP2::zero();

        let mut i: usize = 255;
        loop {
            if aslide[i] != 0 || bslide[i] != 0 {
                break;
            }
            if i == 0 {
                return r;
            }
            i -= 1;
        }

        loop {
            let mut t = r.dbl();
            if aslide[i] > 0 {
                t = t.to_p3() + ai[(aslide[i] / 2) as usize];
            } else if aslide[i] < 0 {
                t = t.to_p3() - ai[(-aslide[i] / 2) as usize];
            }

            if bslide[i] > 0 {
                t = t.to_p3() + BI[(bslide[i] / 2) as usize];
            } else if bslide[i] < 0 {
                t = t.to_p3() - BI[(-bslide[i] / 2) as usize];
            }

            r = t.to_p2();

            if i == 0 {
                return r;
            }
            i -= 1;
        }
    }
}

impl GeP3 {
    pub fn from_bytes_negate_vartime(s: &[u8]) -> Option<GeP3> {
        let y = Fe::from_bytes(s);
        let z = FE_ONE;
        let y_squared = y.square();
        let u = y_squared - FE_ONE;
        let v = (y_squared * FE_D) + FE_ONE;
        let mut x = (u * v).pow25523() * u;

        let vxx = x.square() * v;
        let check = vxx - u;
        if check.is_nonzero() {
            let check2 = vxx + u;
            if check2.is_nonzero() {
                return None;
            }
            x = x * FE_SQRTM1;
        }

        if x.is_negative() == ((s[31] >> 7) != 0) {
            x = x.neg();
        }

        let t = x * y;

        Some(GeP3 { x, y, z, t })
    }

    fn to_p2(&self) -> GeP2 {
        GeP2 {
            x: self.x,
            y: self.y,
            z: self.z,
        }
    }

    fn to_cached(&self) -> GeCached {
        GeCached {
            y_plus_x: self.y + self.x,
            y_minus_x: self.y - self.x,
            z: self.z,
            t2d: self.t * FE_D2,
        }
    }

    fn dbl(&self) -> GeP1P1 {
        self.to_p2().dbl()
    }
}

impl Add<GeCached> for GeP3 {
    type Output = GeP1P1;

    fn add(self, _rhs: GeCached) -> GeP1P1 {
        let y1_plus_x1 = self.y + self.x;
        let y1_minus_x1 = self.y - self.x;
        let a = y1_plus_x1 * _rhs.y_plus_x;
        let b = y1_minus_x1 * _rhs.y_minus_x;
        let c = _rhs.t2d * self.t;
        let zz = self.z * _rhs.z;
        let d = zz + zz;
        let x3 = a - b;
        let y3 = a + b;
        let z3 = d + c;
        let t3 = d - c;

        GeP1P1 {
            x: x3,
            y: y3,
            z: z3,
            t: t3,
        }
    }
}

impl Add<GePrecomp> for GeP3 {
    type Output = GeP1P1;

    fn add(self, _rhs: GePrecomp) -> GeP1P1 {
        let y1_plus_x1 = self.y + self.x;
        let y1_minus_x1 = self.y - self.x;
        let a = y1_plus_x1 * _rhs.y_plus_x;
        let b = y1_minus_x1 * _rhs.y_minus_x;
        let c = _rhs.xy2d * self.t;
        let d = self.z + self.z;
        let x3 = a - b;
        let y3 = a + b;
        let z3 = d + c;
        let t3 = d - c;

        GeP1P1 {
            x: x3,
            y: y3,
            z: z3,
            t: t3,
        }
    }
}

impl Sub<GeCached> for GeP3 {
    type Output = GeP1P1;

    fn sub(self, _rhs: GeCached) -> GeP1P1 {
        let y1_plus_x1 = self.y + self.x;
        let y1_minus_x1 = self.y - self.x;
        let a = y1_plus_x1 * _rhs.y_minus_x;
        let b = y1_minus_x1 * _rhs.y_plus_x;
        let c = _rhs.t2d * self.t;
        let zz = self.z * _rhs.z;
        let d = zz + zz;
        let x3 = a - b;
        let y3 = a + b;
        let z3 = d - c;
        let t3 = d + c;

        GeP1P1 {
            x: x3,
            y: y3,
            z: z3,
            t: t3,
        }
    }
}

impl Sub<GePrecomp> for GeP3 {
    type Output = GeP1P1;

    fn sub(self, _rhs: GePrecomp) -> GeP1P1 {
        let y1_plus_x1 = self.y + self.x;
        let y1_minus_x1 = self.y - self.x;
        let a = y1_plus_x1 * _rhs.y_minus_x;
        let b = y1_minus_x1 * _rhs.y_plus_x;
        let c = _rhs.xy2d * self.t;
        let d = self.z + self.z;
        let x3 = a - b;
        let y3 = a + b;
        let z3 = d - c;
        let t3 = d + c;

        GeP1P1 {
            x: x3,
            y: y3,
            z: z3,
            t: t3,
        }
    }
}

pub fn sc_reduce(s: &mut [u8]) {
    let mut s0: i64 = 2097151 & load_3i(s);
    let mut s1: i64 = 2097151 & (load_4i(&s[2..6]) >> 5);
    let mut s2: i64 = 2097151 & (load_3i(&s[5..8]) >> 2);
    let mut s3: i64 = 2097151 & (load_4i(&s[7..11]) >> 7);
    let mut s4: i64 = 2097151 & (load_4i(&s[10..14]) >> 4);
    let mut s5: i64 = 2097151 & (load_3i(&s[13..16]) >> 1);
    let mut s6: i64 = 2097151 & (load_4i(&s[15..19]) >> 6);
    let mut s7: i64 = 2097151 & (load_3i(&s[18..21]) >> 3);
    let mut s8: i64 = 2097151 & load_3i(&s[21..24]);
    let mut s9: i64 = 2097151 & (load_4i(&s[23..27]) >> 5);
    let mut s10: i64 = 2097151 & (load_3i(&s[26..29]) >> 2);
    let mut s11: i64 = 2097151 & (load_4i(&s[28..32]) >> 7);
    let mut s12: i64 = 2097151 & (load_4i(&s[31..35]) >> 4);
    let mut s13: i64 = 2097151 & (load_3i(&s[34..37]) >> 1);
    let mut s14: i64 = 2097151 & (load_4i(&s[36..40]) >> 6);
    let mut s15: i64 = 2097151 & (load_3i(&s[39..42]) >> 3);
    let mut s16: i64 = 2097151 & load_3i(&s[42..45]);
    let mut s17: i64 = 2097151 & (load_4i(&s[44..48]) >> 5);
    let s18: i64 = 2097151 & (load_3i(&s[47..50]) >> 2);
    let s19: i64 = 2097151 & (load_4i(&s[49..53]) >> 7);
    let s20: i64 = 2097151 & (load_4i(&s[52..56]) >> 4);
    let s21: i64 = 2097151 & (load_3i(&s[55..58]) >> 1);
    let s22: i64 = 2097151 & (load_4i(&s[57..61]) >> 6);
    let s23: i64 = load_4i(&s[60..64]) >> 3;

    s11 += s23 * 666643;
    s12 += s23 * 470296;
    s13 += s23 * 654183;
    s14 -= s23 * 997805;
    s15 += s23 * 136657;
    s16 -= s23 * 683901;

    s10 += s22 * 666643;
    s11 += s22 * 470296;
    s12 += s22 * 654183;
    s13 -= s22 * 997805;
    s14 += s22 * 136657;
    s15 -= s22 * 683901;

    s9 += s21 * 666643;
    s10 += s21 * 470296;
    s11 += s21 * 654183;
    s12 -= s21 * 997805;
    s13 += s21 * 136657;
    s14 -= s21 * 683901;

    s8 += s20 * 666643;
    s9 += s20 * 470296;
    s10 += s20 * 654183;
    s11 -= s20 * 997805;
    s12 += s20 * 136657;
    s13 -= s20 * 683901;

    s7 += s19 * 666643;
    s8 += s19 * 470296;
    s9 += s19 * 654183;
    s10 -= s19 * 997805;
    s11 += s19 * 136657;
    s12 -= s19 * 683901;

    s6 += s18 * 666643;
    s7 += s18 * 470296;
    s8 += s18 * 654183;
    s9 -= s18 * 997805;
    s10 += s18 * 136657;
    s11 -= s18 * 683901;

    let mut carry6: i64 = (s6 + (1 << 20)) >> 21;
    s7 += carry6;
    s6 -= carry6 << 21;
    let mut carry8: i64 = (s8 + (1 << 20)) >> 21;
    s9 += carry8;
    s8 -= carry8 << 21;
    let mut carry10: i64 = (s10 + (1 << 20)) >> 21;
    s11 += carry10;
    s10 -= carry10 << 21;
    let carry12: i64 = (s12 + (1 << 20)) >> 21;
    s13 += carry12;
    s12 -= carry12 << 21;
    let carry14: i64 = (s14 + (1 << 20)) >> 21;
    s15 += carry14;
    s14 -= carry14 << 21;
    let carry16: i64 = (s16 + (1 << 20)) >> 21;
    s17 += carry16;
    s16 -= carry16 << 21;

    let mut carry7: i64 = (s7 + (1 << 20)) >> 21;
    s8 += carry7;
    s7 -= carry7 << 21;
    let mut carry9: i64 = (s9 + (1 << 20)) >> 21;
    s10 += carry9;
    s9 -= carry9 << 21;
    let mut carry11: i64 = (s11 + (1 << 20)) >> 21;
    s12 += carry11;
    s11 -= carry11 << 21;
    let carry13: i64 = (s13 + (1 << 20)) >> 21;
    s14 += carry13;
    s13 -= carry13 << 21;
    let carry15: i64 = (s15 + (1 << 20)) >> 21;
    s16 += carry15;
    s15 -= carry15 << 21;

    s5 += s17 * 666643;
    s6 += s17 * 470296;
    s7 += s17 * 654183;
    s8 -= s17 * 997805;
    s9 += s17 * 136657;
    s10 -= s17 * 683901;

    s4 += s16 * 666643;
    s5 += s16 * 470296;
    s6 += s16 * 654183;
    s7 -= s16 * 997805;
    s8 += s16 * 136657;
    s9 -= s16 * 683901;

    s3 += s15 * 666643;
    s4 += s15 * 470296;
    s5 += s15 * 654183;
    s6 -= s15 * 997805;
    s7 += s15 * 136657;
    s8 -= s15 * 683901;

    s2 += s14 * 666643;
    s3 += s14 * 470296;
    s4 += s14 * 654183;
    s5 -= s14 * 997805;
    s6 += s14 * 136657;
    s7 -= s14 * 683901;

    s1 += s13 * 666643;
    s2 += s13 * 470296;
    s3 += s13 * 654183;
    s4 -= s13 * 997805;
    s5 += s13 * 136657;
    s6 -= s13 * 683901;

    s0 += s12 * 666643;
    s1 += s12 * 470296;
    s2 += s12 * 654183;
    s3 -= s12 * 997805;
    s4 += s12 * 136657;
    s5 -= s12 * 683901;
    s12 = 0;

    let mut carry0: i64 = (s0 + (1 << 20)) >> 21;
    s1 += carry0;
    s0 -= carry0 << 21;
    let mut carry2: i64 = (s2 + (1 << 20)) >> 21;
    s3 += carry2;
    s2 -= carry2 << 21;
    let mut carry4: i64 = (s4 + (1 << 20)) >> 21;
    s5 += carry4;
    s4 -= carry4 << 21;
    carry6 = (s6 + (1 << 20)) >> 21;
    s7 += carry6;
    s6 -= carry6 << 21;
    carry8 = (s8 + (1 << 20)) >> 21;
    s9 += carry8;
    s8 -= carry8 << 21;
    carry10 = (s10 + (1 << 20)) >> 21;
    s11 += carry10;
    s10 -= carry10 << 21;

    let mut carry1: i64 = (s1 + (1 << 20)) >> 21;
    s2 += carry1;
    s1 -= carry1 << 21;
    let mut carry3: i64 = (s3 + (1 << 20)) >> 21;
    s4 += carry3;
    s3 -= carry3 << 21;
    let mut carry5: i64 = (s5 + (1 << 20)) >> 21;
    s6 += carry5;
    s5 -= carry5 << 21;
    carry7 = (s7 + (1 << 20)) >> 21;
    s8 += carry7;
    s7 -= carry7 << 21;
    carry9 = (s9 + (1 << 20)) >> 21;
    s10 += carry9;
    s9 -= carry9 << 21;
    carry11 = (s11 + (1 << 20)) >> 21;
    s12 += carry11;
    s11 -= carry11 << 21;

    s0 += s12 * 666643;
    s1 += s12 * 470296;
    s2 += s12 * 654183;
    s3 -= s12 * 997805;
    s4 += s12 * 136657;
    s5 -= s12 * 683901;
    s12 = 0;

    carry0 = s0 >> 21;
    s1 += carry0;
    s0 -= carry0 << 21;
    carry1 = s1 >> 21;
    s2 += carry1;
    s1 -= carry1 << 21;
    carry2 = s2 >> 21;
    s3 += carry2;
    s2 -= carry2 << 21;
    carry3 = s3 >> 21;
    s4 += carry3;
    s3 -= carry3 << 21;
    carry4 = s4 >> 21;
    s5 += carry4;
    s4 -= carry4 << 21;
    carry5 = s5 >> 21;
    s6 += carry5;
    s5 -= carry5 << 21;
    carry6 = s6 >> 21;
    s7 += carry6;
    s6 -= carry6 << 21;
    carry7 = s7 >> 21;
    s8 += carry7;
    s7 -= carry7 << 21;
    carry8 = s8 >> 21;
    s9 += carry8;
    s8 -= carry8 << 21;
    carry9 = s9 >> 21;
    s10 += carry9;
    s9 -= carry9 << 21;
    carry10 = s10 >> 21;
    s11 += carry10;
    s10 -= carry10 << 21;
    carry11 = s11 >> 21;
    s12 += carry11;
    s11 -= carry11 << 21;

    s0 += s12 * 666643;
    s1 += s12 * 470296;
    s2 += s12 * 654183;
    s3 -= s12 * 997805;
    s4 += s12 * 136657;
    s5 -= s12 * 683901;

    carry0 = s0 >> 21;
    s1 += carry0;
    s0 -= carry0 << 21;
    carry1 = s1 >> 21;
    s2 += carry1;
    s1 -= carry1 << 21;
    carry2 = s2 >> 21;
    s3 += carry2;
    s2 -= carry2 << 21;
    carry3 = s3 >> 21;
    s4 += carry3;
    s3 -= carry3 << 21;
    carry4 = s4 >> 21;
    s5 += carry4;
    s4 -= carry4 << 21;
    carry5 = s5 >> 21;
    s6 += carry5;
    s5 -= carry5 << 21;
    carry6 = s6 >> 21;
    s7 += carry6;
    s6 -= carry6 << 21;
    carry7 = s7 >> 21;
    s8 += carry7;
    s7 -= carry7 << 21;
    carry8 = s8 >> 21;
    s9 += carry8;
    s8 -= carry8 << 21;
    carry9 = s9 >> 21;
    s10 += carry9;
    s9 -= carry9 << 21;
    carry10 = s10 >> 21;
    s11 += carry10;
    s10 -= carry10 << 21;

    s[0] = s0 as u8;
    s[1] = (s0 >> 8) as u8;
    s[2] = ((s0 >> 16) | (s1 << 5)) as u8;
    s[3] = (s1 >> 3) as u8;
    s[4] = (s1 >> 11) as u8;
    s[5] = ((s1 >> 19) | (s2 << 2)) as u8;
    s[6] = (s2 >> 6) as u8;
    s[7] = ((s2 >> 14) | (s3 << 7)) as u8;
    s[8] = (s3 >> 1) as u8;
    s[9] = (s3 >> 9) as u8;
    s[10] = ((s3 >> 17) | (s4 << 4)) as u8;
    s[11] = (s4 >> 4) as u8;
    s[12] = (s4 >> 12) as u8;
    s[13] = ((s4 >> 20) | (s5 << 1)) as u8;
    s[14] = (s5 >> 7) as u8;
    s[15] = ((s5 >> 15) | (s6 << 6)) as u8;
    s[16] = (s6 >> 2) as u8;
    s[17] = (s6 >> 10) as u8;
    s[18] = ((s6 >> 18) | (s7 << 3)) as u8;
    s[19] = (s7 >> 5) as u8;
    s[20] = (s7 >> 13) as u8;
    s[21] = s8 as u8;
    s[22] = (s8 >> 8) as u8;
    s[23] = ((s8 >> 16) | (s9 << 5)) as u8;
    s[24] = (s9 >> 3) as u8;
    s[25] = (s9 >> 11) as u8;
    s[26] = ((s9 >> 19) | (s10 << 2)) as u8;
    s[27] = (s10 >> 6) as u8;
    s[28] = ((s10 >> 14) | (s11 << 7)) as u8;
    s[29] = (s11 >> 1) as u8;
    s[30] = (s11 >> 9) as u8;
    s[31] = (s11 >> 17) as u8;
}

pub fn is_identity(s: &[u8]) -> bool {
    let mut c = s[0] ^ 0x01;
    for i in 1..31 {
        c |= s[i];
    }
    c |= s[31] & 0x7f;
    c == 0
}

static BI: [GePrecomp; 8] = [
    GePrecomp {
        y_plus_x: Fe([
            1288382639258501,
            245678601348599,
            269427782077623,
            1462984067271730,
            137412439391563,
        ]),
        y_minus_x: Fe([
            62697248952638,
            204681361388450,
            631292143396476,
            338455783676468,
            1213667448819585,
        ]),
        xy2d: Fe([
            301289933810280,
            1259582250014073,
            1422107436869536,
            796239922652654,
            1953934009299142,
        ]),
    },
    GePrecomp {
        y_plus_x: Fe([
            1601611775252272,
            1720807796594148,
            1132070835939856,
            1260455018889551,
            2147779492816911,
        ]),
        y_minus_x: Fe([
            316559037616741,
            2177824224946892,
            1459442586438991,
            1461528397712656,
            751590696113597,
        ]),
        xy2d: Fe([
            1850748884277385,
            1200145853858453,
            1068094770532492,
            672251375690438,
            1586055907191707,
        ]),
    },
    GePrecomp {
        y_plus_x: Fe([
            769950342298419,
            132954430919746,
            844085933195555,
            974092374476333,
            726076285546016,
        ]),
        y_minus_x: Fe([
            425251763115706,
            608463272472562,
            442562545713235,
            837766094556764,
            374555092627893,
        ]),
        xy2d: Fe([
            1086255230780037,
            274979815921559,
            1960002765731872,
            929474102396301,
            1190409889297339,
        ]),
    },
    GePrecomp {
        y_plus_x: Fe([
            665000864555967,
            2065379846933859,
            370231110385876,
            350988370788628,
            1233371373142985,
        ]),
        y_minus_x: Fe([
            2019367628972465,
            676711900706637,
            110710997811333,
            1108646842542025,
            517791959672113,
        ]),
        xy2d: Fe([
            965130719900578,
            247011430587952,
            526356006571389,
            91986625355052,
            2157223321444601,
        ]),
    },
    GePrecomp {
        y_plus_x: Fe([
            1802695059465007,
            1664899123557221,
            593559490740857,
            2160434469266659,
            927570450755031,
        ]),
        y_minus_x: Fe([
            1725674970513508,
            1933645953859181,
            1542344539275782,
            1767788773573747,
            1297447965928905,
        ]),
        xy2d: Fe([
            1381809363726107,
            1430341051343062,
            2061843536018959,
            1551778050872521,
            2036394857967624,
        ]),
    },
    GePrecomp {
        y_plus_x: Fe([
            1970894096313054,
            528066325833207,
            1619374932191227,
            2207306624415883,
            1169170329061080,
        ]),
        y_minus_x: Fe([
            2070390218572616,
            1458919061857835,
            624171843017421,
            1055332792707765,
            433987520732508,
        ]),
        xy2d: Fe([
            893653801273833,
            1168026499324677,
            1242553501121234,
            1306366254304474,
            1086752658510815,
        ]),
    },
    GePrecomp {
        y_plus_x: Fe([
            213454002618221,
            939771523987438,
            1159882208056014,
            317388369627517,
            621213314200687,
        ]),
        y_minus_x: Fe([
            1971678598905747,
            338026507889165,
            762398079972271,
            655096486107477,
            42299032696322,
        ]),
        xy2d: Fe([
            177130678690680,
            1754759263300204,
            1864311296286618,
            1180675631479880,
            1292726903152791,
        ]),
    },
    GePrecomp {
        y_plus_x: Fe([
            1913163449625248,
            460779200291993,
            2193883288642314,
            1008900146920800,
            1721983679009502,
        ]),
        y_minus_x: Fe([
            1070401523076875,
            1272492007800961,
            1910153608563310,
            2075579521696771,
            1191169788841221,
        ]),
        xy2d: Fe([
            692896803108118,
            500174642072499,
            2068223309439677,
            1162190621851337,
            1426986007309901,
        ]),
    },
];
