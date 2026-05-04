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

#include <openssl/digest.h>

#include <assert.h>
#include <string.h>

#include <openssl/md4.h>
#include <openssl/md5.h>
#include <openssl/nid.h>
#include <openssl/ripemd.h>
#include <openssl/sha.h>

#include "../../internal.h"
#include "../sha/internal.h"
#include "internal.h"

#if defined(NDEBUG)
#define CHECK(x) (void) (x)
#else
#define CHECK(x) assert(x)
#endif


static void md4_init(EVP_MD_CTX *ctx) {
  CHECK(MD4_Init(ctx->md_data));
}

static int md4_update(EVP_MD_CTX *ctx, const void *data, size_t count) {
  // MD4_Update always returns 1. Internally called function
  // |crypto_md32_update| is void. For test consistency and future
  // compatibility, the return value is propagated and returned
  return MD4_Update(ctx->md_data, data, count);
}

static void md4_final(EVP_MD_CTX *ctx, uint8_t *out) {
  CHECK(MD4_Final(out, ctx->md_data));
}

DEFINE_METHOD_FUNCTION(EVP_MD, EVP_md4) {
  out->type = NID_md4;
  out->md_size = MD4_DIGEST_LENGTH;
  out->flags = 0;
  out->init = md4_init;
  out->update = md4_update;
  out->final = md4_final;
  out->squeezeXOF = NULL;
  out->finalXOF = NULL;
  out->block_size = 64;
  out->ctx_size = sizeof(MD4_CTX);
}


static void md5_init(EVP_MD_CTX *ctx) {
  CHECK(MD5_Init(ctx->md_data));
}

static int md5_update(EVP_MD_CTX *ctx, const void *data, size_t count) {
  // MD5_Update always returns 1. Internally called function
  // |crypto_md32_update| is void. For test consistency and future
  // compatibility, the return value is propagated and returned
  return MD5_Update(ctx->md_data, data, count);
}

static void md5_final(EVP_MD_CTX *ctx, uint8_t *out) {
  CHECK(MD5_Final(out, ctx->md_data));
}

DEFINE_METHOD_FUNCTION(EVP_MD, EVP_md5) {
  out->type = NID_md5;
  out->md_size = MD5_DIGEST_LENGTH;
  out->flags = 0;
  out->init = md5_init;
  out->update = md5_update;
  out->final = md5_final;
  out->squeezeXOF = NULL;
  out->finalXOF = NULL;
  out->block_size = 64;
  out->ctx_size = sizeof(MD5_CTX);
}


static void ripemd160_init(EVP_MD_CTX *ctx) {
  CHECK(RIPEMD160_Init(ctx->md_data));
}

static int ripemd160_update(EVP_MD_CTX *ctx, const void *data, size_t count) {
  // RIPEMD160_Update always returns 1. Internally called function
  // |crypto_md32_update| is void. For test consistency and future
  // compatibility, the return value is propagated and returned
  return RIPEMD160_Update(ctx->md_data, data, count);
}

static void ripemd160_final(EVP_MD_CTX *ctx, uint8_t *out) {
  CHECK(RIPEMD160_Final(out, ctx->md_data));
}

DEFINE_METHOD_FUNCTION(EVP_MD, EVP_ripemd160) {
  out->type = NID_ripemd160;
  out->md_size = RIPEMD160_DIGEST_LENGTH;
  out->flags = 0;
  out->init = ripemd160_init;
  out->update = ripemd160_update;
  out->final = ripemd160_final;
  out->squeezeXOF = NULL;
  out->finalXOF = NULL;
  out->block_size = 64;
  out->ctx_size = sizeof(RIPEMD160_CTX);
}


static void sha1_init(EVP_MD_CTX *ctx) {
  CHECK(SHA1_Init(ctx->md_data));
}

static int sha1_update(EVP_MD_CTX *ctx, const void *data, size_t count) {
  // SHA1_Update always returns 1. Internally called function
  // |crypto_md32_update| is void. For test consistency and future
  // compatibility, the return value is propagated and returned
  return SHA1_Update(ctx->md_data, data, count);
}

static void sha1_final(EVP_MD_CTX *ctx, uint8_t *md) {
  CHECK(SHA1_Final(md, ctx->md_data));
}

DEFINE_METHOD_FUNCTION(EVP_MD, EVP_sha1) {
  out->type = NID_sha1;
  out->md_size = SHA_DIGEST_LENGTH;
  out->flags = 0;
  out->init = sha1_init;
  out->update = sha1_update;
  out->final = sha1_final;
  out->squeezeXOF = NULL;
  out->finalXOF = NULL;
  out->block_size = 64;
  out->ctx_size = sizeof(SHA_CTX);
}


static void sha224_init(EVP_MD_CTX *ctx) {
  CHECK(SHA224_Init(ctx->md_data));
}

static int sha224_update(EVP_MD_CTX *ctx, const void *data, size_t count) {
  // SHA224_Update always returns 1. Internally called function
  // |crypto_md32_update| through |SHA256_Update| is void. For test consistency
  // and future compatibility, the return value is propagated and returned
  return SHA224_Update(ctx->md_data, data, count);
}

static void sha224_final(EVP_MD_CTX *ctx, uint8_t *md) {
  CHECK(SHA224_Final(md, ctx->md_data));
}

DEFINE_METHOD_FUNCTION(EVP_MD, EVP_sha224) {
  out->type = NID_sha224;
  out->md_size = SHA224_DIGEST_LENGTH;
  out->flags = 0;
  out->init = sha224_init;
  out->update = sha224_update;
  out->final = sha224_final;
  out->squeezeXOF = NULL;
  out->finalXOF = NULL;
  out->block_size = 64;
  out->ctx_size = sizeof(SHA256_CTX);
}


static void sha256_init(EVP_MD_CTX *ctx) {
  CHECK(SHA256_Init(ctx->md_data));
}

static int sha256_update(EVP_MD_CTX *ctx, const void *data, size_t count) {
  // SHA256_Update always returns 1. Internally called function
  // |crypto_md32_update| is void. For test consistency and future
  // compatibility, the return value is propagated and returned
  return SHA256_Update(ctx->md_data, data, count);
}

static void sha256_final(EVP_MD_CTX *ctx, uint8_t *md) {
  CHECK(SHA256_Final(md, ctx->md_data));
}

DEFINE_METHOD_FUNCTION(EVP_MD, EVP_sha256) {
  out->type = NID_sha256;
  out->md_size = SHA256_DIGEST_LENGTH;
  out->flags = 0;
  out->init = sha256_init;
  out->update = sha256_update;
  out->final = sha256_final;
  out->squeezeXOF = NULL;
  out->finalXOF = NULL;
  out->block_size = 64;
  out->ctx_size = sizeof(SHA256_CTX);
}


static void sha384_init(EVP_MD_CTX *ctx) {
  CHECK(SHA384_Init(ctx->md_data));
}

static int sha384_update(EVP_MD_CTX *ctx, const void *data, size_t count) {
  // SHA384_Update always returns 1. Internally called function
  // |SHA512_Update| always returns 1. For test consistency
  // and future compatibility, the return value is propagated and returned
  return SHA384_Update(ctx->md_data, data, count);
}

static void sha384_final(EVP_MD_CTX *ctx, uint8_t *md) {
  CHECK(SHA384_Final(md, ctx->md_data));
}

DEFINE_METHOD_FUNCTION(EVP_MD, EVP_sha384) {
  out->type = NID_sha384;
  out->md_size = SHA384_DIGEST_LENGTH;
  out->flags = 0;
  out->init = sha384_init;
  out->update = sha384_update;
  out->final = sha384_final;
  out->squeezeXOF = NULL;
  out->finalXOF = NULL;
  out->block_size = 128;
  out->ctx_size = sizeof(SHA512_CTX);
}


static void sha512_init(EVP_MD_CTX *ctx) {
  CHECK(SHA512_Init(ctx->md_data));
}

static int sha512_update(EVP_MD_CTX *ctx, const void *data, size_t count) {
  // SHA512_Update always returns 1. For test consistency
  // and future compatibility, the return value is propagated and returned
  return SHA512_Update(ctx->md_data, data, count);
}

static void sha512_final(EVP_MD_CTX *ctx, uint8_t *md) {
  CHECK(SHA512_Final(md, ctx->md_data));
}

DEFINE_METHOD_FUNCTION(EVP_MD, EVP_sha512) {
  out->type = NID_sha512;
  out->md_size = SHA512_DIGEST_LENGTH;
  out->flags = 0;
  out->init = sha512_init;
  out->update = sha512_update;
  out->final = sha512_final;
  out->squeezeXOF = NULL;
  out->finalXOF = NULL;
  out->block_size = 128;
  out->ctx_size = sizeof(SHA512_CTX);
}


static void sha512_224_init(EVP_MD_CTX *ctx) {
  CHECK(SHA512_224_Init(ctx->md_data));
}

static int sha512_224_update(EVP_MD_CTX *ctx, const void *data, size_t count) {
  // SHA512_224_Update always returns 1. Internally called function
  // |SHA512_Update| always returns 1. For test consistency
  // and future compatibility, the return value is propagated and returned
  return SHA512_224_Update(ctx->md_data, data, count);
}

static void sha512_224_final(EVP_MD_CTX *ctx, uint8_t *md) {
  CHECK(SHA512_224_Final(md, ctx->md_data));
}

DEFINE_METHOD_FUNCTION(EVP_MD, EVP_sha512_224) {
  out->type = NID_sha512_224;
  out->md_size = SHA512_224_DIGEST_LENGTH;
  out->flags = 0;
  out->init = sha512_224_init;
  out->update = sha512_224_update;
  out->final = sha512_224_final;
  out->block_size = 128;
  out->ctx_size = sizeof(SHA512_CTX);
}


static void sha512_256_init(EVP_MD_CTX *ctx) {
  CHECK(SHA512_256_Init(ctx->md_data));
}

static int sha512_256_update(EVP_MD_CTX *ctx, const void *data, size_t count) {
  // SHA512_256_Update always returns 1. Internally called function
  // |SHA512_Update| always returns 1. For test consistency
  // and future compatibility, the return value is propagated and returned
  return SHA512_256_Update(ctx->md_data, data, count);
}

static void sha512_256_final(EVP_MD_CTX *ctx, uint8_t *md) {
  CHECK(SHA512_256_Final(md, ctx->md_data));
}

DEFINE_METHOD_FUNCTION(EVP_MD, EVP_sha512_256) {
  out->type = NID_sha512_256;
  out->md_size = SHA512_256_DIGEST_LENGTH;
  out->flags = 0;
  out->init = sha512_256_init;
  out->update = sha512_256_update;
  out->final = sha512_256_final;
  out->squeezeXOF = NULL;
  out->finalXOF = NULL;
  out->block_size = 128;
  out->ctx_size = sizeof(SHA512_CTX);
}


static void sha3_224_init(EVP_MD_CTX *ctx) {
  CHECK(SHA3_Init(ctx->md_data, SHA3_224_DIGEST_BITLENGTH));
}

static int sha3_224_update(EVP_MD_CTX *ctx, const void *data, size_t count) {
  // SHA3_Update returns 1 on success and 0 on failure.
  return SHA3_Update(ctx->md_data, data, count);
}

static void sha3_224_final(EVP_MD_CTX *ctx, uint8_t *md) {
  CHECK(SHA3_Final(md, ctx->md_data));
}

DEFINE_METHOD_FUNCTION(EVP_MD, EVP_sha3_224) {
  out->type = NID_sha3_224;
  out->md_size = SHA3_224_DIGEST_LENGTH;
  out->flags = 0;
  out->init = sha3_224_init;
  out->update = sha3_224_update;
  out->final = sha3_224_final;
  out->squeezeXOF = NULL;
  out->finalXOF = NULL;
  out->block_size = SHA3_BLOCKSIZE(SHA3_224_DIGEST_BITLENGTH);
  out->ctx_size = sizeof(KECCAK1600_CTX);
}


static void sha3_256_init(EVP_MD_CTX *ctx) {
  CHECK(SHA3_Init(ctx->md_data, SHA3_256_DIGEST_BITLENGTH));
}

static int sha3_256_update(EVP_MD_CTX *ctx, const void *data, size_t count) {
  // SHA3_Update returns 1 on success and 0 on failure.
  return SHA3_Update(ctx->md_data, data, count);
}

static void sha3_256_final(EVP_MD_CTX *ctx, uint8_t *md) {
  CHECK(SHA3_Final(md, ctx->md_data));
}

DEFINE_METHOD_FUNCTION(EVP_MD, EVP_sha3_256) {
  out->type = NID_sha3_256;
  out->md_size = SHA3_256_DIGEST_LENGTH;
  out->flags = 0;
  out->init = sha3_256_init;
  out->update = sha3_256_update;
  out->final = sha3_256_final;
  out->squeezeXOF = NULL;
  out->finalXOF = NULL;
  out->block_size = SHA3_BLOCKSIZE(SHA3_256_DIGEST_BITLENGTH);
  out->ctx_size = sizeof(KECCAK1600_CTX);
}


static void sha3_384_init(EVP_MD_CTX *ctx) {
  CHECK(SHA3_Init(ctx->md_data, SHA3_384_DIGEST_BITLENGTH));
}

static int sha3_384_update(EVP_MD_CTX *ctx, const void *data, size_t count) {
  // SHA3_Update returns 1 on success and 0 on failure.
  return SHA3_Update(ctx->md_data, data, count);
}

static void sha3_384_final(EVP_MD_CTX *ctx, uint8_t *md) {
  CHECK(SHA3_Final(md, ctx->md_data));
}

DEFINE_METHOD_FUNCTION(EVP_MD, EVP_sha3_384) {
  out->type = NID_sha3_384;
  out->md_size = SHA3_384_DIGEST_LENGTH;
  out->flags = 0;
  out->init = sha3_384_init;
  out->update = sha3_384_update;
  out->final = sha3_384_final;
  out->squeezeXOF = NULL;
  out->finalXOF = NULL;
  out->block_size = SHA3_BLOCKSIZE(SHA3_384_DIGEST_BITLENGTH);
  out->ctx_size = sizeof(KECCAK1600_CTX);
}


static void sha3_512_init(EVP_MD_CTX *ctx) {
  CHECK(SHA3_Init(ctx->md_data, SHA3_512_DIGEST_BITLENGTH));
}

static int sha3_512_update(EVP_MD_CTX *ctx, const void *data, size_t count) {
  // SHA3_Update returns 1 on success and 0 on failure.
  return SHA3_Update(ctx->md_data, data, count);
}

static void sha3_512_final(EVP_MD_CTX *ctx, uint8_t *md) {
  CHECK(SHA3_Final(md, ctx->md_data));
}

DEFINE_METHOD_FUNCTION(EVP_MD, EVP_sha3_512) {
  out->type = NID_sha3_512;
  out->md_size = SHA3_512_DIGEST_LENGTH;
  out->flags = 0;
  out->init = sha3_512_init;
  out->update = sha3_512_update;
  out->final = sha3_512_final;
  out->squeezeXOF = NULL;
  out->finalXOF = NULL;
  out->block_size = SHA3_BLOCKSIZE(SHA3_512_DIGEST_BITLENGTH);
  out->ctx_size = sizeof(KECCAK1600_CTX);
}


static void shake128_init(EVP_MD_CTX *ctx) {
  CHECK(SHAKE_Init(ctx->md_data, SHAKE128_BLOCKSIZE));
}

// shake128_update returns 1 on success and 0 on failure, returned 
// from |SHAKE_Absorb|, to restrict update calls after |squeezeXOF|.
static int shake128_update(EVP_MD_CTX *ctx, const void *data, size_t count) {
  return SHAKE_Absorb(ctx->md_data, data, count);
}

// shake128_final returns 1 on success and 0 on failure, 
// returned from |SHAKE_Final|, to restrict single-call SHAKE_Final 
// calls after |squeezeXOF|.
static int shake128_final(EVP_MD_CTX *ctx, uint8_t *md, size_t len) {
  return SHAKE_Final(md, ctx->md_data, len);
}

static void shake128_squeeze(EVP_MD_CTX *ctx, uint8_t *md, size_t len) {
  CHECK(SHAKE_Squeeze(md, ctx->md_data, len));
}

DEFINE_METHOD_FUNCTION(EVP_MD, EVP_shake128) {
  out->type = NID_shake128;
  out->md_size = 0;
  out->flags = EVP_MD_FLAG_XOF;
  out->init = shake128_init;
  out->update = shake128_update;
  out->final = NULL;
  out->squeezeXOF = shake128_squeeze;
  out->finalXOF = shake128_final;
  out->block_size = SHAKE128_BLOCKSIZE;
  out->ctx_size = sizeof(KECCAK1600_CTX);
}

static void shake256_init(EVP_MD_CTX *ctx) {
  CHECK(SHAKE_Init(ctx->md_data, SHAKE256_BLOCKSIZE));
}

// shake256_update returns 1 on success and 0 on failure, returned 
// from |SHAKE_Absorb|, to restrict update calls after |squeezeXOF|.
static int shake256_update(EVP_MD_CTX *ctx, const void *data, size_t count) {
  return SHAKE_Absorb(ctx->md_data, data, count);
}

// shake256_final returns 1 on success and 0 on failure, 
// returned from |SHAKE_Final|, to restrict single-call SHAKE_Final 
// calls after |squeezeXOF|.
static int shake256_final(EVP_MD_CTX *ctx, uint8_t *md, size_t len) {
  return SHAKE_Final(md, ctx->md_data, len);
}

static void shake256_squeeze(EVP_MD_CTX *ctx, uint8_t *md, size_t len) {
  CHECK(SHAKE_Squeeze(md, ctx->md_data, len));
}

DEFINE_METHOD_FUNCTION(EVP_MD, EVP_shake256) {
  out->type = NID_shake256;
  out->md_size = 0;
  out->flags = EVP_MD_FLAG_XOF;
  out->init = shake256_init;
  out->update = shake256_update;
  out->final = NULL;
  out->squeezeXOF = shake256_squeeze;
  out->finalXOF = shake256_final;
  out->block_size = SHAKE256_BLOCKSIZE;
  out->ctx_size = sizeof(KECCAK1600_CTX);
}

typedef struct {
  MD5_CTX md5;
  SHA_CTX sha1;
} MD5_SHA1_CTX;

static void md5_sha1_init(EVP_MD_CTX *md_ctx) {
  MD5_SHA1_CTX *ctx = md_ctx->md_data;
  CHECK(MD5_Init(&ctx->md5) && SHA1_Init(&ctx->sha1));
}

static int md5_sha1_update(EVP_MD_CTX *md_ctx, const void *data,
                            size_t count) {
  MD5_SHA1_CTX *ctx = md_ctx->md_data;
  // MD5_Update and SHA1_Update always return 1. Internally called function
  // |crypto_md32_update| always returns 1. For test consistency
  // and future compatibility, the return value is propagated and returned
  int ok = MD5_Update(&ctx->md5, data, count) &&
        SHA1_Update(&ctx->sha1, data, count);
  return ok;
}

static void md5_sha1_final(EVP_MD_CTX *md_ctx, uint8_t *out) {
  MD5_SHA1_CTX *ctx = md_ctx->md_data;
  CHECK(MD5_Final(out, &ctx->md5) &&
        SHA1_Final(out + MD5_DIGEST_LENGTH, &ctx->sha1));
}

DEFINE_METHOD_FUNCTION(EVP_MD, EVP_md5_sha1) {
  out->type = NID_md5_sha1;
  out->md_size = MD5_DIGEST_LENGTH + SHA_DIGEST_LENGTH;
  out->flags = 0;
  out->init = md5_sha1_init;
  out->update = md5_sha1_update;
  out->final = md5_sha1_final;
  out->squeezeXOF = NULL;
  out->finalXOF = NULL;
  out->block_size = 64;
  out->ctx_size = sizeof(MD5_SHA1_CTX);
}

#undef CHECK
