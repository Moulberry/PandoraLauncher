/*
 * Copyright (c) The mlkem-native project authors
 * SPDX-License-Identifier: Apache-2.0 OR ISC OR MIT
 */
#ifndef MLK_NATIVE_X86_64_SRC_ARITH_NATIVE_X86_64_H
#define MLK_NATIVE_X86_64_SRC_ARITH_NATIVE_X86_64_H

#include "../../../common.h"

#include <immintrin.h>
#include <stdint.h>
#include "consts.h"

#define MLK_AVX2_REJ_UNIFORM_BUFLEN \
  (3 * 168) /* REJ_UNIFORM_NBLOCKS * SHAKE128_RATE */

#define mlk_rej_uniform_asm MLK_NAMESPACE(rej_uniform_asm)
uint64_t mlk_rej_uniform_asm(int16_t *r, const uint8_t *buf, unsigned buflen,
                             const uint8_t *table);

#define mlk_rej_uniform_table MLK_NAMESPACE(rej_uniform_table)
extern const uint8_t mlk_rej_uniform_table[];

#define mlk_ntt_avx2 MLK_NAMESPACE(ntt_avx2)
void mlk_ntt_avx2(int16_t *r, const int16_t *mlk_qdata);

#define mlk_invntt_avx2 MLK_NAMESPACE(invntt_avx2)
void mlk_invntt_avx2(int16_t *r, const int16_t *mlk_qdata);

#define mlk_nttunpack_avx2 MLK_NAMESPACE(nttunpack_avx2)
void mlk_nttunpack_avx2(int16_t *r);

#define mlk_reduce_avx2 MLK_NAMESPACE(reduce_avx2)
void mlk_reduce_avx2(int16_t *r, const int16_t *mlk_qdata);

#define mlk_poly_mulcache_compute_avx2 MLK_NAMESPACE(poly_mulcache_compute_avx2)
void mlk_poly_mulcache_compute_avx2(int16_t *out, const int16_t *in,
                                    const int16_t *mlk_qdata);

#define mlk_polyvec_basemul_acc_montgomery_cached_asm_k2 \
  MLK_NAMESPACE(polyvec_basemul_acc_montgomery_cached_asm_k2)
void mlk_polyvec_basemul_acc_montgomery_cached_asm_k2(int16_t *r,
                                                      const int16_t *a,
                                                      const int16_t *b,
                                                      const int16_t *b_cache,
                                                      const int16_t *qdata);

#define mlk_polyvec_basemul_acc_montgomery_cached_asm_k3 \
  MLK_NAMESPACE(polyvec_basemul_acc_montgomery_cached_asm_k3)
void mlk_polyvec_basemul_acc_montgomery_cached_asm_k3(int16_t *r,
                                                      const int16_t *a,
                                                      const int16_t *b,
                                                      const int16_t *b_cache,
                                                      const int16_t *qdata);

#define mlk_polyvec_basemul_acc_montgomery_cached_asm_k4 \
  MLK_NAMESPACE(polyvec_basemul_acc_montgomery_cached_asm_k4)
void mlk_polyvec_basemul_acc_montgomery_cached_asm_k4(int16_t *r,
                                                      const int16_t *a,
                                                      const int16_t *b,
                                                      const int16_t *b_cache,
                                                      const int16_t *qdata);

#define mlk_ntttobytes_avx2 MLK_NAMESPACE(ntttobytes_avx2)
void mlk_ntttobytes_avx2(uint8_t *r, const int16_t *a,
                         const int16_t *mlk_qdata);

#define mlk_nttfrombytes_avx2 MLK_NAMESPACE(nttfrombytes_avx2)
void mlk_nttfrombytes_avx2(int16_t *r, const uint8_t *a,
                           const int16_t *mlk_qdata);

#define mlk_tomont_avx2 MLK_NAMESPACE(tomont_avx2)
void mlk_tomont_avx2(int16_t *r, const int16_t *mlk_qdata);

#define mlk_poly_compress_d4_avx2 MLK_NAMESPACE(poly_compress_d4_avx2)
void mlk_poly_compress_d4_avx2(uint8_t r[MLKEM_POLYCOMPRESSEDBYTES_D4],
                               const int16_t *MLK_RESTRICT a);
#define mlk_poly_decompress_d4_avx2 MLK_NAMESPACE(poly_decompress_d4_avx2)
void mlk_poly_decompress_d4_avx2(int16_t *MLK_RESTRICT r,
                                 const uint8_t a[MLKEM_POLYCOMPRESSEDBYTES_D4]);
#define mlk_poly_compress_d10_avx2 MLK_NAMESPACE(poly_compress10_avx2)
void mlk_poly_compress_d10_avx2(uint8_t r[MLKEM_POLYCOMPRESSEDBYTES_D10],
                                const int16_t *MLK_RESTRICT a);
#define mlk_poly_decompress_d10_avx2 MLK_NAMESPACE(poly_decompress10_avx2)
void mlk_poly_decompress_d10_avx2(
    int16_t *MLK_RESTRICT r, const uint8_t a[MLKEM_POLYCOMPRESSEDBYTES_D10]);
#define mlk_poly_compress_d5_avx2 MLK_NAMESPACE(poly_compress_d5_avx2)
void mlk_poly_compress_d5_avx2(uint8_t r[MLKEM_POLYCOMPRESSEDBYTES_D5],
                               const int16_t *MLK_RESTRICT a);
#define mlk_poly_decompress_d5_avx2 MLK_NAMESPACE(poly_decompress_d5_avx2)
void mlk_poly_decompress_d5_avx2(int16_t *MLK_RESTRICT r,
                                 const uint8_t a[MLKEM_POLYCOMPRESSEDBYTES_D5]);
#define mlk_poly_compress_d11_avx2 MLK_NAMESPACE(poly_compress11_avx2)
void mlk_poly_compress_d11_avx2(uint8_t r[MLKEM_POLYCOMPRESSEDBYTES_D11],
                                const int16_t *MLK_RESTRICT a);
#define mlk_poly_decompress_d11_avx2 MLK_NAMESPACE(poly_decompress11_avx2)
void mlk_poly_decompress_d11_avx2(
    int16_t *MLK_RESTRICT r, const uint8_t a[MLKEM_POLYCOMPRESSEDBYTES_D11]);

#endif /* !MLK_NATIVE_X86_64_SRC_ARITH_NATIVE_X86_64_H */
