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

#include <assert.h>

#include <openssl/digest.h>
#include <openssl/err.h>

#include "../../internal.h"
#include "../evp/internal.h"
#include "internal.h"


void EVP_MD_unstable_sha3_enable(bool enable) {  // no-op
}

bool EVP_MD_unstable_sha3_is_enabled(void) { return true; }

int EVP_MD_type(const EVP_MD *md) { return md->type; }

int EVP_MD_nid(const EVP_MD *md) { return EVP_MD_type(md); }

uint32_t EVP_MD_flags(const EVP_MD *md) { return md->flags; }

size_t EVP_MD_size(const EVP_MD *md) { return md->md_size; }

size_t EVP_MD_block_size(const EVP_MD *md) { return md->block_size; }


void EVP_MD_CTX_init(EVP_MD_CTX *ctx) {
  OPENSSL_memset(ctx, 0, sizeof(EVP_MD_CTX));
}

EVP_MD_CTX *EVP_MD_CTX_new(void) {
  EVP_MD_CTX *ctx = OPENSSL_zalloc(sizeof(EVP_MD_CTX));

  if (ctx) {
    // NO-OP: struct already zeroed
    //EVP_MD_CTX_init(ctx);
  }

  return ctx;
}

EVP_MD_CTX *EVP_MD_CTX_create(void) { return EVP_MD_CTX_new(); }

int EVP_MD_CTX_cleanup(EVP_MD_CTX *ctx) {
  if (ctx == NULL) {
    return 1;
  }

  OPENSSL_free(ctx->md_data);

  assert(ctx->pctx == NULL || ctx->pctx_ops != NULL);
  // |pctx| should be freed by the user of |EVP_MD_CTX| if
  // |EVP_MD_CTX_FLAG_KEEP_PKEY_CTX| is set. Everything other than the external |pctx| that |ctx->pctx| was pointing to is cleaned up when the flag is set.
  if (ctx->pctx_ops && !(ctx->flags & EVP_MD_CTX_FLAG_KEEP_PKEY_CTX)) {
    ctx->pctx_ops->free(ctx->pctx);
  }

  EVP_MD_CTX_init(ctx);

  return 1;
}

void EVP_MD_CTX_cleanse(EVP_MD_CTX *ctx) {
  if (ctx == NULL || ctx->md_data == NULL || ctx->digest == NULL) {
    return;
  }
  OPENSSL_cleanse(ctx->md_data, ctx->digest->ctx_size);
  EVP_MD_CTX_cleanup(ctx);
}

void EVP_MD_CTX_free(EVP_MD_CTX *ctx) {
  if (!ctx) {
    return;
  }

  EVP_MD_CTX_cleanup(ctx);
  OPENSSL_free(ctx);
}

void EVP_MD_CTX_destroy(EVP_MD_CTX *ctx) { EVP_MD_CTX_free(ctx); }

// EVP_DigestFinalXOF is a single-call XOF output generation function.
// The |ctx->digest| check prevents calling EVP_DigestFinalXOF consecutively. 
// To catch single-call XOF EVP_DigestFinalXOF calls after |EVP_DigestSqueeze|,
// the return |SHAKE_Final| value is used (the check is internally performed via 
// the |KECCAK1600_CTX *ctx| state flag).
int EVP_DigestFinalXOF(EVP_MD_CTX *ctx, uint8_t *out, size_t len) {
  if (ctx->digest == NULL) {
    return 0;
  }
  if ((EVP_MD_flags(ctx->digest) & EVP_MD_FLAG_XOF) == 0) {
    OPENSSL_PUT_ERROR(DIGEST, ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
    return 0;
  }
  int ok = ctx->digest->finalXOF(ctx, out, len);
  EVP_MD_CTX_cleanse(ctx);
  return ok;
}

// EVP_DigestSqueeze is a streaming XOF output squeeze function
// It can be called multiple times to generate an output of length 
// |len| bytes. 
int EVP_DigestSqueeze(EVP_MD_CTX *ctx, uint8_t *out, size_t len) {
  if (ctx->digest == NULL) {
    return 0;
  }
  if ((EVP_MD_flags(ctx->digest) & EVP_MD_FLAG_XOF) == 0) {
    OPENSSL_PUT_ERROR(DIGEST, ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
    return 0;
  }
  ctx->digest->squeezeXOF(ctx, out, len);
  return 1;
}

uint32_t EVP_MD_meth_get_flags(const EVP_MD *md) { return EVP_MD_flags(md); }

void EVP_MD_CTX_set_flags(EVP_MD_CTX *ctx, int flags) {}

int EVP_MD_CTX_copy_ex(EVP_MD_CTX *out, const EVP_MD_CTX *in) {
  // |in->digest| may be NULL if this is a signing |EVP_MD_CTX| for, e.g.,
  // Ed25519 which does not hash with |EVP_MD_CTX|.
  if (in == NULL || (in->pctx == NULL && in->digest == NULL)) {
    OPENSSL_PUT_ERROR(DIGEST, DIGEST_R_INPUT_NOT_INITIALIZED);
    return 0;
  }

  EVP_PKEY_CTX *pctx = NULL;
  assert(in->pctx == NULL || in->pctx_ops != NULL);
  if (in->pctx) {
    pctx = in->pctx_ops->dup(in->pctx);
    if (!pctx) {
      return 0;
    }
  }

  uint8_t *tmp_buf = NULL;
  if (in->digest != NULL) {
    if (out->digest != in->digest) {
      assert(in->digest->ctx_size != 0);
      tmp_buf = OPENSSL_malloc(in->digest->ctx_size);
      if (tmp_buf == NULL) {
        if (pctx) {
          in->pctx_ops->free(pctx);
        }
        return 0;
      }
    } else {
      // |md_data| will be the correct size in this case. It's removed from
      // |out| so that |EVP_MD_CTX_cleanup| doesn't free it, and then it's
      // reused.
      tmp_buf = out->md_data;
      out->md_data = NULL;
    }
  }

  EVP_MD_CTX_cleanup(out);

  out->digest = in->digest;
  out->md_data = tmp_buf;
  if (in->digest != NULL && in->md_data != NULL) {
    OPENSSL_memcpy(out->md_data, in->md_data, in->digest->ctx_size);
  }
  out->update = in->update;
  out->flags = in->flags;
  // copied |EVP_MD_CTX| should free its newly allocated |EVP_PKEY_CTX|.
  out->flags &= ~EVP_MD_CTX_FLAG_KEEP_PKEY_CTX;

  out->pctx = pctx;
  out->pctx_ops = in->pctx_ops;
  assert(out->pctx == NULL || out->pctx_ops != NULL);

  return 1;
}

void EVP_MD_CTX_move(EVP_MD_CTX *out, EVP_MD_CTX *in) {
  EVP_MD_CTX_cleanup(out);
  // While not guaranteed, |EVP_MD_CTX| is currently safe to move with |memcpy|.
  OPENSSL_memcpy(out, in, sizeof(EVP_MD_CTX));
  EVP_MD_CTX_init(in);
}

int EVP_MD_CTX_copy(EVP_MD_CTX *out, const EVP_MD_CTX *in) {
  EVP_MD_CTX_init(out);
  return EVP_MD_CTX_copy_ex(out, in);
}

int EVP_MD_CTX_reset(EVP_MD_CTX *ctx) {
  EVP_MD_CTX_cleanup(ctx);
  EVP_MD_CTX_init(ctx);
  return 1;
}

int EVP_DigestInit_ex(EVP_MD_CTX *ctx, const EVP_MD *type, ENGINE *engine) {
  if (ctx->digest != type) {
    ctx->digest = type;
    if (!used_for_hmac(ctx)) {
      assert(type->ctx_size != 0);
      ctx->update = type->update;
      uint8_t *md_data = OPENSSL_malloc(type->ctx_size);
      if (md_data == NULL) {
        return 0;
      }
      OPENSSL_free(ctx->md_data);
      ctx->md_data = md_data;
    }
  }

  assert(ctx->pctx == NULL || ctx->pctx_ops != NULL);

  if (used_for_hmac(ctx)) {
    // These configurations are specific to |EVP_PKEY_HMAC|. |HMAC_PKEY_CTX| is
    // newly allocated by |EVP_DigestSignInit| at this point. The actual key
    // data is stored in |ctx->pkey| as |HMAC_KEY|.
    if (ctx->pctx == NULL || ctx->pctx->data == NULL ||
        ctx->pctx->pkey == NULL || ctx->pctx->pkey->pkey.ptr == NULL) {
      return 0;
    }
    const HMAC_KEY *key = ctx->pctx->pkey->pkey.ptr;
    HMAC_PKEY_CTX *hmac_pctx = ctx->pctx->data;
    if (!HMAC_Init_ex(&hmac_pctx->ctx, key->key, key->key_len, hmac_pctx->md,
                      ctx->pctx->engine)) {
      return 0;
    }
    return 1;
  }

  ctx->digest->init(ctx);
  return 1;
}

int EVP_DigestInit(EVP_MD_CTX *ctx, const EVP_MD *type) {
  EVP_MD_CTX_init(ctx);
  return EVP_DigestInit_ex(ctx, type, NULL);
}

int EVP_DigestUpdate(EVP_MD_CTX *ctx, const void *data, size_t len) {
  if (ctx->update == NULL) {
    return 0;
  }
  return ctx->update(ctx, data, len);
}

int EVP_DigestFinal_ex(EVP_MD_CTX *ctx, uint8_t *md_out, unsigned int *size) {
  if (ctx->digest == NULL) {
    return 0;
  }
  if (EVP_MD_flags(ctx->digest) & EVP_MD_FLAG_XOF) {
    OPENSSL_PUT_ERROR(DIGEST, ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
    return 0;
  }

  assert(ctx->digest->md_size <= EVP_MAX_MD_SIZE);
  ctx->digest->final(ctx, md_out);
  if (size != NULL) {
    *size = ctx->digest->md_size;
  }
  OPENSSL_cleanse(ctx->md_data, ctx->digest->ctx_size);
  return 1;
}

int EVP_DigestFinal(EVP_MD_CTX *ctx, uint8_t *md, unsigned int *size) {
  int ok = EVP_DigestFinal_ex(ctx, md, size);
  EVP_MD_CTX_cleanup(ctx);
  return ok;
}

int EVP_Digest(const void *data, size_t count, uint8_t *out_md,
               unsigned int *out_size, const EVP_MD *type, ENGINE *impl) {
  EVP_MD_CTX ctx;
  int ret;

  if ((EVP_MD_flags(type) & EVP_MD_FLAG_XOF) && out_size == NULL) {
    OPENSSL_PUT_ERROR(DIGEST, ERR_R_PASSED_NULL_PARAMETER);
    return 0;
  }

  EVP_MD_CTX_init(&ctx);
  ret = EVP_DigestInit_ex(&ctx, type, impl) &&
        EVP_DigestUpdate(&ctx, data, count);
  if (ret == 0) {
    return 0;
  }

  if (EVP_MD_flags(type) & EVP_MD_FLAG_XOF) {
    ret &= EVP_DigestFinalXOF(&ctx, out_md, *out_size);
  } else {
    ret &= EVP_DigestFinal(&ctx, out_md, out_size);
  }

  return ret;
}


const EVP_MD *EVP_MD_CTX_md(const EVP_MD_CTX *ctx) {
  if (ctx == NULL) {
    return NULL;
  }
  return ctx->digest;
}

size_t EVP_MD_CTX_size(const EVP_MD_CTX *ctx) {
  return EVP_MD_size(EVP_MD_CTX_md(ctx));
}

size_t EVP_MD_CTX_block_size(const EVP_MD_CTX *ctx) {
  return EVP_MD_block_size(EVP_MD_CTX_md(ctx));
}

int EVP_MD_CTX_type(const EVP_MD_CTX *ctx) {
  return EVP_MD_type(EVP_MD_CTX_md(ctx));
}

int EVP_add_digest(const EVP_MD *digest) { return 1; }
