/*
 * Copyright (c) The mlkem-native project authors
 * SPDX-License-Identifier: Apache-2.0 OR ISC OR MIT
 */

/* References
 * ==========
 *
 * - [FIPS203]
 *   FIPS 203 Module-Lattice-Based Key-Encapsulation Mechanism Standard
 *   National Institute of Standards and Technology
 *   https://csrc.nist.gov/pubs/fips/203/final
 */

#ifndef MLK_H
#define MLK_H

/******************************************************************************
 *
 * Public API for mlkem-native
 *
 * This header defines the public API of a single build of mlkem-native.
 *
 * # Examples
 *
 * See [examples/basic], [examples/multilevel_build], and
 * [examples/multilevel_build_native] for examples of how to use this header.
 *
 * # Usage
 *
 * To use this header, configure the following options:
 *
 * - MLK_CONFIG_API_PARAMETER_SET [required]
 *
 *   The parameter set used for the build; 512, 768, or 1024.
 *
 * - MLK_CONFIG_API_NAMESPACE_PREFIX [required]
 *
 *   The namespace prefix used for the build.
 *
 *   NOTE:
 *   For a multi-level build, you must include the 512/768/1024 suffixes
 *   in MLK_CONFIG_API_NAMESPACE_PREFIX.
 *
 * - MLK_CONFIG_API_NO_SUPERCOP [optional]
 *
 *   By default, this header will also expose the mlkem-native API in the
 *   SUPERCOP naming convention crypto_kem_xxx. If you don't want/need this,
 *   set MLK_CONFIG_API_NO_SUPERCOP. You must set this for a multi-level build.
 *
 * - MLK_CONFIG_API_CONSTANTS_ONLY [optional]
 *
 *   If you don't want this header to expose any function declarations,
 *   but only constants for the sizes of key material, set
 *   MLK_CONFIG_API_CONSTANTS_ONLY. In this case, you don't need to set
 *   MLK_CONFIG_API_PARAMETER_SET or MLK_CONFIG_API_NAMESPACE_PREFIX,
 *   nor include a configuration.
 *
 * # Multi-level builds
 *
 * This header specifies a build of mlkem-native for a fixed security level.
 * If you need multiple builds, e.g. to build a library offering multiple
 * security levels, you need multiple instances of this header.
 *
 * NOTE: In this case, you must rename or #undef the MLK_H header guard
 *       prior to subsequent inclusions of this file.
 *
 ******************************************************************************/

/******************************* Key sizes ************************************/

/* Sizes of cryptographic material, per parameter set */
/* See mlkem/common.h for the arithmetic expressions giving rise to these */
/* check-magic: off */
#define MLKEM512_SECRETKEYBYTES 1632
#define MLKEM512_PUBLICKEYBYTES 800
#define MLKEM512_CIPHERTEXTBYTES 768

#define MLKEM768_SECRETKEYBYTES 2400
#define MLKEM768_PUBLICKEYBYTES 1184
#define MLKEM768_CIPHERTEXTBYTES 1088

#define MLKEM1024_SECRETKEYBYTES 3168
#define MLKEM1024_PUBLICKEYBYTES 1568
#define MLKEM1024_CIPHERTEXTBYTES 1568
/* check-magic: on */

/* Size of randomness coins in bytes (level-independent) */
#define MLKEM_SYMBYTES 32
#define MLKEM512_SYMBYTES MLKEM_SYMBYTES
#define MLKEM768_SYMBYTES MLKEM_SYMBYTES
#define MLKEM1024_SYMBYTES MLKEM_SYMBYTES
/* Size of shared secret in bytes (level-independent) */
#define MLKEM_BYTES 32
#define MLKEM512_BYTES MLKEM_BYTES
#define MLKEM768_BYTES MLKEM_BYTES
#define MLKEM1024_BYTES MLKEM_BYTES

/* Sizes of cryptographic material, as a function of LVL=512,768,1024 */
#define MLKEM_SECRETKEYBYTES_(LVL) MLKEM##LVL##_SECRETKEYBYTES
#define MLKEM_PUBLICKEYBYTES_(LVL) MLKEM##LVL##_PUBLICKEYBYTES
#define MLKEM_CIPHERTEXTBYTES_(LVL) MLKEM##LVL##_CIPHERTEXTBYTES
#define MLKEM_SECRETKEYBYTES(LVL) MLKEM_SECRETKEYBYTES_(LVL)
#define MLKEM_PUBLICKEYBYTES(LVL) MLKEM_PUBLICKEYBYTES_(LVL)
#define MLKEM_CIPHERTEXTBYTES(LVL) MLKEM_CIPHERTEXTBYTES_(LVL)

/****************************** Function API **********************************/

#if !defined(MLK_CONFIG_API_CONSTANTS_ONLY)

#if !defined(MLK_CONFIG_API_PARAMETER_SET)
#error MLK_CONFIG_API_PARAMETER_SET not defined
#endif
#if !defined(MLK_CONFIG_API_NAMESPACE_PREFIX)
#error MLK_CONFIG_API_NAMESPACE_PREFIX not defined
#endif

/* Derive namespacing macro */
#define MLK_API_CONCAT_(x, y) x##y
#define MLK_API_CONCAT(x, y) MLK_API_CONCAT_(x, y)
#define MLK_API_CONCAT_UNDERSCORE(x, y) MLK_API_CONCAT(MLK_API_CONCAT(x, _), y)
#define MLK_API_NAMESPACE(sym) \
  MLK_API_CONCAT_UNDERSCORE(MLK_CONFIG_API_NAMESPACE_PREFIX, sym)

#if defined(__GNUC__) || defined(clang)
#define MLK_API_MUST_CHECK_RETURN_VALUE __attribute__((warn_unused_result))
#else
#define MLK_API_MUST_CHECK_RETURN_VALUE
#endif

#include <stdint.h>

/*************************************************
 * Name:        crypto_kem_keypair_derand
 *
 * Description: Generates public and private key
 *              for CCA-secure ML-KEM key encapsulation mechanism
 *
 * Arguments:   - uint8_t pk[]: pointer to output public key, an array of
 *                 length MLKEM{512,768,1024}_PUBLICKEYBYTES bytes.
 *              - uint8_t sk[]: pointer to output private key, an array of
 *                  of MLKEM{512,768,1024}_SECRETKEYBYTES bytes.
 *              - uint8_t *coins: pointer to input randomness, an array of
 *                  2*MLKEM_SYMBYTES uniformly random bytes.
 *
 * Returns:     - 0: On success
 *              - -1: On PCT failure (if MLK_CONFIG_KEYGEN_PCT) is enabled.
 *
 * Specification: Implements @[FIPS203, Algorithm 16, ML-KEM.KeyGen_Internal]
 *
 **************************************************/
MLK_API_MUST_CHECK_RETURN_VALUE
int MLK_API_NAMESPACE(keypair_derand)(
    uint8_t pk[MLKEM_PUBLICKEYBYTES(MLK_CONFIG_API_PARAMETER_SET)],
    uint8_t sk[MLKEM_SECRETKEYBYTES(MLK_CONFIG_API_PARAMETER_SET)],
    const uint8_t coins[2 * MLKEM_SYMBYTES]);

/*************************************************
 * Name:        crypto_kem_keypair
 *
 * Description: Generates public and private key
 *              for CCA-secure ML-KEM key encapsulation mechanism
 *
 * Arguments:   - uint8_t *pk: pointer to output public key, an array of
 *                 MLKEM{512,768,1024}_PUBLICKEYBYTES bytes.
 *              - uint8_t *sk: pointer to output private key, an array of
 *                 MLKEM{512,768,1024}_SECRETKEYBYTES bytes.
 *
 * Returns:     - 0: On success
 *              - -1: On PCT failure (if MLK_CONFIG_KEYGEN_PCT) is enabled.
 *
 * Specification: Implements @[FIPS203, Algorithm 19, ML-KEM.KeyGen]
 *
 **************************************************/
MLK_API_MUST_CHECK_RETURN_VALUE
int MLK_API_NAMESPACE(keypair)(
    uint8_t pk[MLKEM_PUBLICKEYBYTES(MLK_CONFIG_API_PARAMETER_SET)],
    uint8_t sk[MLKEM_SECRETKEYBYTES(MLK_CONFIG_API_PARAMETER_SET)]);

/*************************************************
 * Name:        crypto_kem_enc_derand
 *
 * Description: Generates cipher text and shared
 *              secret for given public key
 *
 * Arguments:   - uint8_t *ct: pointer to output cipher text, an array of
 *                 MLKEM{512,768,1024}_CIPHERTEXTBYTES bytes.
 *              - uint8_t *ss: pointer to output shared secret, an array of
 *                 MLKEM_BYTES bytes.
 *              - const uint8_t *pk: pointer to input public key, an array of
 *                 MLKEM{512,768,1024}_PUBLICKEYBYTES bytes.
 *              - const uint8_t *coins: pointer to input randomness, an array of
 *                 MLKEM_SYMBYTES bytes.
 *
 * Returns: - 0 on success
 *          - -1 if the 'modulus check' @[FIPS203, Section 7.2]
 *            for the public key fails.
 *
 * Specification: Implements @[FIPS203, Algorithm 17, ML-KEM.Encaps_Internal]
 *
 **************************************************/
MLK_API_MUST_CHECK_RETURN_VALUE
int MLK_API_NAMESPACE(enc_derand)(
    uint8_t ct[MLKEM_CIPHERTEXTBYTES(MLK_CONFIG_API_PARAMETER_SET)],
    uint8_t ss[MLKEM_BYTES],
    const uint8_t pk[MLKEM_PUBLICKEYBYTES(MLK_CONFIG_API_PARAMETER_SET)],
    const uint8_t coins[MLKEM_SYMBYTES]);

/*************************************************
 * Name:        crypto_kem_enc
 *
 * Description: Generates cipher text and shared
 *              secret for given public key
 *
 * Arguments:   - uint8_t *ct: pointer to output cipher text, an array of
 *                 MLKEM{512,768,1024}_CIPHERTEXTBYTES bytes.
 *              - uint8_t *ss: pointer to output shared secret, an array of
 *                 MLKEM_BYTES bytes.
 *              - const uint8_t *pk: pointer to input public key, an array of
 *                 MLKEM{512,768,1024}_PUBLICKEYBYTES bytes.
 *
 * Returns: - 0 on success
 *          - -1 if the 'modulus check' @[FIPS203, Section 7.2]
 *            for the public key fails.
 *
 * Specification: Implements @[FIPS203, Algorithm 20, ML-KEM.Encaps]
 *
 **************************************************/
MLK_API_MUST_CHECK_RETURN_VALUE
int MLK_API_NAMESPACE(enc)(
    uint8_t ct[MLKEM_CIPHERTEXTBYTES(MLK_CONFIG_API_PARAMETER_SET)],
    uint8_t ss[MLKEM_BYTES],
    const uint8_t pk[MLKEM_PUBLICKEYBYTES(MLK_CONFIG_API_PARAMETER_SET)]);

/*************************************************
 * Name:        crypto_kem_dec
 *
 * Description: Generates shared secret for given
 *              cipher text and private key
 *
 * Arguments:   - uint8_t *ss: pointer to output shared secret, an array of
 *                 MLKEM_BYTES bytes.
 *              - const uint8_t *ct: pointer to input cipher text, an array of
 *                 MLKEM{512,768,1024}_CIPHERTEXTBYTES bytes.
 *              - const uint8_t *sk: pointer to input private key, an array of
 *                 MLKEM{512,768,1024}_SECRETKEYBYTES bytes.
 *
 * Returns: - 0 on success
 *          - -1 if the 'hash check' @[FIPS203, Section 7.3]
 *            for the secret key fails.
 *
 * Specification: Implements @[FIPS203, Algorithm 21, ML-KEM.Decaps]
 *
 **************************************************/
MLK_API_MUST_CHECK_RETURN_VALUE
int MLK_API_NAMESPACE(dec)(
    uint8_t ss[MLKEM_BYTES],
    const uint8_t ct[MLKEM_CIPHERTEXTBYTES(MLK_CONFIG_API_PARAMETER_SET)],
    const uint8_t sk[MLKEM_SECRETKEYBYTES(MLK_CONFIG_API_PARAMETER_SET)]);

/****************************** SUPERCOP API *********************************/

#if !defined(MLK_CONFIG_API_NO_SUPERCOP)
/* Export API in SUPERCOP naming scheme CRYPTO_xxx / crypto_kem_xxx */
#define CRYPTO_SECRETKEYBYTES MLKEM_SECRETKEYBYTES(MLK_CONFIG_API_PARAMETER_SET)
#define CRYPTO_PUBLICKEYBYTES MLKEM_PUBLICKEYBYTES(MLK_CONFIG_API_PARAMETER_SET)
#define CRYPTO_CIPHERTEXTBYTES \
  MLKEM_CIPHERTEXTBYTES(MLK_CONFIG_API_PARAMETER_SET)
#define CRYPTO_SYMBYTES MLKEM_SYMBYTES
#define CRYPTO_BYTES MLKEM_BYTES

#define crypto_kem_keypair_derand MLK_API_NAMESPACE(keypair_derand)
#define crypto_kem_keypair MLK_API_NAMESPACE(keypair)
#define crypto_kem_enc_derand MLK_API_NAMESPACE(enc_derand)
#define crypto_kem_enc MLK_API_NAMESPACE(enc)
#define crypto_kem_dec MLK_API_NAMESPACE(dec)

#else /* !MLK_CONFIG_API_NO_SUPERCOP */

/* If the SUPERCOP API is not needed, we can undefine the various helper macros
 * above. Otherwise, they are needed for lazy evaluation of crypto_kem_xxx. */
#undef MLK_API_CONCAT
#undef MLK_API_CONCAT_
#undef MLK_API_CONCAT_UNDERSCORE
#undef MLK_API_NAMESPACE
#undef MLK_API_MUST_CHECK_RETURN_VALUE

#endif /* MLK_CONFIG_API_NO_SUPERCOP */
#endif /* !MLK_CONFIG_API_CONSTANTS_ONLY */

#endif /* !MLK_H */
