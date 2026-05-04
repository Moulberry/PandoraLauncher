// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0 OR ISC

use crate::aws_lc::{AES_set_decrypt_key, AES_set_encrypt_key, AES_KEY};
use crate::cipher::block::Block;
use crate::cipher::chacha::ChaCha20Key;
use crate::cipher::{AES_128_KEY_LEN, AES_192_KEY_LEN, AES_256_KEY_LEN};
use crate::error::Unspecified;
use core::mem::{size_of, MaybeUninit};
use core::ptr::copy_nonoverlapping;
// TODO: Uncomment when MSRV >= 1.64
// use core::ffi::c_uint;
use std::os::raw::c_uint;
use zeroize::Zeroize;

pub(crate) enum SymmetricCipherKey {
    Aes128 { enc_key: AES_KEY, dec_key: AES_KEY },
    Aes192 { enc_key: AES_KEY, dec_key: AES_KEY },
    Aes256 { enc_key: AES_KEY, dec_key: AES_KEY },
    ChaCha20 { raw_key: ChaCha20Key },
}

unsafe impl Send for SymmetricCipherKey {}

// The AES_KEY value is only used as a `*const AES_KEY` in calls to `AES_encrypt`.
unsafe impl Sync for SymmetricCipherKey {}

impl Drop for SymmetricCipherKey {
    fn drop(&mut self) {
        // Aes128Key, Aes256Key and ChaCha20Key implement Drop separately.
        match self {
            SymmetricCipherKey::Aes128 { enc_key, dec_key }
            | SymmetricCipherKey::Aes192 { enc_key, dec_key }
            | SymmetricCipherKey::Aes256 { enc_key, dec_key } => unsafe {
                let enc_bytes: &mut [u8; size_of::<AES_KEY>()] = (enc_key as *mut AES_KEY)
                    .cast::<[u8; size_of::<AES_KEY>()]>()
                    .as_mut()
                    .unwrap();
                enc_bytes.zeroize();
                let dec_bytes: &mut [u8; size_of::<AES_KEY>()] = (dec_key as *mut AES_KEY)
                    .cast::<[u8; size_of::<AES_KEY>()]>()
                    .as_mut()
                    .unwrap();
                dec_bytes.zeroize();
            },
            SymmetricCipherKey::ChaCha20 { .. } => {}
        }
    }
}

impl SymmetricCipherKey {
    fn aes(key_bytes: &[u8]) -> Result<(AES_KEY, AES_KEY), Unspecified> {
        let mut enc_key = MaybeUninit::<AES_KEY>::uninit();
        let mut dec_key = MaybeUninit::<AES_KEY>::uninit();
        #[allow(clippy::cast_possible_truncation)]
        if unsafe {
            0 != AES_set_encrypt_key(
                key_bytes.as_ptr(),
                (key_bytes.len() * 8) as c_uint,
                enc_key.as_mut_ptr(),
            )
        } {
            return Err(Unspecified);
        }

        #[allow(clippy::cast_possible_truncation)]
        if unsafe {
            0 != AES_set_decrypt_key(
                key_bytes.as_ptr(),
                (key_bytes.len() * 8) as c_uint,
                dec_key.as_mut_ptr(),
            )
        } {
            return Err(Unspecified);
        }
        unsafe { Ok((enc_key.assume_init(), dec_key.assume_init())) }
    }

    pub(crate) fn aes128(key_bytes: &[u8]) -> Result<Self, Unspecified> {
        if key_bytes.len() != AES_128_KEY_LEN {
            return Err(Unspecified);
        }
        let (enc_key, dec_key) = SymmetricCipherKey::aes(key_bytes)?;
        Ok(SymmetricCipherKey::Aes128 { enc_key, dec_key })
    }

    pub(crate) fn aes192(key_bytes: &[u8]) -> Result<Self, Unspecified> {
        if key_bytes.len() != AES_192_KEY_LEN {
            return Err(Unspecified);
        }
        let (enc_key, dec_key) = SymmetricCipherKey::aes(key_bytes)?;
        Ok(SymmetricCipherKey::Aes192 { enc_key, dec_key })
    }

    pub(crate) fn aes256(key_bytes: &[u8]) -> Result<Self, Unspecified> {
        if key_bytes.len() != AES_256_KEY_LEN {
            return Err(Unspecified);
        }
        let (enc_key, dec_key) = SymmetricCipherKey::aes(key_bytes)?;
        Ok(SymmetricCipherKey::Aes256 { enc_key, dec_key })
    }

    pub(crate) fn chacha20(key_bytes: &[u8]) -> Result<Self, Unspecified> {
        if key_bytes.len() != 32 {
            return Err(Unspecified);
        }
        let mut kb = MaybeUninit::<[u8; 32]>::uninit();
        unsafe {
            copy_nonoverlapping(key_bytes.as_ptr(), kb.as_mut_ptr().cast(), 32);
            Ok(SymmetricCipherKey::ChaCha20 {
                raw_key: ChaCha20Key(kb.assume_init()),
            })
        }
    }

    #[allow(dead_code)]
    #[inline]
    pub(crate) fn encrypt_block(&self, block: Block) -> Block {
        match self {
            SymmetricCipherKey::Aes128 { enc_key, .. }
            | SymmetricCipherKey::Aes192 { enc_key, .. }
            | SymmetricCipherKey::Aes256 { enc_key, .. } => {
                super::aes::encrypt_block(enc_key, block)
            }
            SymmetricCipherKey::ChaCha20 { .. } => panic!("Unsupported algorithm!"),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::cipher::block::{Block, BLOCK_LEN};
    use crate::cipher::key::SymmetricCipherKey;
    use crate::test::from_hex;

    #[test]
    fn test_encrypt_block_aes_128() {
        let key = from_hex("000102030405060708090a0b0c0d0e0f").unwrap();
        let input = from_hex("00112233445566778899aabbccddeeff").unwrap();
        let expected_result = from_hex("69c4e0d86a7b0430d8cdb78070b4c55a").unwrap();
        let input_block: [u8; BLOCK_LEN] = <[u8; BLOCK_LEN]>::try_from(input).unwrap();

        let aes128 = SymmetricCipherKey::aes128(key.as_slice()).unwrap();
        let result = aes128.encrypt_block(Block::from(input_block));

        assert_eq!(expected_result.as_slice(), result.as_ref());
    }

    #[test]
    fn test_encrypt_block_aes_256() {
        let key =
            from_hex("000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f").unwrap();
        let input = from_hex("00112233445566778899aabbccddeeff").unwrap();
        let expected_result = from_hex("8ea2b7ca516745bfeafc49904b496089").unwrap();
        let input_block: [u8; BLOCK_LEN] = <[u8; BLOCK_LEN]>::try_from(input).unwrap();

        let aes128 = SymmetricCipherKey::aes256(key.as_slice()).unwrap();
        let result = aes128.encrypt_block(Block::from(input_block));

        assert_eq!(expected_result.as_slice(), result.as_ref());
    }
}
