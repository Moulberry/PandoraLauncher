/* ====================================================================
 * Copyright (c) 2002-2006 The OpenSSL Project.  All rights reserved.
 *
 * Redistribution and use in source and binary forms, with or without
 * modification, are permitted provided that the following conditions
 * are met:
 *
 * 1. Redistributions of source code must retain the above copyright
 *    notice, this list of conditions and the following disclaimer.
 *
 * 2. Redistributions in binary form must reproduce the above copyright
 *    notice, this list of conditions and the following disclaimer in
 *    the documentation and/or other materials provided with the
 *    distribution.
 *
 * 3. All advertising materials mentioning features or use of this
 *    software must display the following acknowledgment:
 *    "This product includes software developed by the OpenSSL Project
 *    for use in the OpenSSL Toolkit. (http://www.openssl.org/)"
 *
 * 4. The names "OpenSSL Toolkit" and "OpenSSL Project" must not be used to
 *    endorse or promote products derived from this software without
 *    prior written permission. For written permission, please contact
 *    openssl-core@openssl.org.
 *
 * 5. Products derived from this software may not be called "OpenSSL"
 *    nor may "OpenSSL" appear in their names without prior written
 *    permission of the OpenSSL Project.
 *
 * 6. Redistributions of any form whatsoever must retain the following
 *    acknowledgment:
 *    "This product includes software developed by the OpenSSL Project
 *    for use in the OpenSSL Toolkit (http://www.openssl.org/)"
 *
 * THIS SOFTWARE IS PROVIDED BY THE OpenSSL PROJECT ``AS IS'' AND ANY
 * EXPRESSED OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
 * IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR
 * PURPOSE ARE DISCLAIMED.  IN NO EVENT SHALL THE OpenSSL PROJECT OR
 * ITS CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
 * SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT
 * NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES;
 * LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION)
 * HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
 * STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
 * ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED
 * OF THE POSSIBILITY OF SUCH DAMAGE.
 * ==================================================================== */

#include <openssl/aes.h>

#include <assert.h>

#include "../aes/internal.h"
#include "../modes/internal.h"
#include "../cipher/internal.h"

// The following wrappers ensure that the delocator can handle the
// function pointer calculation in AES_ctr128_encrypt. Without it,
// on AArch64 there is risk of the calculations requiring a PC-relative
// offset outside of the range (-1MB,1MB) addressable using `ADR`.
static inline void aes_hw_encrypt_wrapper(const uint8_t *in, uint8_t *out,
                                          const AES_KEY *key) {
  aes_hw_encrypt(in, out, key);
}

static inline void aes_nohw_encrypt_wrapper(const uint8_t *in, uint8_t *out,
                                            const AES_KEY *key) {
  aes_nohw_encrypt(in, out, key);
}

static inline void aes_hw_ctr32_encrypt_blocks_wrapper(const uint8_t *in,
                                                       uint8_t *out, size_t len,
                                                       const AES_KEY *key,
                                                       const uint8_t ivec[16]) {
  aes_hw_ctr32_encrypt_blocks(in, out, len, key, ivec);
}

static inline void aes_nohw_ctr32_encrypt_blocks_wrapper(const uint8_t *in,
                                                         uint8_t *out, size_t len,
                                                         const AES_KEY *key,
                                                         const uint8_t ivec[16]) {
  aes_nohw_ctr32_encrypt_blocks(in, out, len, key, ivec);
}

static inline void vpaes_encrypt_wrapper(const uint8_t *in, uint8_t *out,
                                         const AES_KEY *key) {
  vpaes_encrypt(in, out, key);
}

#if defined(VPAES_CTR32)
static inline void vpaes_ctr32_encrypt_blocks_wrapper(const uint8_t *in,
                                                      uint8_t *out, size_t len,
                                                      const AES_KEY *key,
                                                      const uint8_t ivec[16]) {
  vpaes_ctr32_encrypt_blocks(in, out, len, key, ivec);
}
#endif

void AES_ctr128_encrypt(const uint8_t *in, uint8_t *out, size_t len,
                        const AES_KEY *key, uint8_t ivec[AES_BLOCK_SIZE],
                        uint8_t ecount_buf[AES_BLOCK_SIZE], unsigned int *num) {
  if (hwaes_capable()) {
    CRYPTO_ctr128_encrypt_ctr32(in, out, len, key, ivec, ecount_buf, num,
                                aes_hw_ctr32_encrypt_blocks_wrapper);
  } else if (vpaes_capable()) {
#if defined(VPAES_CTR32)
    // TODO(davidben): On ARM, where |BSAES| is additionally defined, this could
    // use |vpaes_ctr32_encrypt_blocks_with_bsaes|.
    CRYPTO_ctr128_encrypt_ctr32(in, out, len, key, ivec, ecount_buf, num,
                                vpaes_ctr32_encrypt_blocks_wrapper);
#else
    CRYPTO_ctr128_encrypt(in, out, len, key, ivec, ecount_buf, num,
                          vpaes_encrypt_wrapper);
#endif
  } else {
    CRYPTO_ctr128_encrypt_ctr32(in, out, len, key, ivec, ecount_buf, num,
                                aes_nohw_ctr32_encrypt_blocks_wrapper);
  }

  FIPS_service_indicator_update_state();
}

void AES_ecb_encrypt(const uint8_t *in, uint8_t *out, const AES_KEY *key,
                     const int enc) {
  assert(in && out && key);
  assert((AES_ENCRYPT == enc) || (AES_DECRYPT == enc));

  if (AES_ENCRYPT == enc) {
    AES_encrypt(in, out, key);
  } else {
    AES_decrypt(in, out, key);
  }

  FIPS_service_indicator_update_state();
}

void AES_cbc_encrypt(const uint8_t *in, uint8_t *out, size_t len,
                     const AES_KEY *key, uint8_t *ivec, const int enc) {
  if (hwaes_capable()) {
    aes_hw_cbc_encrypt(in, out, len, key, ivec, enc);
  } else if (!vpaes_capable()) {
    aes_nohw_cbc_encrypt(in, out, len, key, ivec, enc);
  } else if (enc) {
    CRYPTO_cbc128_encrypt(in, out, len, key, ivec, AES_encrypt);
  } else {
    CRYPTO_cbc128_decrypt(in, out, len, key, ivec, AES_decrypt);
  }

  FIPS_service_indicator_update_state();
}

void AES_ofb128_encrypt(const uint8_t *in, uint8_t *out, size_t length,
                        const AES_KEY *key, uint8_t *ivec, int *num) {
  unsigned num_u = (unsigned)(*num);
  CRYPTO_ofb128_encrypt(in, out, length, key, ivec, &num_u, AES_encrypt);
  *num = (int)num_u;
}

void AES_cfb1_encrypt(const uint8_t *in, uint8_t *out, size_t bits,
                        const AES_KEY *key, uint8_t *ivec, int *num,
                        int enc) {
  unsigned num_u = (unsigned)(*num);
  CRYPTO_cfb128_1_encrypt(in, out, bits, key, ivec, &num_u, enc, AES_encrypt);
  *num = (int)num_u;
}

void AES_cfb8_encrypt(const uint8_t *in, uint8_t *out, size_t length,
                        const AES_KEY *key, uint8_t *ivec, int *num,
                        int enc) {
  unsigned num_u = (unsigned)(*num);
  CRYPTO_cfb128_8_encrypt(in, out, length, key, ivec, &num_u, enc, AES_encrypt);
  *num = (int)num_u;
}

void AES_cfb128_encrypt(const uint8_t *in, uint8_t *out, size_t length,
                        const AES_KEY *key, uint8_t *ivec, int *num,
                        int enc) {
  unsigned num_u = (unsigned)(*num);
  CRYPTO_cfb128_encrypt(in, out, length, key, ivec, &num_u, enc, AES_encrypt);
  *num = (int)num_u;
}

#if defined(HWAES_XTS)
int aes_hw_xts_cipher(const uint8_t *in, uint8_t *out, size_t length,
                       const AES_KEY *key1, const AES_KEY *key2,
                       const uint8_t iv[16], int enc) {
  // The assembly functions abort on the following condition.
  // They can be modified to return 0/1 instead of void, but
  // this is the easy way out for now.
  if (length < 16) return 0;

  if (enc) {
#if defined(AES_XTS_X86_64_AVX512)
    if (avx512_xts_available()) {
      aes_hw_xts_encrypt_avx512(in, out, length, key1, key2, iv);
      return 1;
    }
#endif
    aes_hw_xts_encrypt(in, out, length, key1, key2, iv);
  } else {
#if defined(AES_XTS_X86_64_AVX512)
    if (avx512_xts_available()) {
      aes_hw_xts_decrypt_avx512(in, out, length, key1, key2, iv);
      return 1;
    }
#endif
    aes_hw_xts_decrypt(in, out, length, key1, key2, iv);
  }
  return 1;
}

#endif // HWAES_XTS
