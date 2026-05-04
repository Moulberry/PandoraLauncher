/* Copyright (c) 2015, Google Inc.
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

#include "test_util.h"

#include <fstream>
#include <ostream>
#include <inttypes.h>

#include <openssl/err.h>

#include <thread>
#if !defined(OPENSSL_WINDOWS)
 #include <sys/wait.h>
#endif

#include <inttypes.h>

#include <openssl/err.h>

#include "../internal.h"
#include "../ube/fork_ube_detect.h"
#include "openssl/pem.h"
#include "openssl/rand.h"


void hexdump(FILE *fp, const char *msg, const void *in, size_t len) {
  const uint8_t *data = reinterpret_cast<const uint8_t *>(in);

  fputs(msg, fp);
  for (size_t i = 0; i < len; i++) {
    fprintf(fp, "%02x", data[i]);
  }
  fputs("\n", fp);
}

std::ostream &operator<<(std::ostream &os, const Bytes &in) {
  if (in.span_.empty()) {
    return os << "<empty Bytes>";
  }

  // Print a byte slice as hex.
  os << EncodeHex(in.span_);
  return os;
}

bool DecodeHex(std::vector<uint8_t> *out, const std::string &in) {
  out->clear();
  if (in.size() % 2 != 0) {
    return false;
  }
  out->reserve(in.size() / 2);
  for (size_t i = 0; i < in.size(); i += 2) {
    uint8_t hi, lo;
    if (!OPENSSL_fromxdigit(&hi, in[i]) ||
        !OPENSSL_fromxdigit(&lo, in[i + 1])) {
      return false;
    }
    out->push_back((hi << 4) | lo);
  }
  return true;
}

std::vector<uint8_t> HexToBytes(const char *str) {
  std::vector<uint8_t> ret;
  if (!DecodeHex(&ret, str)) {
    abort();
  }
  return ret;
}

std::string EncodeHex(bssl::Span<const uint8_t> in) {
  static const char kHexDigits[] = "0123456789abcdef";
  std::string ret;
  ret.reserve(in.size() * 2);
  for (uint8_t b : in) {
    ret += kHexDigits[b >> 4];
    ret += kHexDigits[b & 0xf];
  }
  return ret;
}

testing::AssertionResult ErrorEquals(uint32_t err, int lib, int reason) {
  if (ERR_GET_LIB(err) == lib && ERR_GET_REASON(err) == reason) {
    return testing::AssertionSuccess();
  }

  char buf[128], expected[128];
  return testing::AssertionFailure()
         << "Got \"" << ERR_error_string_n(err, buf, sizeof(buf))
         << "\", wanted \""
         << ERR_error_string_n(ERR_PACK(lib, reason), expected,
                               sizeof(expected))
         << "\"";
}
// CertFromPEM parses the given, NUL-terminated pem block and returns an
// |X509*|.
bssl::UniquePtr<X509> CertFromPEM(const char *pem) {
  bssl::UniquePtr<BIO> bio(BIO_new_mem_buf(pem, strlen(pem)));
  if (!bio) {
    return nullptr;
  }
  return bssl::UniquePtr<X509>(
      PEM_read_bio_X509(bio.get(), nullptr, nullptr, nullptr));
}

bssl::UniquePtr<RSA> RSAFromPEM(const char *pem) {
  bssl::UniquePtr<BIO> bio(BIO_new_mem_buf(pem, strlen(pem)));
  if (!bio) {
    return nullptr;
  }
  return bssl::UniquePtr<RSA>(
      PEM_read_bio_RSAPrivateKey(bio.get(), nullptr, nullptr, nullptr));
}

bssl::UniquePtr<X509> MakeTestCert(const char *issuer,
                                          const char *subject, EVP_PKEY *key,
                                          bool is_ca) {
  bssl::UniquePtr<X509> cert(X509_new());
  if (!cert ||  //
      !X509_set_version(cert.get(), X509_VERSION_3) ||
      !X509_NAME_add_entry_by_txt(
          X509_get_issuer_name(cert.get()), "CN", MBSTRING_UTF8,
          reinterpret_cast<const uint8_t *>(issuer), -1, -1, 0) ||
      !X509_NAME_add_entry_by_txt(
          X509_get_subject_name(cert.get()), "CN", MBSTRING_UTF8,
          reinterpret_cast<const uint8_t *>(subject), -1, -1, 0) ||
      !X509_set_pubkey(cert.get(), key) ||
      !ASN1_TIME_adj(X509_getm_notBefore(cert.get()), kReferenceTime, -1, 0) ||
      !ASN1_TIME_adj(X509_getm_notAfter(cert.get()), kReferenceTime, 1, 0)) {
    return nullptr;
  }
  bssl::UniquePtr<BASIC_CONSTRAINTS> bc(BASIC_CONSTRAINTS_new());
  if (!bc) {
    return nullptr;
  }
  bc->ca = is_ca ? ASN1_BOOLEAN_TRUE : ASN1_BOOLEAN_FALSE;
  if (!X509_add1_ext_i2d(cert.get(), NID_basic_constraints, bc.get(),
                         /*crit=*/1, /*flags=*/0)) {
    return nullptr;
  }
  return cert;
}

bssl::UniquePtr<STACK_OF(X509)> CertsToStack(
    const std::vector<X509 *> &certs) {
  bssl::UniquePtr<STACK_OF(X509)> stack(sk_X509_new_null());
  if (!stack) {
    return nullptr;
  }
  for (auto cert : certs) {
    if (!bssl::PushToStack(stack.get(), bssl::UpRef(cert))) {
      return nullptr;
    }
  }
  return stack;
}

bool PEM_to_DER(const char *pem_str, uint8_t **out_der, long *out_der_len) {
  char *name = nullptr;
  char *header = nullptr;

  // Create BIO from memory
  bssl::UniquePtr<BIO> bio(BIO_new_mem_buf(pem_str, strlen(pem_str)));
  if (!bio) {
    return false;
  }

  // Read PEM into DER
  if (PEM_read_bio(bio.get(), &name, &header, out_der, out_der_len) <= 0) {
    OPENSSL_free(name);
    OPENSSL_free(header);
    OPENSSL_free(*out_der);
    *out_der = nullptr;
    return false;
  }

  OPENSSL_free(name);
  OPENSSL_free(header);
  return true;
}

#if defined(OPENSSL_WINDOWS)
size_t createTempFILEpath(char buffer[PATH_MAX]) {
  // On Windows, tmpfile() may attempt to create temp files in the root directory
  // of the drive, which requires Admin privileges, resulting in test failure.
  char pathname[PATH_MAX];
  if(0 == GetTempPathA(PATH_MAX, pathname)) {
    return 0;
  }
  return GetTempFileNameA(pathname, "awslctest", 0, buffer);
}

size_t createTempDirPath(char buffer[PATH_MAX]) {
  char temp_path[PATH_MAX];
  union {
    uint8_t bytes[8];
    uint64_t value;
  } random_bytes;

  // Get the temporary path
  if (0 == GetTempPathA(PATH_MAX, temp_path)) {
    return 0;
  }

  if (!RAND_bytes(random_bytes.bytes, sizeof(random_bytes.bytes))) {
    return 0;
  }

  int written = snprintf(buffer, PATH_MAX, "%s\\awslctest_%" PRIX64, temp_path, random_bytes.value);

  // Check for truncation of dirname
  if (written < 0 || written >= PATH_MAX) {
    return 0;
  }

  if (!CreateDirectoryA(buffer, NULL)) {
    return 0;
  }

  return (size_t)written;
}

FILE* createRawTempFILE() {
  char filename[PATH_MAX];
  if(createTempFILEpath(filename) == 0) {
    return nullptr;
  }
  return fopen(filename, "w+b");
}

#else
#include <cstdlib>
#include <unistd.h>
size_t createTempFILEpath(char buffer[PATH_MAX]) {
  snprintf(buffer, PATH_MAX, "awslcTestTmpFileXXXXXX");

  int fd = mkstemp(buffer);
  if (fd == -1) {
    return 0;
  }

  close(fd);
  return strnlen(buffer, PATH_MAX);
}

size_t createTempDirPath(char buffer[PATH_MAX]) {
  snprintf(buffer, PATH_MAX, "/tmp/awslcTestDirXXXXXX");
  if (mkdtemp(buffer) == NULL) {
    return 0;
  }
  return strnlen(buffer, PATH_MAX);
}

FILE* createRawTempFILE() {
  return tmpfile();
}
#endif


TempFILE createTempFILE() {
  return TempFILE(createRawTempFILE());
}

void CustomDataFree(void *parent, void *ptr, CRYPTO_EX_DATA *ad,
                           int index, long argl, void *argp) {
  free(ptr);
}

bool osIsAmazonLinux(void) {
  bool res = false;
#if defined(OPENSSL_LINUX)
  // Per https://docs.aws.amazon.com/linux/al2023/ug/naming-and-versioning.html.
  std::ifstream amazonLinuxSpecificFile("/etc/amazon-linux-release-cpe");
  if (amazonLinuxSpecificFile.is_open()) {
    // Definitely on Amazon Linux.
    amazonLinuxSpecificFile.close();
    return true;
  }

  // /etc/amazon-linux-release-cpe was introduced in AL2023. For earlier, parse
  // and read /etc/system-release-cpe.
  std::ifstream osRelease("/etc/system-release-cpe");
  if (!osRelease.is_open()) {
    return false;
  }

  std::string line;
  while (std::getline(osRelease, line)) {
    // AL2:
    // $ cat /etc/system-release-cpe
    // cpe:2.3:o:amazon:amazon_linux:2
    //
    // AL2023:
    // $ cat /etc/system-release-cpe
    // cpe:2.3:o:amazon:amazon_linux:2023
    if (line.find("amazon") != std::string::npos) {
      res = true;
    } else if (line.find("amazon_linux") != std::string::npos) {
      res = true;
    }
  }
  osRelease.close();
#endif
  return res;
}

bool threadTest(const size_t numberOfThreads, std::function<void(bool*)> testFunc) {
  bool res = true;

#if defined(OPENSSL_THREADS)
  // char to be able to pass-as-reference.
  std::vector<char> retValueVec(numberOfThreads, 0);
  std::vector<std::thread> threadVec;

  for (size_t i = 0; i < numberOfThreads; i++) {
    threadVec.emplace_back(testFunc, reinterpret_cast<bool*>(&retValueVec[i]));
  }

  for (auto& thread : threadVec) {
    thread.join();
  }

  for (size_t i = 0; i < numberOfThreads; i++) {
    if (!static_cast<bool>(retValueVec[i])) {
      fprintf(stderr, "Thread %lu failed\n", (long unsigned int) i);
      res = false;
    }
  }

#else
  testFunc(&res);
#endif

  return res;
}

bool forkAndRunTest(std::function<bool()> child_func,
  std::function<bool()> parent_func) {

#if defined(OPENSSL_WINDOWS)
  // fork() is not supported on Windows. We could potentially add support for
  // the CreateProcess API at some point.
  return false;
#else
  pid_t pid = fork();
  if (pid == 0) { // Child
    bool success = child_func();
    exit(success ? 0 : 1);
  } else if (pid > 0) { // Parent
    bool parent_success = parent_func();
    int status;
    waitpid(pid, &status, 0);
    return parent_success && WIFEXITED(status) && WEXITSTATUS(status) == 0;
  }

  // Fork failed
  return false;
#endif
}

void maybeDisableSomeForkUbeDetectMechanisms(void) {
  if (getenv("AWSLC_IGNORE_FORK_UBE_DETECTION")) {
    CRYPTO_fork_detect_ignore_wipeonfork_FOR_TESTING();
    CRYPTO_fork_detect_ignore_inheritzero_FOR_TESTING();
  }
}

bool runtimeEmulationIsIntelSde(void) {
  if (getenv("RUNTIME_EMULATION_SDE")) {
    return true;
  }
  return false;
}

bool addressSanitizerIsEnabled(void) {
#if defined(OPENSSL_ASAN)
  return true;
#else
  return false;
#endif
}
