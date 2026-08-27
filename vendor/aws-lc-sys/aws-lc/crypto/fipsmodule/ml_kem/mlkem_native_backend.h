// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0 OR ISC

#ifndef MLKEM_NATIVE_BACKEND_H
#define MLKEM_NATIVE_BACKEND_H

#include <openssl/target.h>

// For now, we only include an AArch64 backend, used on Linux and MacOS systems
#if !defined(OPENSSL_NO_ASM) &&				\
    (defined(OPENSSL_LINUX) || defined(OPENSSL_APPLE))

#if defined(OPENSSL_AARCH64)
// AWS-LC-owned replacement for mlkem/native/aarch64/meta.h that adds a runtime
// NEON capability gate; falls back to the C reference impl when NEON is absent.
#include "mlkem_aarch64_meta.h"
#elif defined(OPENSSL_X86_64) && !defined(MY_ASSEMBLER_IS_TOO_OLD_FOR_AVX)
#include "mlkem/native/x86_64/meta.h"
#endif

#endif

#endif /* MLKEM_NATIVE_BACKEND_H */
