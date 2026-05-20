// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0 OR ISC

#include <openssl/evp.h>

#include <openssl/bytestring.h>
#include <openssl/err.h>
#include <openssl/mem.h>

#include "../crypto/fipsmodule/pqdsa/internal.h"
#include "../crypto/internal.h"
#include "../fipsmodule/evp/internal.h"
#include "../fipsmodule/ml_dsa/ml_dsa.h"
#include "internal.h"

static void pqdsa_free(EVP_PKEY *pkey) {
  PQDSA_KEY_free(pkey->pkey.pqdsa_key);
  pkey->pkey.pqdsa_key = NULL;
}

static int pqdsa_get_priv_raw(const EVP_PKEY *pkey, uint8_t *out,
                                   size_t *out_len) {
  GUARD_PTR(pkey);
  GUARD_PTR(out_len);

  if (pkey->pkey.pqdsa_key == NULL) {
    OPENSSL_PUT_ERROR(EVP, EVP_R_NO_PARAMETERS_SET);
    return 0;
  }

  PQDSA_KEY *key = pkey->pkey.pqdsa_key;
  const PQDSA *pqdsa = key->pqdsa;

  if (key->private_key == NULL) {
    OPENSSL_PUT_ERROR(EVP, EVP_R_NOT_A_PRIVATE_KEY);
    return 0;
  }

  if (pqdsa == NULL) {
    OPENSSL_PUT_ERROR(EVP, EVP_R_NO_PARAMETERS_SET);
    return 0;
  }

  if (out == NULL) {
    *out_len = key->pqdsa->private_key_len;
    return 1;
  }

  if (*out_len < key->pqdsa->private_key_len) {
    OPENSSL_PUT_ERROR(EVP, EVP_R_BUFFER_TOO_SMALL);
    return 0;
  }

  OPENSSL_memcpy(out, key->private_key, pqdsa->private_key_len);
  *out_len = pqdsa->private_key_len;
  return 1;
}

static int pqdsa_get_pub_raw(const EVP_PKEY *pkey, uint8_t *out,
                                  size_t *out_len) {
  if (pkey->pkey.pqdsa_key == NULL) {
    OPENSSL_PUT_ERROR(EVP, EVP_R_NO_PARAMETERS_SET);
    return 0;
  }

  PQDSA_KEY *key = pkey->pkey.pqdsa_key;
  const PQDSA *pqdsa = key->pqdsa;

  if (pqdsa == NULL) {
    OPENSSL_PUT_ERROR(EVP, EVP_R_NO_PARAMETERS_SET);
    return 0;
  }

  if (key->public_key == NULL) {
    OPENSSL_PUT_ERROR(EVP, EVP_R_OPERATION_NOT_SUPPORTED_FOR_THIS_KEYTYPE);
    return 0;
  }

  if (out == NULL) {
    *out_len = pqdsa->public_key_len;
    return 1;
  }

  if (*out_len < key->pqdsa->public_key_len) {
    OPENSSL_PUT_ERROR(EVP, EVP_R_BUFFER_TOO_SMALL);
    return 0;
  }

  OPENSSL_memcpy(out, key->public_key, pqdsa->public_key_len);
  *out_len = pqdsa->public_key_len;
  return 1;
}

static int pqdsa_pub_decode(EVP_PKEY *out, CBS *oid, CBS *params, CBS *key) {
  // See https://datatracker.ietf.org/doc/draft-ietf-lamps-dilithium-certificates/
  // section 4. There should be no parameters
  if (CBS_len(params) > 0) {
    OPENSSL_PUT_ERROR(EVP, EVP_R_DECODE_ERROR);
    return 0;
  }
  // Set the pqdsa params on |out|.
  if (!EVP_PKEY_pqdsa_set_params(out, OBJ_cbs2nid(oid))) {
    OPENSSL_PUT_ERROR(EVP, EVP_R_DECODE_ERROR);
    return 0;
  }
  return PQDSA_KEY_set_raw_public_key(out->pkey.pqdsa_key, key);
}

static int pqdsa_pub_encode(CBB *out, const EVP_PKEY *pkey) {
  PQDSA_KEY *key = pkey->pkey.pqdsa_key;
  const PQDSA *pqdsa = key->pqdsa;
  if (key->public_key == NULL) {
    OPENSSL_PUT_ERROR(EVP, EVP_R_OPERATION_NOT_SUPPORTED_FOR_THIS_KEYTYPE);
    return 0;
  }

  // See https://datatracker.ietf.org/doc/draft-ietf-lamps-dilithium-certificates/ section 4.
  CBB spki, algorithm, oid, key_bitstring;
  if (!CBB_add_asn1(out, &spki, CBS_ASN1_SEQUENCE) ||
      !CBB_add_asn1(&spki, &algorithm, CBS_ASN1_SEQUENCE) ||
      !CBB_add_asn1(&algorithm, &oid, CBS_ASN1_OBJECT) ||
      !CBB_add_bytes(&oid, pqdsa->oid, pqdsa->oid_len) ||
      !CBB_add_asn1(&spki, &key_bitstring, CBS_ASN1_BITSTRING) ||
      !CBB_add_u8(&key_bitstring, 0 /* padding */) ||
      !CBB_add_bytes(&key_bitstring, key->public_key, pqdsa->public_key_len) ||
      !CBB_flush(out)) {
    OPENSSL_PUT_ERROR(EVP, EVP_R_ENCODE_ERROR);
    return 0;
      }

  return 1;
}

static int pqdsa_pub_cmp(const EVP_PKEY *a, const EVP_PKEY *b) {
  PQDSA_KEY *a_key = a->pkey.pqdsa_key;
  PQDSA_KEY *b_key = b->pkey.pqdsa_key;

  return OPENSSL_memcmp(a_key->public_key,
                        b_key->public_key,
                        a->pkey.pqdsa_key->pqdsa->public_key_len) == 0;
}

static int pqdsa_priv_decode(EVP_PKEY *out, CBS *oid, CBS *params, CBS *key, CBS *pubkey) {
  // See https://datatracker.ietf.org/doc/draft-ietf-lamps-dilithium-certificates/
  // section 6. There should be no parameters.
  if (CBS_len(params) > 0) {
    OPENSSL_PUT_ERROR(EVP, EVP_R_DECODE_ERROR);
    return 0;
  }

  // Set the pqdsa params on |out|.
  if (!EVP_PKEY_pqdsa_set_params(out, OBJ_cbs2nid(oid))) {
    OPENSSL_PUT_ERROR(EVP, EVP_R_DECODE_ERROR);
    return 0;
  }

  // Try to parse as one of the three ASN.1 formats defined in ML-DSA-XX-PrivateKey
  // https://datatracker.ietf.org/doc/draft-ietf-lamps-dilithium-certificates/
  // Case 1: seed [0] OCTET STRING
  // Case 2: expandedKey OCTET STRING
  // Case 3: both SEQUENCE {seed, expandedKey}

  if (CBS_peek_asn1_tag(key, CBS_ASN1_CONTEXT_SPECIFIC | 0)) {
    // Case 1: seed [0] OCTET STRING
    CBS seed;
    if (!CBS_get_asn1(key, &seed, CBS_ASN1_CONTEXT_SPECIFIC | 0)) {
      OPENSSL_PUT_ERROR(EVP, EVP_R_DECODE_ERROR);
      return 0;
    }

    if (CBS_len(&seed) != out->pkey.pqdsa_key->pqdsa->keygen_seed_len) {
      OPENSSL_PUT_ERROR(EVP, EVP_R_INVALID_BUFFER_SIZE);
      return 0;
    }

    return PQDSA_KEY_set_raw_keypair_from_seed(out->pkey.pqdsa_key, &seed);
  } else if (CBS_peek_asn1_tag(key, CBS_ASN1_OCTETSTRING)) {
    // Case 2: expandedKey OCTET STRING
    CBS expanded_key;
    if (!CBS_get_asn1(key, &expanded_key, CBS_ASN1_OCTETSTRING)) {
      OPENSSL_PUT_ERROR(EVP, EVP_R_DECODE_ERROR);
      return 0;
    }

    if (CBS_len(&expanded_key) != out->pkey.pqdsa_key->pqdsa->private_key_len) {
      OPENSSL_PUT_ERROR(EVP, EVP_R_INVALID_BUFFER_SIZE);
      return 0;
    }

    return PQDSA_KEY_set_raw_private_key(out->pkey.pqdsa_key, &expanded_key);
  } else if (CBS_peek_asn1_tag(key, CBS_ASN1_SEQUENCE)) {
    // Case 3: both SEQUENCE {seed, expandedKey}
    CBS sequence, seed, expanded_key;
    if (!CBS_get_asn1(key, &sequence, CBS_ASN1_SEQUENCE) ||
        !CBS_get_asn1(&sequence, &seed, CBS_ASN1_OCTETSTRING) ||
        !CBS_get_asn1(&sequence, &expanded_key, CBS_ASN1_OCTETSTRING)) {
      OPENSSL_PUT_ERROR(EVP, EVP_R_DECODE_ERROR);
      return 0;
    }

  return PQDSA_KEY_set_raw_keypair_from_both(out->pkey.pqdsa_key, &seed, &expanded_key);
  } else {
    OPENSSL_PUT_ERROR(EVP, EVP_R_DECODE_ERROR);
    return 0;
  }
}

static int pqdsa_priv_encode(CBB *out, const EVP_PKEY *pkey) {
  PQDSA_KEY *key = pkey->pkey.pqdsa_key;
  const PQDSA *pqdsa = key->pqdsa;
  if (key->seed == NULL) {
    OPENSSL_PUT_ERROR(EVP, EVP_R_NOT_A_PRIVATE_KEY);
    return 0;
  }
  // See https://datatracker.ietf.org/doc/draft-ietf-lamps-dilithium-certificates/ section 6.
  CBB pkcs8, algorithm, oid, private_key, seed_choice;
  if (!CBB_add_asn1(out, &pkcs8, CBS_ASN1_SEQUENCE) ||
      !CBB_add_asn1_uint64(&pkcs8, PKCS8_VERSION_ONE /* version */) ||
      !CBB_add_asn1(&pkcs8, &algorithm, CBS_ASN1_SEQUENCE) ||
      !CBB_add_asn1(&algorithm, &oid, CBS_ASN1_OBJECT) ||
      !CBB_add_bytes(&oid, pqdsa->oid, pqdsa->oid_len) ||
      !CBB_add_asn1(&pkcs8, &private_key, CBS_ASN1_OCTETSTRING) ||
      !CBB_add_asn1(&private_key, &seed_choice, CBS_ASN1_CONTEXT_SPECIFIC | 0) ||
      !CBB_add_bytes(&seed_choice, key->seed, pqdsa->keygen_seed_len) ||
      !CBB_flush(out)) {
    OPENSSL_PUT_ERROR(EVP, EVP_R_ENCODE_ERROR);
    return 0;
      }

  return 1;
}

static int pqdsa_size(const EVP_PKEY *pkey) {
  if (pkey->pkey.pqdsa_key == NULL) {
    OPENSSL_PUT_ERROR(EVP, EVP_R_NO_PARAMETERS_SET);
    return 0;
  }
  return pkey->pkey.pqdsa_key->pqdsa->signature_len;
}

static int pqdsa_bits(const EVP_PKEY *pkey) {
  if (pkey->pkey.pqdsa_key == NULL) {
    OPENSSL_PUT_ERROR(EVP, EVP_R_NO_PARAMETERS_SET);
    return 0;
  }
  return 8 * (pkey->pkey.pqdsa_key->pqdsa->public_key_len);
}

const EVP_PKEY_ASN1_METHOD pqdsa_asn1_meth = {
  //2.16.840.1.101.3.4.3
  EVP_PKEY_PQDSA,

  {0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x03},
  8,

  "PQ DSA",
  "AWS-LC PQ DSA method",

  pqdsa_pub_decode,
  pqdsa_pub_encode,
  pqdsa_pub_cmp,
  pqdsa_priv_decode,
  pqdsa_priv_encode,
  NULL /*priv_encode_v2*/,
  NULL /* pqdsa_set_priv_raw */,
  NULL /*pqdsa_set_pub_raw */ ,
  pqdsa_get_priv_raw,
  pqdsa_get_pub_raw,
  NULL /* pkey_opaque */,
  pqdsa_size,
  pqdsa_bits,
  NULL /* param_missing */,
  NULL /* param_copy */,
  NULL /* param_cmp */,
  pqdsa_free,
};
