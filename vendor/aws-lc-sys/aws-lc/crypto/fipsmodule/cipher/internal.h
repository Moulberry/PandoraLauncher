/* Copyright (C) 1995-1998 Eric Young (eay@cryptsoft.com)
 * All rights reserved.
 *
 * This package is an SSL implementation written
 * by Eric Young (eay@cryptsoft.com).
 * The implementation was written so as to conform with Netscapes SSL.
 *
 * This library is free for commercial and non-commercial use as long as
 * the following conditions are aheared to.  The following conditions
 * apply to all code found in this distribution, be it the RC4, RSA,
 * lhash, DES, etc., code; not just the SSL code.  The SSL documentation
 * included with this distribution is covered by the same copyright terms
 * except that the holder is Tim Hudson (tjh@cryptsoft.com).
 *
 * Copyright remains Eric Young's, and as such any Copyright notices in
 * the code are not to be removed.
 * If this package is used in a product, Eric Young should be given attribution
 * as the author of the parts of the library used.
 * This can be in the form of a textual message at program startup or
 * in documentation (online or textual) provided with the package.
 *
 * Redistribution and use in source and binary forms, with or without
 * modification, are permitted provided that the following conditions
 * are met:
 * 1. Redistributions of source code must retain the copyright
 *    notice, this list of conditions and the following disclaimer.
 * 2. Redistributions in binary form must reproduce the above copyright
 *    notice, this list of conditions and the following disclaimer in the
 *    documentation and/or other materials provided with the distribution.
 * 3. All advertising materials mentioning features or use of this software
 *    must display the following acknowledgement:
 *    "This product includes cryptographic software written by
 *     Eric Young (eay@cryptsoft.com)"
 *    The word 'cryptographic' can be left out if the rouines from the library
 *    being used are not cryptographic related :-).
 * 4. If you include any Windows specific code (or a derivative thereof) from
 *    the apps directory (application code) you must include an acknowledgement:
 *    "This product includes software written by Tim Hudson (tjh@cryptsoft.com)"
 *
 * THIS SOFTWARE IS PROVIDED BY ERIC YOUNG ``AS IS'' AND
 * ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
 * IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE
 * ARE DISCLAIMED.  IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS BE LIABLE
 * FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
 * DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS
 * OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION)
 * HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT
 * LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY
 * OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF
 * SUCH DAMAGE.
 *
 * The licence and distribution terms for any publically available version or
 * derivative of this code cannot be changed.  i.e. this code cannot simply be
 * copied and put under another distribution licence
 * [including the GNU Public Licence.] */

#ifndef OPENSSL_HEADER_CIPHER_INTERNAL_H
#define OPENSSL_HEADER_CIPHER_INTERNAL_H

#include <openssl/base.h>

#include <openssl/aead.h>
#include <openssl/aes.h>
#include <openssl/bytestring.h>

#include "../../internal.h"
#include "../modes/internal.h"

#if defined(__cplusplus)
extern "C" {
#endif


// EVP_CIPH_MODE_MASK contains the bits of |flags| that represent the mode.
#define EVP_CIPH_MODE_MASK 0x3f

// Set of EVP_AEAD->aead_id identifiers, zero is reserved as the "unknown"
// value since it is the default for a structure. Implementations of the same
// algorithms should use the same identifier. For example, machine-optimised
// assembly versions should use the same identifier as their C counterparts.
#define AEAD_UNKNOWN_ID 0
#define AEAD_AES_128_CTR_HMAC_SHA256_ID 1
#define AEAD_AES_256_CTR_HMAC_SHA256_ID 2
#define AEAD_AES_128_GCM_SIV_ID 3
#define AEAD_AES_256_GCM_SIV_ID 4
#define AEAD_CHACHA20_POLY1305_ID 5
#define AEAD_XCHACHA20_POLY1305_ID 6
#define AEAD_AES_128_CBC_SHA1_TLS_ID 7
#define AEAD_AES_128_CBC_SHA1_TLS_IMPLICIT_IV_ID 8
#define AEAD_AES_256_CBC_SHA1_TLS_ID 9
#define AEAD_AES_256_CBC_SHA1_TLS_IMPLICIT_IV_ID 10
#define AEAD_AES_128_CBC_SHA256_TLS_ID 11
#define AEAD_AES_128_CBC_SHA256_TLS_IMPLICIT_IV_ID 12
#define AEAD_DES_EDE3_CBC_SHA1_TLS_ID 13
#define AEAD_DES_EDE3_CBC_SHA1_TLS_IMPLICIT_IV_ID 14
#define AEAD_NULL_SHA1_TLS_ID 15
#define AEAD_AES_128_GCM_ID 16
#define AEAD_AES_192_GCM_ID 17
#define AEAD_AES_256_GCM_ID 18
#define AEAD_AES_128_GCM_RANDNONCE_ID 19
#define AEAD_AES_256_GCM_RANDNONCE_ID 20
#define AEAD_AES_128_GCM_TLS12_ID 21
#define AEAD_AES_256_GCM_TLS12_ID 22
#define AEAD_AES_128_GCM_TLS13_ID 23
#define AEAD_AES_256_GCM_TLS13_ID 24
#define AEAD_AES_128_CCM_BLUETOOTH_ID 25
#define AEAD_AES_128_CCM_BLUETOOTH_8_ID 26
#define AEAD_AES_128_CCM_MATTER_ID 27
#define AEAD_AES_256_CBC_SHA384_TLS_ID 28
#define AEAD_MAX_ID 28

// EVP_AEAD represents a specific AEAD algorithm.
struct evp_aead_st {
  uint8_t key_len;
  uint8_t nonce_len;
  uint8_t overhead;
  uint8_t max_tag_len;
  uint16_t aead_id;
  int seal_scatter_supports_extra_in;

  // init initialises an |EVP_AEAD_CTX|. If this call returns zero then
  // |cleanup| will not be called for that context.
  int (*init)(EVP_AEAD_CTX *, const uint8_t *key, size_t key_len,
              size_t tag_len);
  int (*init_with_direction)(EVP_AEAD_CTX *, const uint8_t *key, size_t key_len,
                             size_t tag_len, enum evp_aead_direction_t dir);
  void (*cleanup)(EVP_AEAD_CTX *);

  int (*open)(const EVP_AEAD_CTX *ctx, uint8_t *out, size_t *out_len,
              size_t max_out_len, const uint8_t *nonce, size_t nonce_len,
              const uint8_t *in, size_t in_len, const uint8_t *ad,
              size_t ad_len);

  int (*seal_scatter)(const EVP_AEAD_CTX *ctx, uint8_t *out, uint8_t *out_tag,
                      size_t *out_tag_len, size_t max_out_tag_len,
                      const uint8_t *nonce, size_t nonce_len, const uint8_t *in,
                      size_t in_len, const uint8_t *extra_in,
                      size_t extra_in_len, const uint8_t *ad, size_t ad_len);

  int (*open_gather)(const EVP_AEAD_CTX *ctx, uint8_t *out,
                     const uint8_t *nonce, size_t nonce_len, const uint8_t *in,
                     size_t in_len, const uint8_t *in_tag, size_t in_tag_len,
                     const uint8_t *ad, size_t ad_len);

  int (*get_iv)(const EVP_AEAD_CTX *ctx, const uint8_t **out_iv,
                size_t *out_len);

  size_t (*tag_len)(const EVP_AEAD_CTX *ctx, size_t in_Len,
                    size_t extra_in_len);

  int (*serialize_state)(const EVP_AEAD_CTX *ctx, CBB *cbb);

  int (*deserialize_state)(const EVP_AEAD_CTX *ctx, CBS *cbs);
};

struct evp_cipher_st {
  // type contains a NID identifying the cipher. (e.g. NID_aes_128_gcm.)
  int nid;

  // block_size contains the block size, in bytes, of the cipher, or 1 for a
  // stream cipher.
  unsigned block_size;

  // key_len contains the key size, in bytes, for the cipher. If the cipher
  // takes a variable key size then this contains the default size.
  unsigned key_len;

  // iv_len contains the IV size, in bytes, or zero if inapplicable.
  unsigned iv_len;

  // ctx_size contains the size, in bytes, of the per-key context for this
  // cipher.
  unsigned ctx_size;

  // flags contains the OR of a number of flags. See |EVP_CIPH_*|.
  uint32_t flags;

  int (*init)(EVP_CIPHER_CTX *ctx, const uint8_t *key, const uint8_t *iv,
              int enc);

  int (*cipher)(EVP_CIPHER_CTX *ctx, uint8_t *out, const uint8_t *in,
                size_t inl);

  // cleanup, if non-NULL, releases memory associated with the context. It is
  // called if |EVP_CTRL_INIT| succeeds. Note that |init| may not have been
  // called at this point.
  void (*cleanup)(EVP_CIPHER_CTX *);

  int (*ctrl)(EVP_CIPHER_CTX *, int type, int arg, void *ptr);
};

// aes_ctr_set_key initialises |*aes_key| using |key_bytes| bytes from |key|,
// where |key_bytes| must either be 16, 24 or 32. If not NULL, |*out_block| is
// set to a function that encrypts single blocks. If not NULL, |*gcm_key| is
// initialised to do GHASH with the given key. It returns a function for
// optimised CTR-mode, or NULL if CTR-mode should be built using |*out_block|.
ctr128_f aes_ctr_set_key(AES_KEY *aes_key, GCM128_KEY *gcm_key,
                         block128_f *out_block, const uint8_t *key,
                         size_t key_bytes);


// EXPERIMENTAL functions for use in the TLS Transfer function. See
// |SSL_to_bytes| for more details.

// EVP_AEAD_CTX_serialize_state serializes the state of |ctx|,
// and writes it to |cbb|. The serialized bytes contains only the subset of data
// necessary to restore the state of an |EVP_AEAD_CTX| after initializing a new
// instance using |EVP_AEAD_CTX_init|. Function returns 1 on success or zero for
// an error.
//
// EvpAeadCtxStateSerializationVersion ::= INTEGER {v1 (1)}
//
// EvpAeadCtxState ::= SEQUENCE {
//   serializationVersion EvpAeadCtxStateSerializationVersion,
//   evpAeadCipherIdentifier INTEGER,
//   state          OCTET STRING
// }
OPENSSL_EXPORT int EVP_AEAD_CTX_serialize_state(const EVP_AEAD_CTX *ctx,
                                                CBB *cbb);

// EVP_AEAD_CTX_deserialize_state deserializes the state
// contained in |cbs|, configures the |ctx| to match. The deserialized bytes
// contains only the subset of data necessary to restore the state of an
// |EVP_AEAD_CTX| after initializing a new instance using |EVP_AEAD_CTX_init|.
// The function returns 1 on success or zero for an error.
OPENSSL_EXPORT int EVP_AEAD_CTX_deserialize_state(const EVP_AEAD_CTX *ctx,
                                                  CBS *cbs);

OPENSSL_EXPORT uint16_t EVP_AEAD_CTX_get_aead_id(const EVP_AEAD_CTX *ctx);

#if defined(__cplusplus)
}  // extern C
#endif

#endif  // OPENSSL_HEADER_CIPHER_INTERNAL_H
