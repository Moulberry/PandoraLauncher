// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0 OR ISC

// AWS-LC-owned replacement for the vendored mlkem/native/aarch64/meta.h.
//
// The vendored header calls the NEON assembly unconditionally. This copy is
// identical except that every native entry point is guarded by a runtime NEON
// check (mlk_sys_check_capability(MLK_SYS_CAP_NEON)); when NEON is absent it
// returns MLK_NATIVE_FUNC_FALLBACK and the frontend uses its C reference path
// (e.g. under OPENSSL_armcap=0). Kept outside the vendored `mlkem/` tree, which
// importer.sh clobbers on re-import, and selected via mlkem_native_backend.h.
// Keep the toggles/identifier/asm signatures in sync with the vendored header.

// Reuse the vendored include-guard name on purpose: mlkem_native_bcm.c
// #undef's it between per-level rounds to re-emit the per-MLKEM_K native
// functions. A different guard name would break that re-emission.
#ifndef MLK_NATIVE_AARCH64_META_H
#define MLK_NATIVE_AARCH64_META_H

/* Set of primitives that this backend replaces */
#define MLK_USE_NATIVE_NTT
#define MLK_USE_NATIVE_INTT
#define MLK_USE_NATIVE_POLY_REDUCE
#define MLK_USE_NATIVE_POLY_TOMONT
#define MLK_USE_NATIVE_POLY_MULCACHE_COMPUTE
#define MLK_USE_NATIVE_POLYVEC_BASEMUL_ACC_MONTGOMERY_CACHED
#define MLK_USE_NATIVE_POLY_TOBYTES
#define MLK_USE_NATIVE_REJ_UNIFORM

/* Identifier for this backend so that source and assembly files
 * in the build can be appropriately guarded. */
#define MLK_ARITH_BACKEND_AARCH64


#if !defined(__ASSEMBLER__)
#include "mlkem/native/api.h"
#include "mlkem/native/aarch64/src/arith_native_aarch64.h"

MLK_MUST_CHECK_RETURN_VALUE
static MLK_INLINE int mlk_ntt_native(int16_t data[MLKEM_N])
{
  if (!mlk_sys_check_capability(MLK_SYS_CAP_NEON))
  {
    return MLK_NATIVE_FUNC_FALLBACK;
  }
  mlk_ntt_asm(data, mlk_aarch64_ntt_zetas_layer12345,
              mlk_aarch64_ntt_zetas_layer67);
  return MLK_NATIVE_FUNC_SUCCESS;
}

MLK_MUST_CHECK_RETURN_VALUE
static MLK_INLINE int mlk_intt_native(int16_t data[MLKEM_N])
{
  if (!mlk_sys_check_capability(MLK_SYS_CAP_NEON))
  {
    return MLK_NATIVE_FUNC_FALLBACK;
  }
  mlk_intt_asm(data, mlk_aarch64_invntt_zetas_layer12345,
               mlk_aarch64_invntt_zetas_layer67);
  return MLK_NATIVE_FUNC_SUCCESS;
}

MLK_MUST_CHECK_RETURN_VALUE
static MLK_INLINE int mlk_poly_reduce_native(int16_t data[MLKEM_N])
{
  if (!mlk_sys_check_capability(MLK_SYS_CAP_NEON))
  {
    return MLK_NATIVE_FUNC_FALLBACK;
  }
  mlk_poly_reduce_asm(data);
  return MLK_NATIVE_FUNC_SUCCESS;
}

MLK_MUST_CHECK_RETURN_VALUE
static MLK_INLINE int mlk_poly_tomont_native(int16_t data[MLKEM_N])
{
  if (!mlk_sys_check_capability(MLK_SYS_CAP_NEON))
  {
    return MLK_NATIVE_FUNC_FALLBACK;
  }
  mlk_poly_tomont_asm(data);
  return MLK_NATIVE_FUNC_SUCCESS;
}

MLK_MUST_CHECK_RETURN_VALUE
static MLK_INLINE int mlk_poly_mulcache_compute_native(int16_t x[MLKEM_N / 2],
                                                       const int16_t y[MLKEM_N])
{
  if (!mlk_sys_check_capability(MLK_SYS_CAP_NEON))
  {
    return MLK_NATIVE_FUNC_FALLBACK;
  }
  mlk_poly_mulcache_compute_asm(x, y, mlk_aarch64_zetas_mulcache_native,
                                mlk_aarch64_zetas_mulcache_twisted_native);
  return MLK_NATIVE_FUNC_SUCCESS;
}

#if defined(MLK_CONFIG_MULTILEVEL_WITH_SHARED) || MLKEM_K == 2
MLK_MUST_CHECK_RETURN_VALUE
static MLK_INLINE int mlk_polyvec_basemul_acc_montgomery_cached_k2_native(
    int16_t r[MLKEM_N], const int16_t a[2 * MLKEM_N],
    const int16_t b[2 * MLKEM_N], const int16_t b_cache[2 * (MLKEM_N / 2)])
{
  if (!mlk_sys_check_capability(MLK_SYS_CAP_NEON))
  {
    return MLK_NATIVE_FUNC_FALLBACK;
  }
  mlk_polyvec_basemul_acc_montgomery_cached_asm_k2(r, a, b, b_cache);
  return MLK_NATIVE_FUNC_SUCCESS;
}
#endif /* MLK_CONFIG_MULTILEVEL_WITH_SHARED || MLKEM_K == 2 */

#if defined(MLK_CONFIG_MULTILEVEL_WITH_SHARED) || MLKEM_K == 3
MLK_MUST_CHECK_RETURN_VALUE
static MLK_INLINE int mlk_polyvec_basemul_acc_montgomery_cached_k3_native(
    int16_t r[MLKEM_N], const int16_t a[3 * MLKEM_N],
    const int16_t b[3 * MLKEM_N], const int16_t b_cache[3 * (MLKEM_N / 2)])
{
  if (!mlk_sys_check_capability(MLK_SYS_CAP_NEON))
  {
    return MLK_NATIVE_FUNC_FALLBACK;
  }
  mlk_polyvec_basemul_acc_montgomery_cached_asm_k3(r, a, b, b_cache);
  return MLK_NATIVE_FUNC_SUCCESS;
}
#endif /* MLK_CONFIG_MULTILEVEL_WITH_SHARED || MLKEM_K == 3 */

#if defined(MLK_CONFIG_MULTILEVEL_WITH_SHARED) || MLKEM_K == 4
MLK_MUST_CHECK_RETURN_VALUE
static MLK_INLINE int mlk_polyvec_basemul_acc_montgomery_cached_k4_native(
    int16_t r[MLKEM_N], const int16_t a[4 * MLKEM_N],
    const int16_t b[4 * MLKEM_N], const int16_t b_cache[4 * (MLKEM_N / 2)])
{
  if (!mlk_sys_check_capability(MLK_SYS_CAP_NEON))
  {
    return MLK_NATIVE_FUNC_FALLBACK;
  }
  mlk_polyvec_basemul_acc_montgomery_cached_asm_k4(r, a, b, b_cache);
  return MLK_NATIVE_FUNC_SUCCESS;
}
#endif /* MLK_CONFIG_MULTILEVEL_WITH_SHARED || MLKEM_K == 4 */

MLK_MUST_CHECK_RETURN_VALUE
static MLK_INLINE int mlk_poly_tobytes_native(uint8_t r[MLKEM_POLYBYTES],
                                              const int16_t a[MLKEM_N])
{
  if (!mlk_sys_check_capability(MLK_SYS_CAP_NEON))
  {
    return MLK_NATIVE_FUNC_FALLBACK;
  }
  mlk_poly_tobytes_asm(r, a);
  return MLK_NATIVE_FUNC_SUCCESS;
}

MLK_MUST_CHECK_RETURN_VALUE
static MLK_INLINE int mlk_rej_uniform_native(int16_t *r, unsigned len,
                                             const uint8_t *buf,
                                             unsigned buflen)
{
  if (!mlk_sys_check_capability(MLK_SYS_CAP_NEON) || len != MLKEM_N ||
      buflen % 24 != 0)
  {
    return MLK_NATIVE_FUNC_FALLBACK;
  }
  return (int)mlk_rej_uniform_asm(r, buf, buflen, mlk_rej_uniform_table);
}
#endif /* !__ASSEMBLER__ */

#endif /* !MLK_NATIVE_AARCH64_META_H */
