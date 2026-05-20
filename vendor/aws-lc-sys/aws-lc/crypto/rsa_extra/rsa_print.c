/*
 * Copyright 2006-2017 The OpenSSL Project Authors. All Rights Reserved.
 *
 * Licensed under the OpenSSL license (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#include <openssl/rsa.h>

#include <openssl/bio.h>
#include <openssl/err.h>
#include <openssl/evp.h>


int RSA_print(BIO *bio, const RSA *rsa, int indent) {
  EVP_PKEY *pkey = EVP_PKEY_new();
  int ret = pkey != NULL &&
            EVP_PKEY_set1_RSA(pkey, (RSA *)rsa) &&
            EVP_PKEY_print_private(bio, pkey, indent, NULL);
  EVP_PKEY_free(pkey);
  return ret;
}

int RSA_print_fp(FILE *fp, const RSA *rsa, int indent) {
  BIO *bio = BIO_new(BIO_s_file());
  if (bio == NULL) {
    OPENSSL_PUT_ERROR(RSA, ERR_R_BUF_LIB);
    return 0;
  }
  BIO_set_fp(bio, fp, BIO_NOCLOSE);
  int ret = RSA_print(bio, rsa, indent);
  BIO_free(bio);
  return ret;
}
