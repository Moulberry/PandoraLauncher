// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0 OR ISC

#include <openssl/pkcs7.h>

#include <openssl/asn1t.h>
#include <openssl/pem.h>

#include "../bytestring/internal.h"
#include "../internal.h"
#include "internal.h"

ASN1_ADB_TEMPLATE(p7default) = ASN1_EXP_OPT(PKCS7, d.other, ASN1_ANY, 0);

ASN1_ADB(PKCS7) = {
    ADB_ENTRY(NID_pkcs7_data,
              ASN1_EXP_OPT(PKCS7, d.data, ASN1_OCTET_STRING, 0)),
    ADB_ENTRY(NID_pkcs7_signed, ASN1_EXP_OPT(PKCS7, d.sign, PKCS7_SIGNED, 0)),
    ADB_ENTRY(NID_pkcs7_enveloped,
              ASN1_EXP_OPT(PKCS7, d.enveloped, PKCS7_ENVELOPE, 0)),
    ADB_ENTRY(NID_pkcs7_signedAndEnveloped,
              ASN1_EXP_OPT(PKCS7, d.signed_and_enveloped, PKCS7_SIGN_ENVELOPE,
                           0)),
    ADB_ENTRY(NID_pkcs7_digest, ASN1_EXP_OPT(PKCS7, d.digest, PKCS7_DIGEST, 0)),
    ADB_ENTRY(
        NID_pkcs7_encrypted,
        ASN1_EXP_OPT(PKCS7, d.encrypted, PKCS7_ENCRYPT,
                     0))} ASN1_ADB_END(PKCS7, 0, type, 0, &p7default_tt, &p7default_tt);

ASN1_SEQUENCE(PKCS7) = {ASN1_SIMPLE(PKCS7, type, ASN1_OBJECT),
                        ASN1_ADB_OBJECT(PKCS7)} ASN1_SEQUENCE_END(PKCS7)

IMPLEMENT_ASN1_ALLOC_FUNCTIONS(PKCS7)

PKCS7 *d2i_PKCS7(PKCS7 **a, const unsigned char **in, long len) {
  uint8_t *der_bytes = NULL;
  PKCS7 *ret = NULL;
  CBS cbs, cbs_der;

  if (!in) {
    return NULL;
  }

  CBS_init(&cbs, *in, len);
  // |CBS_asn1_ber_to_der| will allocate memory and point |der_bytes| to it.
  // we're responsible for freeing this below.
  if (!CBS_asn1_ber_to_der(&cbs, &cbs_der, &der_bytes)) {
    goto err;
  }

  // |CBS_asn1_ber_to_der| will set |der_bytes| to NULL if it doesn't detect
  // any convertible BER elements in |in|.
  if (der_bytes == NULL) {
    ret = (PKCS7 *)ASN1_item_d2i((ASN1_VALUE **)a, in, len,
                                 ASN1_ITEM_rptr(PKCS7));
  } else {
    // |ASN1_item_d2i| will increment the input pointer by |der_len| length, so
    // save off another pointer so we can free |der_bytes| at the end of this
    // function.
    uint8_t *der_bytes_ptr = der_bytes;
    size_t der_len = CBS_len(&cbs_der);
    ret = (PKCS7 *)ASN1_item_d2i((ASN1_VALUE **)a,
                                 (const uint8_t **)&der_bytes_ptr, der_len,
                                 ASN1_ITEM_rptr(PKCS7));
    // Advance |*in| by however many bytes |ASN1_item_d2i| advanced
    // |der_bytes_ptr|
    *in += der_bytes_ptr - der_bytes;
  }

err:
  OPENSSL_free(der_bytes);
  der_bytes = NULL;
  return ret;
}

int i2d_PKCS7(PKCS7 *a, unsigned char **out) {
  return ASN1_item_i2d((ASN1_VALUE *)a, out, ASN1_ITEM_rptr(PKCS7));
}

IMPLEMENT_ASN1_DUP_FUNCTION(PKCS7)

ASN1_SEQUENCE(PKCS7_SIGNED) = {
    ASN1_SIMPLE(PKCS7_SIGNED, version, ASN1_INTEGER),
    ASN1_SET_OF(PKCS7_SIGNED, md_algs, X509_ALGOR),
    ASN1_SIMPLE(PKCS7_SIGNED, contents, PKCS7),
    ASN1_IMP_SEQUENCE_OF_OPT(PKCS7_SIGNED, cert, X509, 0),
    ASN1_IMP_SET_OF_OPT(PKCS7_SIGNED, crl, X509_CRL, 1),
    ASN1_SET_OF(PKCS7_SIGNED, signer_info,
                PKCS7_SIGNER_INFO)} ASN1_SEQUENCE_END(PKCS7_SIGNED)

IMPLEMENT_ASN1_FUNCTIONS(PKCS7_SIGNED)

ASN1_SEQUENCE(PKCS7_ISSUER_AND_SERIAL) = {
    ASN1_SIMPLE(PKCS7_ISSUER_AND_SERIAL, issuer, X509_NAME),
    ASN1_SIMPLE(PKCS7_ISSUER_AND_SERIAL, serial,
                ASN1_INTEGER)} ASN1_SEQUENCE_END(PKCS7_ISSUER_AND_SERIAL)

IMPLEMENT_ASN1_FUNCTIONS(PKCS7_ISSUER_AND_SERIAL)

// Minor tweak to operation: free up X509.
static int recip_info_cb(int operation, ASN1_VALUE **pval, const ASN1_ITEM *it,
                         void *exarg) {
  if (operation == ASN1_OP_FREE_POST) {
    PKCS7_RECIP_INFO *ri = (PKCS7_RECIP_INFO *)*pval;
    X509_free(ri->cert);
  }
  return 1;
}

ASN1_SEQUENCE_cb(PKCS7_RECIP_INFO, recip_info_cb) =
    {ASN1_SIMPLE(PKCS7_RECIP_INFO, version, ASN1_INTEGER),
     ASN1_SIMPLE(PKCS7_RECIP_INFO, issuer_and_serial, PKCS7_ISSUER_AND_SERIAL),
     ASN1_SIMPLE(PKCS7_RECIP_INFO, key_enc_algor, X509_ALGOR),
     ASN1_SIMPLE(PKCS7_RECIP_INFO, enc_key,
                 ASN1_OCTET_STRING)} ASN1_SEQUENCE_END_cb(PKCS7_RECIP_INFO,
                                                          PKCS7_RECIP_INFO)

IMPLEMENT_ASN1_FUNCTIONS(PKCS7_RECIP_INFO)

// Minor tweak to operation: free up EVP_PKEY.
static int signer_info_cb(int operation, ASN1_VALUE **pval, const ASN1_ITEM *it,
                          void *exarg) {
  PKCS7_SIGNER_INFO *si = (PKCS7_SIGNER_INFO *)*pval;
  if (operation == ASN1_OP_FREE_POST) {
    EVP_PKEY_free(si->pkey);
  }
  return 1;
}

ASN1_SEQUENCE_cb(PKCS7_SIGNER_INFO, signer_info_cb) = {
    ASN1_SIMPLE(PKCS7_SIGNER_INFO, version, ASN1_INTEGER),
    ASN1_SIMPLE(PKCS7_SIGNER_INFO, issuer_and_serial, PKCS7_ISSUER_AND_SERIAL),
    ASN1_SIMPLE(PKCS7_SIGNER_INFO, digest_alg, X509_ALGOR),
    // NB this should be a SET OF but we use a SEQUENCE OF so the original
    // order is retained when the structure is reencoded. Since the attributes
    // are implicitly tagged this will not affect the encoding.
    ASN1_IMP_SEQUENCE_OF_OPT(PKCS7_SIGNER_INFO, auth_attr, X509_ATTRIBUTE, 0),
    ASN1_SIMPLE(PKCS7_SIGNER_INFO, digest_enc_alg, X509_ALGOR),
    ASN1_SIMPLE(PKCS7_SIGNER_INFO, enc_digest, ASN1_OCTET_STRING),
    ASN1_IMP_SET_OF_OPT(
        PKCS7_SIGNER_INFO, unauth_attr, X509_ATTRIBUTE,
        1)} ASN1_SEQUENCE_END_cb(PKCS7_SIGNER_INFO, PKCS7_SIGNER_INFO)

IMPLEMENT_ASN1_FUNCTIONS(PKCS7_SIGNER_INFO)

ASN1_SEQUENCE(PKCS7_ENC_CONTENT) = {
    ASN1_SIMPLE(PKCS7_ENC_CONTENT, content_type, ASN1_OBJECT),
    ASN1_SIMPLE(PKCS7_ENC_CONTENT, algorithm, X509_ALGOR),
    ASN1_IMP_OPT(PKCS7_ENC_CONTENT, enc_data, ASN1_OCTET_STRING,
                 0)} ASN1_SEQUENCE_END(PKCS7_ENC_CONTENT)

IMPLEMENT_ASN1_FUNCTIONS(PKCS7_ENC_CONTENT)

ASN1_SEQUENCE(PKCS7_SIGN_ENVELOPE) = {
    ASN1_SIMPLE(PKCS7_SIGN_ENVELOPE, version, ASN1_INTEGER),
    ASN1_SET_OF(PKCS7_SIGN_ENVELOPE, recipientinfo, PKCS7_RECIP_INFO),
    ASN1_SET_OF(PKCS7_SIGN_ENVELOPE, md_algs, X509_ALGOR),
    ASN1_SIMPLE(PKCS7_SIGN_ENVELOPE, enc_data, PKCS7_ENC_CONTENT),
    ASN1_IMP_SET_OF_OPT(PKCS7_SIGN_ENVELOPE, cert, X509, 0),
    ASN1_IMP_SET_OF_OPT(PKCS7_SIGN_ENVELOPE, crl, X509_CRL, 1),
    ASN1_SET_OF(PKCS7_SIGN_ENVELOPE, signer_info,
                PKCS7_SIGNER_INFO)} ASN1_SEQUENCE_END(PKCS7_SIGN_ENVELOPE)

IMPLEMENT_ASN1_FUNCTIONS(PKCS7_SIGN_ENVELOPE)

ASN1_SEQUENCE(PKCS7_ENCRYPT) = {
    ASN1_SIMPLE(PKCS7_ENCRYPT, version, ASN1_INTEGER),
    ASN1_SIMPLE(PKCS7_ENCRYPT, enc_data,
                PKCS7_ENC_CONTENT)} ASN1_SEQUENCE_END(PKCS7_ENCRYPT)

IMPLEMENT_ASN1_FUNCTIONS(PKCS7_ENCRYPT)

ASN1_SEQUENCE(PKCS7_DIGEST) = {
    ASN1_SIMPLE(PKCS7_DIGEST, version, ASN1_INTEGER),
    ASN1_SIMPLE(PKCS7_DIGEST, digest_alg, X509_ALGOR),
    ASN1_SIMPLE(PKCS7_DIGEST, contents, PKCS7),
    ASN1_SIMPLE(PKCS7_DIGEST, digest,
                ASN1_OCTET_STRING)} ASN1_SEQUENCE_END(PKCS7_DIGEST)

IMPLEMENT_ASN1_FUNCTIONS(PKCS7_DIGEST)

ASN1_SEQUENCE(PKCS7_ENVELOPE) = {
    ASN1_SIMPLE(PKCS7_ENVELOPE, version, ASN1_INTEGER),
    ASN1_SET_OF(PKCS7_ENVELOPE, recipientinfo, PKCS7_RECIP_INFO),
    ASN1_SIMPLE(PKCS7_ENVELOPE, enc_data,
                PKCS7_ENC_CONTENT)} ASN1_SEQUENCE_END(PKCS7_ENVELOPE)

IMPLEMENT_ASN1_FUNCTIONS(PKCS7_ENVELOPE)

ASN1_ITEM_TEMPLATE(PKCS7_ATTR_VERIFY) = ASN1_EX_TEMPLATE_TYPE(
    ASN1_TFLG_SEQUENCE_OF | ASN1_TFLG_IMPTAG | ASN1_TFLG_UNIVERSAL, V_ASN1_SET,
    PKCS7_ATTRIBUTES, X509_ATTRIBUTE)
ASN1_ITEM_TEMPLATE_END(PKCS7_ATTR_VERIFY)

int PKCS7_print_ctx(BIO *bio, PKCS7 *pkcs7, int indent, const ASN1_PCTX *pctx) {
  GUARD_PTR(bio);
  GUARD_PTR(pkcs7);

  if (BIO_printf(bio, "PKCS7 printing is not supported") <= 0) {
    return 0;
  }
  return 1;
}
