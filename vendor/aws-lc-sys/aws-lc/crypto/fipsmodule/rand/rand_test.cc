// Copyright Amazon.com Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0 OR ISC

#include <gtest/gtest.h>


#include <stdio.h>

#include <openssl/ctrdrbg.h>
#include <openssl/mem.h>
#include <openssl/rand.h>
#include <openssl/span.h>

#include "internal.h"
#include "entropy/internal.h"
#include "../../ube/internal.h"

#include "../../test/abi_test.h"
#include "../../test/ube_test.h"
#include "../../test/test_util.h"

#if defined(OPENSSL_THREADS)
#include <array>
#include <thread>
#include <vector>
#endif

#if !defined(OPENSSL_WINDOWS)
#include <errno.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <unistd.h>
#endif


#define MAX_REQUEST_SIZE (CTR_DRBG_MAX_GENERATE_LENGTH * 2 + 1)

static const size_t global_request_len = 64;
static const size_t number_of_threads = 8;

static void test_all_exported_functions(size_t request_len, uint8_t *out_buf,
  uint8_t user_pred_res[RAND_PRED_RESISTANCE_LEN]) {
  ASSERT_TRUE(RAND_bytes(out_buf, request_len));
  ASSERT_TRUE(RAND_priv_bytes(out_buf, request_len));
  ASSERT_TRUE(RAND_pseudo_bytes(out_buf, request_len));
  ASSERT_TRUE(RAND_bytes_with_user_prediction_resistance(out_buf, request_len, user_pred_res));
}

class randTest : public::testing::Test {
  private:
    UbeBase ube_base_;

  protected:
    void SetUp() override {
      ube_base_.SetUp();

      // Ensure randomness generation state is initialized.
      uint8_t *randomness = (uint8_t *) OPENSSL_zalloc(MAX_REQUEST_SIZE);
      bssl::UniquePtr<uint8_t> deleter(randomness);
      uint8_t user_prediction_resistance[RAND_PRED_RESISTANCE_LEN] = {0};
      test_all_exported_functions(global_request_len, randomness, user_prediction_resistance);
    }

    void TearDown() override {
      ube_base_.TearDown();
    }

    bool UbeIsSupported() const {
      return ube_base_.UbeIsSupported();
    }

    void allowMockedUbe() const {
      return ube_base_.allowMockedUbe();
    }
};

static void randBasicTests(bool *returnFlag) {
  // Do not use stack arrays for these. For example, Alpine OS has too low
  // default thread stack size limit to accommodate.
  uint8_t *randomness = (uint8_t *) OPENSSL_zalloc(MAX_REQUEST_SIZE);
  bssl::UniquePtr<uint8_t> deleter(randomness);
  uint8_t user_prediction_resistance[RAND_PRED_RESISTANCE_LEN] = {0};

  for (size_t i = 0; i < 65; i++) {
    test_all_exported_functions(i, randomness, user_prediction_resistance);
  }

  for (size_t i : {CTR_DRBG_MAX_GENERATE_LENGTH-1, CTR_DRBG_MAX_GENERATE_LENGTH, CTR_DRBG_MAX_GENERATE_LENGTH + 1, CTR_DRBG_MAX_GENERATE_LENGTH * 2}) {
    test_all_exported_functions(i, randomness, user_prediction_resistance);
  }

  *returnFlag = true;
}

TEST_F(randTest, Basic) {
  ASSERT_TRUE(threadTest(number_of_threads, randBasicTests));
}

#if !defined(AWSLC_VM_UBE_TESTING)
// VM UBE testing is globally configured via a file. Predicting reseeding is
// sensitive to testing VM UBE in parallel because a UBE-triggered reseed can
// happen during execution.

static void randReseedIntervalUbeIsSupportedTests(bool *returnFlag) {
  uint8_t *randomness = (uint8_t *) OPENSSL_zalloc(CTR_DRBG_MAX_GENERATE_LENGTH * 5 + 1);
  bssl::UniquePtr<uint8_t> deleter(randomness);

  // If in a new thread, this will initialize the state.
  ASSERT_TRUE(RAND_bytes(randomness, global_request_len));

  uint64_t reseed_calls_since_initialization = get_thread_reseed_calls_since_initialization();
  uint64_t generate_calls_since_seed = get_thread_generate_calls_since_seed();

  // First check that we can predict when a reseed happens based on the current
  // number of invoked generate calls. After the loop, we expect to be one
  // invoke generate call from a reseed.
  for(size_t i = 0; i < (kCtrDrbgReseedInterval - generate_calls_since_seed); i++) {
    ASSERT_TRUE(RAND_bytes(randomness, 1));
    ASSERT_EQ(get_thread_reseed_calls_since_initialization(), reseed_calls_since_initialization);
  }
  ASSERT_TRUE(RAND_bytes(randomness, 1));
  ASSERT_EQ(get_thread_reseed_calls_since_initialization(), reseed_calls_since_initialization + 1);
  ASSERT_EQ(get_thread_generate_calls_since_seed(), 1ULL);

  ASSERT_TRUE(RAND_bytes(randomness, 1));
  ASSERT_EQ(get_thread_reseed_calls_since_initialization(), reseed_calls_since_initialization + 1);
  ASSERT_EQ(get_thread_generate_calls_since_seed(), 2ULL);

  // Should be able to perform kCtrDrbgReseedInterval-2 generate calls before a
  // reseed is emitted. Requesting
  // CTR_DRBG_MAX_GENERATE_LENGTH * (kCtrDrbgReseedInterval-2) + 1 would require
  // quite a large buffer. Instead iterate until we need
  // 5 iterations and request 5 * CTR_DRBG_MAX_GENERATE_LENGTH+1, which is a
  // much smaller buffer.
  for(size_t i = 0; i < (kCtrDrbgReseedInterval - 7); i++) {
    ASSERT_TRUE(RAND_bytes(randomness, 1));
    ASSERT_EQ(get_thread_reseed_calls_since_initialization(), reseed_calls_since_initialization + 1);
    ASSERT_EQ(get_thread_generate_calls_since_seed(), 2 + (i + 1));
  }
  ASSERT_EQ(get_thread_generate_calls_since_seed(), kCtrDrbgReseedInterval - 5);
  size_t request_len_new_reseed = CTR_DRBG_MAX_GENERATE_LENGTH * 5 + 1;
  ASSERT_TRUE(RAND_bytes(randomness, request_len_new_reseed));
  ASSERT_EQ(get_thread_reseed_calls_since_initialization(), reseed_calls_since_initialization + 2);
  ASSERT_EQ(get_thread_generate_calls_since_seed(), 1ULL);

  *returnFlag = true;
}

TEST_F(randTest, ReseedIntervalWhenUbeIsSupported) {
  if (!UbeIsSupported()) {
    GTEST_SKIP() << "UBE detection is not supported";
  }
  ASSERT_TRUE(threadTest(number_of_threads, randReseedIntervalUbeIsSupportedTests));
}

static void randReseedIntervalUbeNotSupportedTests(bool *returnFlag) {
  uint8_t *randomness = (uint8_t *) OPENSSL_zalloc(CTR_DRBG_MAX_GENERATE_LENGTH);
  bssl::UniquePtr<uint8_t> deleter(randomness);

  // If in a new thread, this will initialize the state.
  ASSERT_TRUE(RAND_bytes(randomness, global_request_len));

  uint64_t generate_calls_since_seed = get_thread_generate_calls_since_seed();
  uint64_t reseed_calls_since_initialization = get_thread_reseed_calls_since_initialization();

  if (kCtrDrbgReseedInterval - generate_calls_since_seed < 2) {
    // Ensure the reseed interval doesn't conflict with logic below.
    ASSERT_TRUE(RAND_bytes(randomness, 1));
    ASSERT_TRUE(RAND_bytes(randomness, 1));
  }

  // Each invocation of the randomness generation induce a reseed due to UBE
  // detection not being supported.
  ASSERT_TRUE(RAND_bytes(randomness, 1));
  ASSERT_EQ(get_thread_generate_calls_since_seed(), 1ULL);
  ASSERT_EQ(get_thread_reseed_calls_since_initialization(), reseed_calls_since_initialization + 1);

  ASSERT_TRUE(RAND_bytes(randomness, 1));
  ASSERT_EQ(get_thread_generate_calls_since_seed(), 1ULL);
  ASSERT_EQ(get_thread_reseed_calls_since_initialization(), reseed_calls_since_initialization + 2);

  *returnFlag = true;
}

TEST_F(randTest, ReseedIntervalWhenUbeNotSupported) {

  if (UbeIsSupported()) {
    GTEST_SKIP() << "UBE detection is supported";
  }
  ASSERT_TRUE(threadTest(number_of_threads, randReseedIntervalUbeNotSupportedTests));
}

static void MockedUbeDetection(std::function<void(uint64_t)> set_detection_method_gn) {

  const size_t request_size_one_generate = 10;
  const size_t request_size_two_generate = CTR_DRBG_MAX_GENERATE_LENGTH + 1;
  uint64_t current_reseed_calls = 0;
  uint8_t *randomness = (uint8_t *) OPENSSL_zalloc(CTR_DRBG_MAX_GENERATE_LENGTH * 5 + 1);
  bssl::UniquePtr<uint8_t> deleter(randomness);

  // Make sure things are initialized and at default values. Cache
  // current_reseed_calls last in case RAND_bytes() invokes a reseed.
  set_detection_method_gn(1);
  ASSERT_TRUE(RAND_bytes(randomness, request_size_one_generate));
  current_reseed_calls = get_thread_reseed_calls_since_initialization();

  // Bump fork generation number and expect one reseed. In addition, expect one
  // generate call since request size is less than CTR_DRBG_MAX_GENERATE_LENGTH.
  set_detection_method_gn(2);
  ASSERT_TRUE(RAND_bytes(randomness, request_size_one_generate));
  ASSERT_EQ(get_thread_reseed_calls_since_initialization(), current_reseed_calls + 1ULL);
  ASSERT_EQ(get_thread_generate_calls_since_seed(), 1ULL);

  // Bump fork generation number again and expect one reseed. In addition,
  // expect two generate call since request size is higher than
  // CTR_DRBG_MAX_GENERATE_LENGTH.
  set_detection_method_gn(3);
  ASSERT_TRUE(RAND_bytes(randomness, request_size_two_generate));
  ASSERT_EQ(get_thread_reseed_calls_since_initialization(), current_reseed_calls + 2ULL);
  ASSERT_EQ(get_thread_generate_calls_since_seed(), 2ULL);
}

TEST_F(randTest, UbeDetectionMocked) {

  allowMockedUbe();

  MockedUbeDetection(
    [](uint64_t gn) {
      set_fork_ube_generation_number_FOR_TESTING(gn);
    }
  );

  MockedUbeDetection(
    [](uint64_t gn) {
      set_vm_ube_generation_number_FOR_TESTING(static_cast<uint32_t>(gn));
    }
  );
}

#endif

// These tests are, strictly speaking, flaky, but we use large enough buffers
// that the probability of failing when we should pass is negligible.

TEST_F(randTest, NotObviouslyBroken) {
  static const uint8_t kZeros[256] = {0};

  maybeDisableSomeForkUbeDetectMechanisms();

  uint8_t buf1[256], buf2[256];
  RAND_bytes(buf1, sizeof(buf1));
  RAND_bytes(buf2, sizeof(buf2));

  EXPECT_NE(Bytes(buf1), Bytes(buf2));
  EXPECT_NE(Bytes(buf1), Bytes(kZeros));
  EXPECT_NE(Bytes(buf2), Bytes(kZeros));
}

#if !defined(OPENSSL_WINDOWS) && !defined(OPENSSL_IOS) && \
    !defined(BORINGSSL_UNSAFE_DETERMINISTIC_MODE)
static bool ForkAndRand(bssl::Span<uint8_t> out) {
  int pipefds[2];
  if (pipe(pipefds) < 0) {
    perror("pipe");
    return false;
  }

  // This is a multi-threaded process, but GTest does not run tests concurrently
  // and there currently are no threads, so this should be safe.
  pid_t child = fork();
  if (child < 0) {
    perror("fork");
    close(pipefds[0]);
    close(pipefds[1]);
    return false;
  }

  if (child == 0) {
    // This is the child. Generate entropy and write it to the parent.
    close(pipefds[0]);
    RAND_bytes(out.data(), out.size());
    while (!out.empty()) {
      ssize_t ret = write(pipefds[1], out.data(), out.size());
      if (ret < 0) {
        if (errno == EINTR) {
          continue;
        }
        perror("write");
        _exit(1);
      }
      out = out.subspan(static_cast<size_t>(ret));
    }
    _exit(0);
  }

  // This is the parent. Read the entropy from the child.
  close(pipefds[1]);
  while (!out.empty()) {
    ssize_t ret = read(pipefds[0], out.data(), out.size());
    if (ret <= 0) {
      if (ret == 0) {
        fprintf(stderr, "Unexpected EOF from child.\n");
      } else {
        if (errno == EINTR) {
          continue;
        }
        perror("read");
      }
      close(pipefds[0]);
      return false;
    }
    out = out.subspan(static_cast<size_t>(ret));
  }
  close(pipefds[0]);

  // Wait for the child to exit.
  int status;
  if (waitpid(child, &status, 0) < 0) {
    perror("waitpid");
    return false;
  }
  if (!WIFEXITED(status) || WEXITSTATUS(status) != 0) {
    fprintf(stderr, "Child did not exit cleanly.\n");
    return false;
  }

  return true;
}

TEST_F(randTest, Fork) {
  static const uint8_t kZeros[16] = {0};

  maybeDisableSomeForkUbeDetectMechanisms();

  // Draw a little entropy to initialize any internal PRNG buffering.
  uint8_t byte;
  RAND_bytes(&byte, 1);

  // Draw entropy in two child processes and the parent process. This test
  // intentionally uses smaller buffers than the others, to minimize the chance
  // of sneaking by with a large enough buffer that we've since reseeded from
  // the OS.
  //
  // All child processes should have different PRNGs, including the ones that
  // disavow fork-safety. Although they are produced by fork, they themselves do
  // not fork after that call.
  uint8_t bufs[5][16];
  ASSERT_TRUE(ForkAndRand(bufs[0]));
  ASSERT_TRUE(ForkAndRand(bufs[1]));
  ASSERT_TRUE(ForkAndRand(bufs[2]));
  ASSERT_TRUE(ForkAndRand(bufs[3]));
  RAND_bytes(bufs[4], sizeof(bufs[4]));

  // All should be different and non-zero.
  for (const auto &buf : bufs) {
    EXPECT_NE(Bytes(buf), Bytes(kZeros));
  }
  for (size_t i = 0; i < OPENSSL_ARRAY_SIZE(bufs); i++) {
    for (size_t j = 0; j < i; j++) {
      EXPECT_NE(Bytes(bufs[i]), Bytes(bufs[j]))
          << "buffers " << i << " and " << j << " matched";
    }
  }
}
#endif  // !OPENSSL_WINDOWS && !OPENSSL_IOS &&
        // !BORINGSSL_UNSAFE_DETERMINISTIC_MODE

#if defined(OPENSSL_THREADS)
static void RunConcurrentRands(size_t num_threads) {
  static const uint8_t kZeros[256] = {0};

  std::vector<std::array<uint8_t, 256>> bufs(num_threads);
  std::vector<std::thread> threads(num_threads);

  for (size_t i = 0; i < num_threads; i++) {
    threads[i] =
        std::thread([i, &bufs] { RAND_bytes(bufs[i].data(), bufs[i].size()); });
  }
  for (size_t i = 0; i < num_threads; i++) {
    threads[i].join();
  }

  for (size_t i = 0; i < num_threads; i++) {
    EXPECT_NE(Bytes(bufs[i]), Bytes(kZeros));
    for (size_t j = i + 1; j < num_threads; j++) {
      EXPECT_NE(Bytes(bufs[i]), Bytes(bufs[j]));
    }
  }
}

// Test that threads may concurrently draw entropy without tripping TSan.
TEST_F(randTest, Threads) {
  constexpr size_t kFewerThreads = 10;
  constexpr size_t kMoreThreads = 20;

  maybeDisableSomeForkUbeDetectMechanisms();

  // Draw entropy in parallel.
  RunConcurrentRands(kFewerThreads);
  // Draw entropy in parallel with higher concurrency than the previous maximum.
  RunConcurrentRands(kMoreThreads);
  // Draw entropy in parallel with lower concurrency than the previous maximum.
  RunConcurrentRands(kFewerThreads);
}
#endif  // OPENSSL_THREADS

#if defined(OPENSSL_X86_64) && defined(SUPPORTS_ABI_TEST)
TEST_F(randTest, RdrandABI) {
  if (!have_hw_rng_x86_64_for_testing()) {
    fprintf(stderr, "rdrand not supported. Skipping.\n");
    return;
  }

  uint8_t buf[32];
  CHECK_ABI_SEH(CRYPTO_rdrand_multiple8, nullptr, 0);
  CHECK_ABI_SEH(CRYPTO_rdrand_multiple8, buf, 8);
  CHECK_ABI_SEH(CRYPTO_rdrand_multiple8, buf, 16);
  CHECK_ABI_SEH(CRYPTO_rdrand_multiple8, buf, 24);
  CHECK_ABI_SEH(CRYPTO_rdrand_multiple8, buf, 32);
}
#endif  // OPENSSL_X86_64 && SUPPORTS_ABI_TEST
