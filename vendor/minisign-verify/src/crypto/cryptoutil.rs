// Copyright 2012-2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::ptr;
use std::{io, mem::MaybeUninit};

pub fn write_u64_le(dst: &mut [u8], mut input: u64) {
    assert!(dst.len() == 8);
    input = input.to_le();
    unsafe {
        let tmp = &input as *const _ as *const u8;
        ptr::copy_nonoverlapping(tmp, dst.get_unchecked_mut(0), 8);
    }
}

pub fn write_u64v_le(dst: &mut [u8], input: &[u64]) {
    assert!(dst.len() == 8 * input.len());
    unsafe {
        let mut x: *mut u8 = dst.get_unchecked_mut(0);
        let mut y: *const u64 = input.get_unchecked(0);
        for _ in 0..input.len() {
            let tmp = (*y).to_le();
            ptr::copy_nonoverlapping(&tmp as *const _ as *const u8, x, 8);
            x = x.offset(8);
            y = y.offset(1);
        }
    }
}

pub fn write_u32_le(dst: &mut [u8], mut input: u32) {
    assert!(dst.len() == 4);
    input = input.to_le();
    unsafe {
        let tmp = &input as *const _ as *const u8;
        ptr::copy_nonoverlapping(tmp, dst.get_unchecked_mut(0), 4);
    }
}

pub fn read_u64v_le(dst: &mut [u64], input: &[u8]) {
    assert!(dst.len() * 8 == input.len());
    unsafe {
        let mut x: *mut u64 = dst.get_unchecked_mut(0);
        let mut y: *const u8 = input.get_unchecked(0);
        for _ in 0..dst.len() {
            let mut tmp = MaybeUninit::<u64>::uninit();
            ptr::copy_nonoverlapping(y, tmp.as_mut_ptr() as *mut u8, 8);
            *x = u64::from_le(tmp.assume_init());
            x = x.offset(1);
            y = y.offset(8);
        }
    }
}

#[inline]
pub fn copy_memory(src: &[u8], dst: &mut [u8]) {
    assert!(dst.len() >= src.len());
    unsafe {
        let srcp = src.as_ptr();
        let dstp = dst.as_mut_ptr();
        ptr::copy_nonoverlapping(srcp, dstp, src.len());
    }
}

pub trait WriteExt {
    fn write_u8(&mut self, val: u8) -> io::Result<()>;
    fn write_u32_le(&mut self, val: u32) -> io::Result<()>;
    fn write_u64_le(&mut self, val: u64) -> io::Result<()>;
}

impl<T> WriteExt for T
where
    T: io::Write,
{
    fn write_u8(&mut self, val: u8) -> io::Result<()> {
        let buff = [val];
        self.write_all(&buff)
    }

    fn write_u32_le(&mut self, val: u32) -> io::Result<()> {
        let mut buff = [0u8; 4];
        write_u32_le(&mut buff, val);
        self.write_all(&buff)
    }

    fn write_u64_le(&mut self, val: u64) -> io::Result<()> {
        let mut buff = [0u8; 8];
        write_u64_le(&mut buff, val);
        self.write_all(&buff)
    }
}
