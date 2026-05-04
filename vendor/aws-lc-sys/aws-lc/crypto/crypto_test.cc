/* Copyright (c) 2020, Google Inc.
 *
 * Permission to use, copy, modify, and/or distribute this software for any
 * purpose with or without fee is hereby granted, provided that the above
 * copyright notice and this permission notice appear in all copies.
 *
 * THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
 * WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
 * MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR ANY
 * SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
 * WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN ACTION
 * OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF OR IN
 * CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE. */

#include <stdio.h>
#include <string.h>

#include <string>

#include <openssl/base.h>
#include <openssl/aead.h>
#include <openssl/crypto.h>
#include <openssl/cipher.h>
#include <openssl/mem.h>
#include <openssl/service_indicator.h>

#include <gtest/gtest.h>
#include "test/test_util.h"
#include "ube/vm_ube_detect.h"

static int AWS_LC_ERROR_return(void) {
  GUARD_PTR(NULL);
  return 1;
}

static int AWS_LC_SUCCESS_return(void) {
  char non_null_ptr[1];
  GUARD_PTR(non_null_ptr);
  return 1;
}

TEST(CryptoTest, SafetyMacro) {
  // It is assumed that |GUARD_PTR| returns 0 for fail/false and 1 for
  // success/true. Change these default values with care because code might not
  // use the related macros |AWS_LC_ERROR| or |AWS_LC_SUCCESS|.
  EXPECT_EQ(AWS_LC_ERROR_return(), 0);
  EXPECT_EQ(AWS_LC_SUCCESS_return(), 1);
}

// Test that OPENSSL_VERSION_NUMBER and OPENSSL_VERSION_TEXT are consistent.
// Node.js parses the version out of OPENSSL_VERSION_TEXT instead of using
// OPENSSL_VERSION_NUMBER.
TEST(CryptoTest, Version) {
  char expected[512];
  snprintf(expected, sizeof(expected), "OpenSSL %d.%d.%d ",
           OPENSSL_VERSION_NUMBER >> 28, (OPENSSL_VERSION_NUMBER >> 20) & 0xff,
           (OPENSSL_VERSION_NUMBER >> 12) & 0xff);
  EXPECT_EQ(expected,
            std::string(OPENSSL_VERSION_TEXT).substr(0, strlen(expected)));

  std::string full_expected = "OpenSSL 1.1.1 (compatible; AWS-LC ";
  full_expected += AWSLC_VERSION_NUMBER_STRING;
  full_expected += ")";
  EXPECT_EQ(OPENSSL_VERSION_TEXT, full_expected);

  full_expected = AWSLC_VERSION_STRING;
  std::string actual = std::string(OpenSSL_version(OPENSSL_VERSION));
  EXPECT_EQ(actual, full_expected);
}

TEST(CryptoTest, Strndup) {
  bssl::UniquePtr<char> str(OPENSSL_strndup(nullptr, 0));
  EXPECT_TRUE(str);
  EXPECT_STREQ("", str.get());
}

TEST(CryptoTest, aws_lc_assert_entropy_cpu_jitter) {
  if (FIPS_mode() == 1 && CRYPTO_get_vm_ube_supported() != 1) {
    ASSERT_EQ(1, FIPS_is_entropy_cpu_jitter());
  }
}

TEST(CryptoTest, OPENSSL_hexstr2buf) {
  const char *test_cases[][2] = {{"a2", "\xa2"},
                                 {"a213", "\xa2\x13"},
                                 {"ffeedd", "\xff\xee\xdd"},
                                 {"10aab1c2", "\x10\xaa\xb1\xc2"}};

  for (auto test_case : test_cases) {
    const char *test_value = test_case[0];
    const char *expected_answer = test_case[1];
    size_t actual_answer_len = 0;
    // The longest test case we have is currently 4 bytes long
    size_t expected_answer_len = OPENSSL_strnlen(test_case[1], 5);
    unsigned char *buf = OPENSSL_hexstr2buf(test_value, &actual_answer_len);
    ASSERT_TRUE(buf != nullptr);
    EXPECT_EQ(expected_answer_len, actual_answer_len);
    EXPECT_EQ(0, OPENSSL_memcmp(buf, expected_answer, expected_answer_len));
    OPENSSL_free(buf);
  }

  // Test failure modes
  size_t actual_answer_len = 0;
  EXPECT_FALSE(OPENSSL_hexstr2buf("a", &actual_answer_len));
  EXPECT_FALSE(OPENSSL_hexstr2buf(NULL, &actual_answer_len));
  EXPECT_FALSE(OPENSSL_hexstr2buf("ab", nullptr));
  EXPECT_FALSE(OPENSSL_hexstr2buf("ag", &actual_answer_len));
}

#if defined(BORINGSSL_FIPS_COUNTERS)
using CounterArray = size_t[fips_counter_max + 1];

static void read_all_counters(CounterArray counters) {
  for (fips_counter_t counter = static_cast<fips_counter_t>(0);
       counter <= fips_counter_max;
       counter = static_cast<fips_counter_t>(counter + 1)) {
    counters[counter] = FIPS_read_counter(counter);
  }
}

static void expect_counter_delta_is_zero_except_for_a_one_at(
    CounterArray before, CounterArray after, fips_counter_t position) {
  for (fips_counter_t counter = static_cast<fips_counter_t>(0);
       counter <= fips_counter_max;
       counter = static_cast<fips_counter_t>(counter + 1)) {
    const size_t expected_delta = counter == position ? 1 : 0;
    EXPECT_EQ(after[counter], before[counter] + expected_delta) << counter;
  }
}

TEST(CryptoTest, FIPSCountersEVP) {
  constexpr struct {
    const EVP_CIPHER *(*cipher)();
    fips_counter_t counter;
  } kTests[] = {
      {
          EVP_aes_128_gcm,
          fips_counter_evp_aes_128_gcm,
      },
      {
          EVP_aes_256_gcm,
          fips_counter_evp_aes_256_gcm,
      },
      {
          EVP_aes_128_ctr,
          fips_counter_evp_aes_128_ctr,
      },
      {
          EVP_aes_256_ctr,
          fips_counter_evp_aes_256_ctr,
      },
  };

  uint8_t key[EVP_MAX_KEY_LENGTH] = {0};
  uint8_t iv[EVP_MAX_IV_LENGTH] = {1};
  CounterArray before, after;
  for (const auto &test : kTests) {
    read_all_counters(before);
    bssl::ScopedEVP_CIPHER_CTX ctx;
    ASSERT_TRUE(EVP_EncryptInit_ex(ctx.get(), test.cipher(), /*engine=*/nullptr,
                                   key, iv));
    read_all_counters(after);

    expect_counter_delta_is_zero_except_for_a_one_at(before, after,
                                                     test.counter);
  }
}

TEST(CryptoTest, FIPSCountersEVP_AEAD) {
  constexpr struct {
    const EVP_AEAD *(*aead)();
    unsigned key_len;
    fips_counter_t counter;
  } kTests[] = {
      {
          EVP_aead_aes_128_gcm,
          16,
          fips_counter_evp_aes_128_gcm,
      },
      {
          EVP_aead_aes_256_gcm,
          32,
          fips_counter_evp_aes_256_gcm,
      },
  };

  uint8_t key[EVP_AEAD_MAX_KEY_LENGTH] = {0};
  CounterArray before, after;
  for (const auto &test : kTests) {
    ASSERT_LE(test.key_len, sizeof(key));

    read_all_counters(before);
    bssl::ScopedEVP_AEAD_CTX ctx;
    ASSERT_TRUE(EVP_AEAD_CTX_init(ctx.get(), test.aead(), key, test.key_len,
                                  EVP_AEAD_DEFAULT_TAG_LENGTH,
                                  /*engine=*/nullptr));
    read_all_counters(after);

    expect_counter_delta_is_zero_except_for_a_one_at(before, after,
                                                     test.counter);
  }
}

#endif  // BORINGSSL_FIPS_COUNTERS

#if defined(BORINGSSL_FIPS)
TEST(CryptoTest, FIPSdownstreamPrecompilationFlag) {
#if defined(AWSLC_FIPS)
  ASSERT_TRUE(1);
#else
  ASSERT_TRUE(0);
#endif
}
#endif // defined(BORINGSSL_FIPS)

#if defined(BORINGSSL_FIPS_140_3)
TEST(Crypto, QueryAlgorithmStatus) {
#if defined(BORINGSSL_FIPS)
  const bool is_fips_build = true;
#else
  const bool is_fips_build = false;
#endif

  EXPECT_EQ(FIPS_query_algorithm_status("AES-GCM"), is_fips_build);
  EXPECT_EQ(FIPS_query_algorithm_status("AES-ECB"), is_fips_build);

  EXPECT_FALSE(FIPS_query_algorithm_status("FakeEncrypt"));
  EXPECT_FALSE(FIPS_query_algorithm_status(""));
}
#endif //BORINGSSL_FIPS_140_3

#if defined(BORINGSSL_FIPS) && !defined(OPENSSL_ASAN)
TEST(Crypto, OnDemandIntegrityTest) {
  BORINGSSL_integrity_test();
}
#endif

OPENSSL_DEPRECATED static void DeprecatedFunction() {}

OPENSSL_BEGIN_ALLOW_DEPRECATED
TEST(CryptoTest, DeprecatedFunction) {
  // This is deprecated, but should not trigger any warnings.
  DeprecatedFunction();
}
OPENSSL_END_ALLOW_DEPRECATED
