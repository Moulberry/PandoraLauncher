/* Copyright (c) 2014, Google Inc.
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

#include <limits.h>
#include <stdio.h>
#include <string.h>
#include <time.h>

#include <algorithm>
#include <array>
#include <limits>
#include <string>
#include <utility>
#include <vector>

#include <gtest/gtest.h>

#include <openssl/aead.h>
#include <openssl/base64.h>
#include <openssl/bio.h>
#include <openssl/bytestring.h>
#include <openssl/cipher.h>
#include <openssl/crypto.h>
#include <openssl/curve25519.h>
#include <openssl/err.h>
#include <openssl/hmac.h>
#include <openssl/hpke.h>
#include <openssl/pem.h>
#include <openssl/rand.h>
#include <openssl/sha.h>
#include <openssl/ssl.h>
#include <openssl/x509.h>

#include "../crypto/fipsmodule/ec/internal.h"
#include "../crypto/internal.h"
#include "../crypto/test/file_util.h"
#include "../crypto/test/test_util.h"
#include "internal.h"
#include "ssl_common_test.h"

#if defined(OPENSSL_WINDOWS)
// Windows defines struct timeval in winsock2.h.
OPENSSL_MSVC_PRAGMA(warning(push, 3))
#include <winsock2.h>
OPENSSL_MSVC_PRAGMA(warning(pop))
#else
#include <sys/time.h>
#endif

#if defined(OPENSSL_THREADS)
#include <thread>
#endif

BSSL_NAMESPACE_BEGIN

namespace {

INSTANTIATE_TEST_SUITE_P(SSLTests, SSLTest, testing::ValuesIn(kSSLTestParams),
                         [](const testing::TestParamInfo<SSLTestParam> &i) {
                           if (i.param.transfer_ssl) {
                             return "SSL_Transfer";
                           } else {
                             return "NO_SSL_Transfer";
                           }
                         });

static bssl::UniquePtr<SSL_CTX> CreateContextWithCertificate(
    const SSL_METHOD *method, bssl::UniquePtr<X509> cert,
    bssl::UniquePtr<EVP_PKEY> key) {
  bssl::UniquePtr<SSL_CTX> ctx(SSL_CTX_new(method));
  if (!ctx || !cert || !key ||
      !SSL_CTX_use_certificate(ctx.get(), cert.get()) ||
      !SSL_CTX_use_PrivateKey(ctx.get(), key.get())) {
    return nullptr;
  }
  return ctx;
}

// Test that |SSL_get_client_CA_list| echoes back the configured parameter even
// before configuring as a server.
TEST(SSLTest, ClientCAList) {
  bssl::UniquePtr<SSL_CTX> ctx(SSL_CTX_new(TLS_method()));
  ASSERT_TRUE(ctx);
  bssl::UniquePtr<SSL> ssl(SSL_new(ctx.get()));
  ASSERT_TRUE(ssl);

  bssl::UniquePtr<X509_NAME> name(X509_NAME_new());
  ASSERT_TRUE(name);

  bssl::UniquePtr<X509_NAME> name_dup(X509_NAME_dup(name.get()));
  ASSERT_TRUE(name_dup);

  bssl::UniquePtr<STACK_OF(X509_NAME)> stack(sk_X509_NAME_new_null());
  ASSERT_TRUE(stack);
  ASSERT_TRUE(PushToStack(stack.get(), std::move(name_dup)));

  // |SSL_set_client_CA_list| takes ownership.
  SSL_set_client_CA_list(ssl.get(), stack.release());

  STACK_OF(X509_NAME) *result = SSL_get_client_CA_list(ssl.get());
  ASSERT_TRUE(result);
  ASSERT_EQ(1u, sk_X509_NAME_num(result));
  EXPECT_EQ(0, X509_NAME_cmp(sk_X509_NAME_value(result, 0), name.get()));
}

TEST(SSLTest, AddClientCA) {
  bssl::UniquePtr<SSL_CTX> ctx(SSL_CTX_new(TLS_method()));
  ASSERT_TRUE(ctx);
  bssl::UniquePtr<SSL> ssl(SSL_new(ctx.get()));
  ASSERT_TRUE(ssl);

  bssl::UniquePtr<X509> cert1 = GetTestCertificate();
  bssl::UniquePtr<X509> cert2 = GetChainTestCertificate();
  ASSERT_TRUE(cert1 && cert2);
  X509_NAME *name1 = X509_get_subject_name(cert1.get());
  X509_NAME *name2 = X509_get_subject_name(cert2.get());

  EXPECT_EQ(0u, sk_X509_NAME_num(SSL_get_client_CA_list(ssl.get())));

  ASSERT_TRUE(SSL_add_client_CA(ssl.get(), cert1.get()));
  ASSERT_TRUE(SSL_add_client_CA(ssl.get(), cert2.get()));

  STACK_OF(X509_NAME) *list = SSL_get_client_CA_list(ssl.get());
  ASSERT_EQ(2u, sk_X509_NAME_num(list));
  EXPECT_EQ(0, X509_NAME_cmp(sk_X509_NAME_value(list, 0), name1));
  EXPECT_EQ(0, X509_NAME_cmp(sk_X509_NAME_value(list, 1), name2));

  ASSERT_TRUE(SSL_add_client_CA(ssl.get(), cert1.get()));

  list = SSL_get_client_CA_list(ssl.get());
  ASSERT_EQ(3u, sk_X509_NAME_num(list));
  EXPECT_EQ(0, X509_NAME_cmp(sk_X509_NAME_value(list, 0), name1));
  EXPECT_EQ(0, X509_NAME_cmp(sk_X509_NAME_value(list, 1), name2));
  EXPECT_EQ(0, X509_NAME_cmp(sk_X509_NAME_value(list, 2), name1));
}


TEST(SSLTest, TLS13ExporterAvailability) {
  bssl::UniquePtr<SSL_CTX> client_ctx(SSL_CTX_new(TLS_method()));
  bssl::UniquePtr<SSL_CTX> server_ctx =
      CreateContextWithTestCertificate(TLS_method());
  ASSERT_TRUE(client_ctx);
  ASSERT_TRUE(server_ctx);
  // Configure only TLS 1.3.
  ASSERT_TRUE(SSL_CTX_set_min_proto_version(client_ctx.get(), TLS1_3_VERSION));
  ASSERT_TRUE(SSL_CTX_set_max_proto_version(client_ctx.get(), TLS1_3_VERSION));

  bssl::UniquePtr<SSL> client, server;
  ASSERT_TRUE(CreateClientAndServer(&client, &server, client_ctx.get(),
                                    server_ctx.get()));

  std::vector<uint8_t> buffer(32);
  const char *label = "EXPORTER-test-label";

  // The exporters are not available before the handshake starts.
  EXPECT_FALSE(SSL_export_keying_material(client.get(), buffer.data(),
                                          buffer.size(), label, strlen(label),
                                          nullptr, 0, 0));
  EXPECT_FALSE(SSL_export_keying_material(server.get(), buffer.data(),
                                          buffer.size(), label, strlen(label),
                                          nullptr, 0, 0));

  // Send the client's first flight of handshake messages.
  int client_ret = SSL_do_handshake(client.get());
  EXPECT_EQ(SSL_get_error(client.get(), client_ret), SSL_ERROR_WANT_READ);

  // The handshake isn't far enough for the exporters to work.
  EXPECT_FALSE(SSL_export_keying_material(client.get(), buffer.data(),
                                          buffer.size(), label, strlen(label),
                                          nullptr, 0, 0));
  EXPECT_FALSE(SSL_export_keying_material(server.get(), buffer.data(),
                                          buffer.size(), label, strlen(label),
                                          nullptr, 0, 0));

  // Send all the server's handshake messages.
  int server_ret = SSL_do_handshake(server.get());
  EXPECT_EQ(SSL_get_error(server.get(), server_ret), SSL_ERROR_WANT_READ);

  // At this point in the handshake, the server should have the exporter key
  // derived since it's sent its Finished message. The client hasn't yet
  // processed the server's handshake messages, so the exporter shouldn't be
  // available to the client.
  EXPECT_FALSE(SSL_export_keying_material(client.get(), buffer.data(),
                                          buffer.size(), label, strlen(label),
                                          nullptr, 0, 0));
  EXPECT_TRUE(SSL_export_keying_material(server.get(), buffer.data(),
                                         buffer.size(), label, strlen(label),
                                         nullptr, 0, 0));

  // Finish the handshake on the client.
  EXPECT_EQ(SSL_do_handshake(client.get()), 1);

  // The exporter should be available on both endpoints.
  EXPECT_TRUE(SSL_export_keying_material(client.get(), buffer.data(),
                                         buffer.size(), label, strlen(label),
                                         nullptr, 0, 0));
  EXPECT_TRUE(SSL_export_keying_material(server.get(), buffer.data(),
                                         buffer.size(), label, strlen(label),
                                         nullptr, 0, 0));

  // Finish the handshake on the server.
  EXPECT_EQ(SSL_do_handshake(server.get()), 1);

  // The exporter should still be available on both endpoints.
  EXPECT_TRUE(SSL_export_keying_material(client.get(), buffer.data(),
                                         buffer.size(), label, strlen(label),
                                         nullptr, 0, 0));
  EXPECT_TRUE(SSL_export_keying_material(server.get(), buffer.data(),
                                         buffer.size(), label, strlen(label),
                                         nullptr, 0, 0));
}

static void AppendSession(SSL_SESSION *session, void *arg) {
  std::vector<SSL_SESSION *> *out =
      reinterpret_cast<std::vector<SSL_SESSION *> *>(arg);
  out->push_back(session);
}

// CacheEquals returns true if |ctx|'s session cache consists of |expected|, in
// order.
static bool CacheEquals(SSL_CTX *ctx,
                        const std::vector<SSL_SESSION *> &expected) {
  // Check the linked list.
  SSL_SESSION *ptr = ctx->session_cache_head;
  for (SSL_SESSION *session : expected) {
    if (ptr != session) {
      return false;
    }
    // Redundant w/ above, but avoids static analysis failure
    if (ptr == nullptr) {
      return false;
    }
    // TODO(davidben): This is an absurd way to denote the end of the list.
    if (ptr->next ==
        reinterpret_cast<SSL_SESSION *>(&ctx->session_cache_tail)) {
      ptr = nullptr;
    } else {
      ptr = ptr->next;
    }
  }
  if (ptr != nullptr) {
    return false;
  }

  // Check the hash table.
  std::vector<SSL_SESSION *> actual, expected_copy;
  lh_SSL_SESSION_doall_arg(ctx->sessions, AppendSession, &actual);
  expected_copy = expected;

  std::sort(actual.begin(), actual.end());
  std::sort(expected_copy.begin(), expected_copy.end());

  return actual == expected_copy;
}

static bssl::UniquePtr<SSL_SESSION> CreateTestSession(uint32_t number) {
  bssl::UniquePtr<SSL_CTX> ssl_ctx(SSL_CTX_new(TLS_method()));
  if (!ssl_ctx) {
    return nullptr;
  }
  bssl::UniquePtr<SSL_SESSION> ret(SSL_SESSION_new(ssl_ctx.get()));
  if (!ret) {
    return nullptr;
  }

  uint8_t id[SSL3_SSL_SESSION_ID_LENGTH] = {0};
  OPENSSL_memcpy(id, &number, sizeof(number));
  if (!SSL_SESSION_set1_id(ret.get(), id, sizeof(id))) {
    return nullptr;
  }
  return ret;
}

// Test that the internal session cache behaves as expected.
TEST(SSLTest, InternalSessionCache) {
  bssl::UniquePtr<SSL_CTX> ctx(SSL_CTX_new(TLS_method()));
  ASSERT_TRUE(ctx);

  // Prepare 10 test sessions.
  std::vector<bssl::UniquePtr<SSL_SESSION>> sessions;
  for (int i = 0; i < 10; i++) {
    bssl::UniquePtr<SSL_SESSION> session = CreateTestSession(i);
    ASSERT_TRUE(session);
    sessions.push_back(std::move(session));
  }

  SSL_CTX_sess_set_cache_size(ctx.get(), 5);

  // Insert all the test sessions.
  for (const auto &session : sessions) {
    ASSERT_TRUE(SSL_CTX_add_session(ctx.get(), session.get()));
  }

  // Only the last five should be in the list.
  ASSERT_TRUE(CacheEquals(
      ctx.get(), {sessions[9].get(), sessions[8].get(), sessions[7].get(),
                  sessions[6].get(), sessions[5].get()}));

  // Inserting an element already in the cache should fail and leave the cache
  // unchanged.
  ASSERT_FALSE(SSL_CTX_add_session(ctx.get(), sessions[7].get()));
  ASSERT_TRUE(CacheEquals(
      ctx.get(), {sessions[9].get(), sessions[8].get(), sessions[7].get(),
                  sessions[6].get(), sessions[5].get()}));

  // Although collisions should be impossible (256-bit session IDs), the cache
  // must handle them gracefully.
  bssl::UniquePtr<SSL_SESSION> collision(CreateTestSession(7));
  ASSERT_TRUE(collision);
  ASSERT_TRUE(SSL_CTX_add_session(ctx.get(), collision.get()));
  ASSERT_TRUE(CacheEquals(
      ctx.get(), {collision.get(), sessions[9].get(), sessions[8].get(),
                  sessions[6].get(), sessions[5].get()}));

  // Removing sessions behaves correctly.
  ASSERT_TRUE(SSL_CTX_remove_session(ctx.get(), sessions[6].get()));
  ASSERT_TRUE(CacheEquals(ctx.get(), {collision.get(), sessions[9].get(),
                                      sessions[8].get(), sessions[5].get()}));

  // Removing sessions requires an exact match.
  ASSERT_FALSE(SSL_CTX_remove_session(ctx.get(), sessions[0].get()));
  ASSERT_FALSE(SSL_CTX_remove_session(ctx.get(), sessions[7].get()));

  // The cache remains unchanged.
  ASSERT_TRUE(CacheEquals(ctx.get(), {collision.get(), sessions[9].get(),
                                      sessions[8].get(), sessions[5].get()}));
}

static const uint8_t kTestName[] = {
    0x30, 0x45, 0x31, 0x0b, 0x30, 0x09, 0x06, 0x03, 0x55, 0x04, 0x06, 0x13,
    0x02, 0x41, 0x55, 0x31, 0x13, 0x30, 0x11, 0x06, 0x03, 0x55, 0x04, 0x08,
    0x0c, 0x0a, 0x53, 0x6f, 0x6d, 0x65, 0x2d, 0x53, 0x74, 0x61, 0x74, 0x65,
    0x31, 0x21, 0x30, 0x1f, 0x06, 0x03, 0x55, 0x04, 0x0a, 0x0c, 0x18, 0x49,
    0x6e, 0x74, 0x65, 0x72, 0x6e, 0x65, 0x74, 0x20, 0x57, 0x69, 0x64, 0x67,
    0x69, 0x74, 0x73, 0x20, 0x50, 0x74, 0x79, 0x20, 0x4c, 0x74, 0x64,
};

// Test that, after seeing TLS 1.2 in response to early data, |SSL_write|
// continues to report |SSL_R_WRONG_VERSION_ON_EARLY_DATA|. See
// https://crbug.com/1078515.
TEST(SSLTest, WriteAfterWrongVersionOnEarlyData) {
  // Set up some 0-RTT-enabled contexts.
  bssl::UniquePtr<SSL_CTX> client_ctx(SSL_CTX_new(TLS_method()));
  bssl::UniquePtr<SSL_CTX> server_ctx =
      CreateContextWithTestCertificate(TLS_method());
  ASSERT_TRUE(client_ctx);
  ASSERT_TRUE(server_ctx);
  SSL_CTX_set_early_data_enabled(client_ctx.get(), 1);
  SSL_CTX_set_early_data_enabled(server_ctx.get(), 1);
  SSL_CTX_set_session_cache_mode(client_ctx.get(), SSL_SESS_CACHE_BOTH);
  SSL_CTX_set_session_cache_mode(server_ctx.get(), SSL_SESS_CACHE_BOTH);

  // Get an early-data-capable session.
  bssl::UniquePtr<SSL_SESSION> session =
      CreateClientSession(client_ctx.get(), server_ctx.get());
  ASSERT_TRUE(session);
  EXPECT_TRUE(SSL_SESSION_early_data_capable(session.get()));

  // Offer the session to the server, but now the server speaks TLS 1.2.
  bssl::UniquePtr<SSL> client, server;
  ASSERT_TRUE(CreateClientAndServer(&client, &server, client_ctx.get(),
                                    server_ctx.get()));
  SSL_set_session(client.get(), session.get());
  EXPECT_TRUE(SSL_set_max_proto_version(server.get(), TLS1_2_VERSION));

  // The client handshake initially succeeds in the early data state.
  EXPECT_EQ(1, SSL_do_handshake(client.get()));
  EXPECT_TRUE(SSL_in_early_data(client.get()));

  // The server processes the ClientHello and negotiates TLS 1.2.
  EXPECT_EQ(-1, SSL_do_handshake(server.get()));
  EXPECT_EQ(SSL_ERROR_WANT_READ, SSL_get_error(server.get(), -1));
  EXPECT_EQ(TLS1_2_VERSION, SSL_version(server.get()));

  // Capture the client's output.
  bssl::UniquePtr<BIO> mem(BIO_new(BIO_s_mem()));
  ASSERT_TRUE(mem);
  SSL_set0_wbio(client.get(), bssl::UpRef(mem).release());

  // The client processes the ServerHello and fails.
  EXPECT_EQ(-1, SSL_do_handshake(client.get()));
  EXPECT_EQ(SSL_ERROR_SSL, SSL_get_error(client.get(), -1));
  EXPECT_TRUE(ErrorEquals(ERR_get_error(), ERR_LIB_SSL,
                          SSL_R_WRONG_VERSION_ON_EARLY_DATA));

  // The client should have written an alert to the transport.
  const uint8_t *unused = nullptr;
  size_t len = 0;
  ASSERT_TRUE(BIO_mem_contents(mem.get(), &unused, &len));
  EXPECT_NE(0u, len);
  EXPECT_TRUE(BIO_reset(mem.get()));

  // Writing should fail, with the same error as the handshake.
  EXPECT_EQ(-1, SSL_write(client.get(), "a", 1));
  EXPECT_EQ(SSL_ERROR_SSL, SSL_get_error(client.get(), -1));
  EXPECT_TRUE(ErrorEquals(ERR_get_error(), ERR_LIB_SSL,
                          SSL_R_WRONG_VERSION_ON_EARLY_DATA));

  // Nothing should be written to the transport.
  ASSERT_TRUE(BIO_mem_contents(mem.get(), &unused, &len));
  EXPECT_EQ(0u, len);
}

TEST(SSLTest, SessionDuplication) {
  bssl::UniquePtr<SSL_CTX> client_ctx(SSL_CTX_new(TLS_method()));
  bssl::UniquePtr<SSL_CTX> server_ctx =
      CreateContextWithTestCertificate(TLS_method());
  ASSERT_TRUE(client_ctx);
  ASSERT_TRUE(server_ctx);

  bssl::UniquePtr<SSL> client, server;
  ASSERT_TRUE(ConnectClientAndServer(&client, &server, client_ctx.get(),
                                     server_ctx.get()));

  SSL_SESSION *session0 = SSL_get_session(client.get());
  bssl::UniquePtr<SSL_SESSION> session1 =
      bssl::SSL_SESSION_dup(session0, SSL_SESSION_DUP_ALL);
  ASSERT_TRUE(session1);

  session1->not_resumable = false;

  uint8_t *s0_bytes = nullptr, *s1_bytes = nullptr;
  size_t s0_len = 0, s1_len = 0;

  ASSERT_TRUE(SSL_SESSION_to_bytes(session0, &s0_bytes, &s0_len));
  bssl::UniquePtr<uint8_t> free_s0(s0_bytes);

  ASSERT_TRUE(SSL_SESSION_to_bytes(session1.get(), &s1_bytes, &s1_len));
  bssl::UniquePtr<uint8_t> free_s1(s1_bytes);

  EXPECT_EQ(Bytes(s0_bytes, s0_len), Bytes(s1_bytes, s1_len));
}

static void ExpectFDs(const SSL *ssl, int rfd, int wfd) {
  EXPECT_EQ(rfd, SSL_get_fd(ssl));
  EXPECT_EQ(rfd, SSL_get_rfd(ssl));
  EXPECT_EQ(wfd, SSL_get_wfd(ssl));

  // The wrapper BIOs are always equal when fds are equal, even if set
  // individually.
  if (rfd == wfd) {
    EXPECT_EQ(SSL_get_rbio(ssl), SSL_get_wbio(ssl));
  }
}

TEST(SSLTest, SetFD) {
  bssl::UniquePtr<SSL_CTX> ctx(SSL_CTX_new(TLS_method()));
  ASSERT_TRUE(ctx);

  // Test setting different read and write FDs.
  bssl::UniquePtr<SSL> ssl(SSL_new(ctx.get()));
  ASSERT_TRUE(ssl);
  EXPECT_TRUE(SSL_set_rfd(ssl.get(), 1));
  EXPECT_TRUE(SSL_set_wfd(ssl.get(), 2));
  ExpectFDs(ssl.get(), 1, 2);

  // Test setting the same FD.
  ssl.reset(SSL_new(ctx.get()));
  ASSERT_TRUE(ssl);
  EXPECT_TRUE(SSL_set_fd(ssl.get(), 1));
  ExpectFDs(ssl.get(), 1, 1);

  // Test setting the same FD one side at a time.
  ssl.reset(SSL_new(ctx.get()));
  ASSERT_TRUE(ssl);
  EXPECT_TRUE(SSL_set_rfd(ssl.get(), 1));
  EXPECT_TRUE(SSL_set_wfd(ssl.get(), 1));
  ExpectFDs(ssl.get(), 1, 1);

  // Test setting the same FD in the other order.
  ssl.reset(SSL_new(ctx.get()));
  ASSERT_TRUE(ssl);
  EXPECT_TRUE(SSL_set_wfd(ssl.get(), 1));
  EXPECT_TRUE(SSL_set_rfd(ssl.get(), 1));
  ExpectFDs(ssl.get(), 1, 1);

  // Test changing the read FD partway through.
  ssl.reset(SSL_new(ctx.get()));
  ASSERT_TRUE(ssl);
  EXPECT_TRUE(SSL_set_fd(ssl.get(), 1));
  EXPECT_TRUE(SSL_set_rfd(ssl.get(), 2));
  ExpectFDs(ssl.get(), 2, 1);

  // Test changing the write FD partway through.
  ssl.reset(SSL_new(ctx.get()));
  ASSERT_TRUE(ssl);
  EXPECT_TRUE(SSL_set_fd(ssl.get(), 1));
  EXPECT_TRUE(SSL_set_wfd(ssl.get(), 2));
  ExpectFDs(ssl.get(), 1, 2);

  // Test a no-op change to the read FD partway through.
  ssl.reset(SSL_new(ctx.get()));
  ASSERT_TRUE(ssl);
  EXPECT_TRUE(SSL_set_fd(ssl.get(), 1));
  EXPECT_TRUE(SSL_set_rfd(ssl.get(), 1));
  ExpectFDs(ssl.get(), 1, 1);

  // Test a no-op change to the write FD partway through.
  ssl.reset(SSL_new(ctx.get()));
  ASSERT_TRUE(ssl);
  EXPECT_TRUE(SSL_set_fd(ssl.get(), 1));
  EXPECT_TRUE(SSL_set_wfd(ssl.get(), 1));
  ExpectFDs(ssl.get(), 1, 1);

  // ASan builds will implicitly test that the internal |BIO| reference-counting
  // is correct.
}

TEST(SSLTest, SetBIO) {
  bssl::UniquePtr<SSL_CTX> ctx(SSL_CTX_new(TLS_method()));
  ASSERT_TRUE(ctx);

  bssl::UniquePtr<SSL> ssl(SSL_new(ctx.get()));
  bssl::UniquePtr<BIO> bio1(BIO_new(BIO_s_mem())), bio2(BIO_new(BIO_s_mem())),
      bio3(BIO_new(BIO_s_mem()));
  ASSERT_TRUE(ssl);
  ASSERT_TRUE(bio1);
  ASSERT_TRUE(bio2);
  ASSERT_TRUE(bio3);

  // SSL_set_bio takes one reference when the parameters are the same.
  BIO_up_ref(bio1.get());
  SSL_set_bio(ssl.get(), bio1.get(), bio1.get());

  // Repeating the call does nothing.
  SSL_set_bio(ssl.get(), bio1.get(), bio1.get());

  // It takes one reference each when the parameters are different.
  BIO_up_ref(bio2.get());
  BIO_up_ref(bio3.get());
  SSL_set_bio(ssl.get(), bio2.get(), bio3.get());

  // Repeating the call does nothing.
  SSL_set_bio(ssl.get(), bio2.get(), bio3.get());

  // It takes one reference when changing only wbio.
  BIO_up_ref(bio1.get());
  SSL_set_bio(ssl.get(), bio2.get(), bio1.get());

  // It takes one reference when changing only rbio and the two are different.
  BIO_up_ref(bio3.get());
  SSL_set_bio(ssl.get(), bio3.get(), bio1.get());

  // If setting wbio to rbio, it takes no additional references.
  SSL_set_bio(ssl.get(), bio3.get(), bio3.get());

  // From there, wbio may be switched to something else.
  BIO_up_ref(bio1.get());
  SSL_set_bio(ssl.get(), bio3.get(), bio1.get());

  // If setting rbio to wbio, it takes no additional references.
  SSL_set_bio(ssl.get(), bio1.get(), bio1.get());

  // From there, rbio may be switched to something else, but, for historical
  // reasons, it takes a reference to both parameters.
  BIO_up_ref(bio1.get());
  BIO_up_ref(bio2.get());
  SSL_set_bio(ssl.get(), bio2.get(), bio1.get());

  // ASAN builds will implicitly test that the internal |BIO| reference-counting
  // is correct.
}



// Tests that our ClientHellos do not change unexpectedly. These are purely
// change detection tests. If they fail as part of an intentional ClientHello
// change, update the test vector.
TEST(SSLTest, ClientHello) {
  struct {
    uint16_t max_version;
    std::vector<uint8_t> expected;
  } kTests[] = {
      {TLS1_VERSION,
       {0x16, 0x03, 0x01, 0x00, 0x58, 0x01, 0x00, 0x00, 0x54, 0x03, 0x01, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x0c, 0xc0, 0x09,
        0xc0, 0x13, 0xc0, 0x0a, 0xc0, 0x14, 0x00, 0x2f, 0x00, 0x35, 0x01, 0x00,
        0x00, 0x1f, 0x00, 0x17, 0x00, 0x00, 0xff, 0x01, 0x00, 0x01, 0x00, 0x00,
        0x0a, 0x00, 0x08, 0x00, 0x06, 0x00, 0x1d, 0x00, 0x17, 0x00, 0x18, 0x00,
        0x0b, 0x00, 0x02, 0x01, 0x00, 0x00, 0x23, 0x00, 0x00}},
      {TLS1_1_VERSION,
       {0x16, 0x03, 0x01, 0x00, 0x58, 0x01, 0x00, 0x00, 0x54, 0x03, 0x02, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x0c, 0xc0, 0x09,
        0xc0, 0x13, 0xc0, 0x0a, 0xc0, 0x14, 0x00, 0x2f, 0x00, 0x35, 0x01, 0x00,
        0x00, 0x1f, 0x00, 0x17, 0x00, 0x00, 0xff, 0x01, 0x00, 0x01, 0x00, 0x00,
        0x0a, 0x00, 0x08, 0x00, 0x06, 0x00, 0x1d, 0x00, 0x17, 0x00, 0x18, 0x00,
        0x0b, 0x00, 0x02, 0x01, 0x00, 0x00, 0x23, 0x00, 0x00}},
      {TLS1_2_VERSION,
       {0x16, 0x03, 0x01, 0x00, 0x88, 0x01, 0x00, 0x00, 0x84, 0x03, 0x03, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x22, 0xcc, 0xa9,
        0xcc, 0xa8, 0xc0, 0x2b, 0xc0, 0x2f, 0xc0, 0x2c, 0xc0, 0x30, 0xc0, 0x09,
        0xc0, 0x13, 0xc0, 0x27, 0xc0, 0x0a, 0xc0, 0x14, 0xc0, 0x28, 0x00, 0x9c,
        0x00, 0x9d, 0x00, 0x2f, 0x00, 0x3c, 0x00, 0x35, 0x01, 0x00, 0x00, 0x39,
        0x00, 0x17, 0x00, 0x00, 0xff, 0x01, 0x00, 0x01, 0x00, 0x00, 0x0a, 0x00,
        0x08, 0x00, 0x06, 0x00, 0x1d, 0x00, 0x17, 0x00, 0x18, 0x00, 0x0b, 0x00,
        0x02, 0x01, 0x00, 0x00, 0x23, 0x00, 0x00, 0x00, 0x0d, 0x00, 0x16, 0x00,
        0x14, 0x04, 0x03, 0x08, 0x04, 0x04, 0x01, 0x05, 0x03, 0x08, 0x05, 0x05,
        0x01, 0x06, 0x03, 0x08, 0x06, 0x06, 0x01, 0x02, 0x01}},
      {TLS1_3_VERSION,
       {0x16, 0x03, 0x01, 0x00, 0xeb, 0x01, 0x00, 0x00, 0xe7, 0x03, 0x03, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x28, 0x13, 0x01, 0x13, 0x02, 0x13, 0x03,
        0xcc, 0xa9, 0xcc, 0xa8, 0xc0, 0x2b, 0xc0, 0x2f, 0xc0, 0x2c, 0xc0, 0x30,
        0xc0, 0x09, 0xc0, 0x13, 0xc0, 0x27, 0xc0, 0x0a, 0xc0, 0x14, 0xc0, 0x28,
        0x00, 0x9c, 0x00, 0x9d, 0x00, 0x2f, 0x00, 0x3c, 0x00, 0x35, 0x01, 0x00,
        0x00, 0x76, 0x00, 0x17, 0x00, 0x00, 0xff, 0x01, 0x00, 0x01, 0x00, 0x00,
        0x0a, 0x00, 0x08, 0x00, 0x06, 0x00, 0x1d, 0x00, 0x17, 0x00, 0x18, 0x00,
        0x0b, 0x00, 0x02, 0x01, 0x00, 0x00, 0x23, 0x00, 0x00, 0x00, 0x0d, 0x00,
        0x16, 0x00, 0x14, 0x04, 0x03, 0x08, 0x04, 0x04, 0x01, 0x05, 0x03, 0x08,
        0x05, 0x05, 0x01, 0x06, 0x03, 0x08, 0x06, 0x06, 0x01, 0x02, 0x01, 0x00,
        0x33, 0x00, 0x26, 0x00, 0x24, 0x00, 0x1d, 0x00, 0x20, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x2d, 0x00, 0x02, 0x01, 0x01, 0x00,
        0x2b, 0x00, 0x09, 0x08, 0x03, 0x04, 0x03, 0x03, 0x03, 0x02, 0x03, 0x01}},
  };

  for (const auto &t : kTests) {
    SCOPED_TRACE(t.max_version);

    bssl::UniquePtr<SSL_CTX> ctx(SSL_CTX_new(TLS_method()));
    ASSERT_TRUE(ctx);
    // Our default cipher list varies by CPU capabilities, so manually place the
    // ChaCha20 ciphers in front.
    const char *cipher_list = "CHACHA20:ALL";
    ASSERT_TRUE(SSL_CTX_set_max_proto_version(ctx.get(), t.max_version));
    ASSERT_TRUE(SSL_CTX_set_strict_cipher_list(ctx.get(), cipher_list));

    // Explicitly set TLS 1.3 ciphersuites so CPU capabilities don't affect
    // order
    ASSERT_TRUE(
        SSL_CTX_set_ciphersuites(ctx.get(), TLS13_DEFAULT_CIPHER_LIST_AES_HW));

    bssl::UniquePtr<SSL> ssl(SSL_new(ctx.get()));
    ASSERT_TRUE(ssl);
    std::vector<uint8_t> client_hello;
    ASSERT_TRUE(GetClientHello(ssl.get(), &client_hello));

    // Zero the client_random.
    constexpr size_t kRandomOffset = 1 + 2 + 2 +  // record header
                                     1 + 3 +      // handshake message header
                                     2;           // client_version

    int pre = client_hello.size();
    if (t.max_version == TLS1_3_VERSION) {
      ASSERT_GE(client_hello.size(),
                kRandomOffset + SSL3_RANDOM_SIZE + 1 + SSL3_SESSION_ID_SIZE);
      OPENSSL_memset(client_hello.data() + kRandomOffset, 0,
                     SSL3_RANDOM_SIZE + 1 + SSL3_SESSION_ID_SIZE);
      // Jump to key share extension and zero out the key
      OPENSSL_memset(client_hello.data() + 189, 0, 32);
    } else {
      ASSERT_GE(client_hello.size(), kRandomOffset + SSL3_RANDOM_SIZE);
      OPENSSL_memset(client_hello.data() + kRandomOffset, 0, SSL3_RANDOM_SIZE);
    }
    int post = client_hello.size();
    ASSERT_EQ(pre, post);

    if (client_hello != t.expected) {
      ADD_FAILURE() << "ClientHellos did not match.";
      // Print the value manually so it is easier to update the test vector.
      for (size_t i = 0; i < client_hello.size(); i += 12) {
        printf("       %c", i == 0 ? '{' : ' ');
        for (size_t j = i; j < client_hello.size() && j < i + 12; j++) {
          if (j > i) {
            printf(" ");
          }
          printf("0x%02x", client_hello[j]);
          if (j < client_hello.size() - 1) {
            printf(",");
          }
        }
        if (i + 12 >= client_hello.size()) {
          printf("}},");
        }
        printf("\n");
      }
    }
  }
}



// Test that the early callback can swap the maximum version.
TEST(SSLTest, EarlyCallbackVersionSwitch) {
  bssl::UniquePtr<SSL_CTX> server_ctx =
      CreateContextWithTestCertificate(TLS_method());
  bssl::UniquePtr<SSL_CTX> client_ctx(SSL_CTX_new(TLS_method()));
  ASSERT_TRUE(server_ctx);
  ASSERT_TRUE(client_ctx);
  ASSERT_TRUE(SSL_CTX_set_max_proto_version(client_ctx.get(), TLS1_3_VERSION));
  ASSERT_TRUE(SSL_CTX_set_max_proto_version(server_ctx.get(), TLS1_3_VERSION));

  SSL_CTX_set_select_certificate_cb(
      server_ctx.get(),
      [](const SSL_CLIENT_HELLO *client_hello) -> ssl_select_cert_result_t {
        if (!SSL_set_max_proto_version(client_hello->ssl, TLS1_2_VERSION)) {
          return ssl_select_cert_error;
        }

        return ssl_select_cert_success;
      });

  bssl::UniquePtr<SSL> client, server;
  ASSERT_TRUE(ConnectClientAndServer(&client, &server, client_ctx.get(),
                                     server_ctx.get()));
  EXPECT_EQ(TLS1_2_VERSION, SSL_version(client.get()));
}

TEST(SSLTest, SetVersion) {
  bssl::UniquePtr<SSL_CTX> ctx(SSL_CTX_new(TLS_method()));
  ASSERT_TRUE(ctx);
  bssl::UniquePtr<SSL> ssl(SSL_new(ctx.get()));
  ASSERT_TRUE(ssl);

  // Set valid TLS versions.
  for (const auto &vers : kAllVersions) {
    SCOPED_TRACE(vers.name);
    if (vers.ssl_method == VersionParam::is_tls) {
      EXPECT_TRUE(SSL_CTX_set_max_proto_version(ctx.get(), vers.version));
      EXPECT_EQ(SSL_CTX_get_max_proto_version(ctx.get()), vers.version);
      EXPECT_TRUE(SSL_CTX_set_min_proto_version(ctx.get(), vers.version));
      EXPECT_EQ(SSL_CTX_get_min_proto_version(ctx.get()), vers.version);
      EXPECT_TRUE(SSL_set_max_proto_version(ssl.get(), vers.version));
      EXPECT_EQ(SSL_get_max_proto_version(ssl.get()), vers.version);
      EXPECT_TRUE(SSL_set_min_proto_version(ssl.get(), vers.version));
      EXPECT_EQ(SSL_get_min_proto_version(ssl.get()), vers.version);
    }
  }

  // Invalid TLS versions are rejected.
  EXPECT_FALSE(SSL_CTX_set_max_proto_version(ctx.get(), DTLS1_VERSION));
  EXPECT_FALSE(SSL_CTX_set_max_proto_version(ctx.get(), 0x0200));
  EXPECT_FALSE(SSL_CTX_set_max_proto_version(ctx.get(), 0x1234));
  EXPECT_FALSE(SSL_CTX_set_min_proto_version(ctx.get(), DTLS1_VERSION));
  EXPECT_FALSE(SSL_CTX_set_min_proto_version(ctx.get(), 0x0200));
  EXPECT_FALSE(SSL_CTX_set_min_proto_version(ctx.get(), 0x1234));
  EXPECT_FALSE(SSL_set_max_proto_version(ssl.get(), 0x0200));
  EXPECT_FALSE(SSL_set_max_proto_version(ssl.get(), 0x1234));
  EXPECT_FALSE(SSL_set_min_proto_version(ssl.get(), DTLS1_VERSION));
  EXPECT_FALSE(SSL_set_min_proto_version(ssl.get(), 0x0200));
  EXPECT_FALSE(SSL_set_min_proto_version(ssl.get(), 0x1234));

  // Zero represents the default version.
  EXPECT_TRUE(SSL_CTX_set_max_proto_version(ctx.get(), 0));
  EXPECT_EQ(0, SSL_CTX_get_max_proto_version(ctx.get()));
  EXPECT_TRUE(SSL_CTX_set_min_proto_version(ctx.get(), 0));
  EXPECT_EQ(0, SSL_CTX_get_min_proto_version(ctx.get()));
  EXPECT_TRUE(SSL_set_max_proto_version(ssl.get(), 0));
  EXPECT_EQ(0, SSL_get_max_proto_version(ssl.get()));
  EXPECT_TRUE(SSL_set_min_proto_version(ssl.get(), 0));
  EXPECT_EQ(0, SSL_get_min_proto_version(ssl.get()));

  // SSL 3.0 is not available.
  EXPECT_FALSE(SSL_CTX_set_min_proto_version(ctx.get(), SSL3_VERSION));
  EXPECT_FALSE(SSL_set_min_proto_version(ssl.get(), SSL3_VERSION));

  ctx.reset(SSL_CTX_new(DTLS_method()));
  ASSERT_TRUE(ctx);
  ssl.reset(SSL_new(ctx.get()));
  ASSERT_TRUE(ssl);

  // Set valid DTLS versions.
  for (const auto &vers : kAllVersions) {
    SCOPED_TRACE(vers.name);
    if (vers.ssl_method == VersionParam::is_dtls) {
      EXPECT_TRUE(SSL_CTX_set_max_proto_version(ctx.get(), vers.version));
      EXPECT_EQ(SSL_CTX_get_max_proto_version(ctx.get()), vers.version);
      EXPECT_TRUE(SSL_CTX_set_min_proto_version(ctx.get(), vers.version));
      EXPECT_EQ(SSL_CTX_get_min_proto_version(ctx.get()), vers.version);
    }
  }

  // Invalid DTLS versions are rejected.
  EXPECT_FALSE(SSL_CTX_set_max_proto_version(ctx.get(), TLS1_VERSION));
  EXPECT_FALSE(SSL_CTX_set_max_proto_version(ctx.get(), 0xfefe /* DTLS 1.1 */));
  EXPECT_FALSE(SSL_CTX_set_max_proto_version(ctx.get(), 0xfffe /* DTLS 0.1 */));
  EXPECT_FALSE(SSL_CTX_set_max_proto_version(ctx.get(), 0x1234));
  EXPECT_FALSE(SSL_CTX_set_min_proto_version(ctx.get(), TLS1_VERSION));
  EXPECT_FALSE(SSL_CTX_set_min_proto_version(ctx.get(), 0xfefe /* DTLS 1.1 */));
  EXPECT_FALSE(SSL_CTX_set_min_proto_version(ctx.get(), 0xfffe /* DTLS 0.1 */));
  EXPECT_FALSE(SSL_CTX_set_min_proto_version(ctx.get(), 0x1234));

  // Zero represents the default version.
  EXPECT_TRUE(SSL_CTX_set_max_proto_version(ctx.get(), 0));
  EXPECT_EQ(0, SSL_CTX_get_max_proto_version(ctx.get()));
  EXPECT_TRUE(SSL_CTX_set_min_proto_version(ctx.get(), 0));
  EXPECT_EQ(0, SSL_CTX_get_min_proto_version(ctx.get()));
}

TEST(SSLTest, SetVerifyResult) {
  bssl::UniquePtr<SSL_CTX> client_ctx(SSL_CTX_new(TLS_method()));
  bssl::UniquePtr<SSL_CTX> server_ctx =
      CreateContextWithTestCertificate(TLS_method());
  ASSERT_TRUE(client_ctx);
  ASSERT_TRUE(server_ctx);

  bssl::UniquePtr<SSL> client, server;
  ASSERT_TRUE(ConnectClientAndServer(&client, &server, client_ctx.get(),
                                     server_ctx.get()));

  SSL_SESSION *client_session = SSL_get_session(client.get());
  SSL_SESSION *server_session = SSL_get_session(server.get());

  // SSL and SSL_SESSION should share the same verify_result after a handshake
  EXPECT_EQ(SSL_get_verify_result(client.get()), client_session->verify_result);
  EXPECT_EQ(SSL_get_verify_result(server.get()), server_session->verify_result);

  // Use custom verification result
  SSL_set_verify_result(client.get(), X509_V_ERR_CERT_REVOKED);
  EXPECT_EQ(X509_V_ERR_CERT_REVOKED, SSL_get_verify_result(client.get()));
  EXPECT_NE(SSL_get_verify_result(client.get()), client_session->verify_result);
}

TEST(SSLTest, BuildCertChain) {
  bssl::UniquePtr<SSL_CTX> ctx(SSL_CTX_new(TLS_method()));

  // No certificate set, so this should fail.
  EXPECT_FALSE(SSL_CTX_build_cert_chain(ctx.get(), 0));
  EXPECT_TRUE(ExpectSingleError(ERR_LIB_SSL, SSL_R_NO_CERTIFICATE_SET));

  ASSERT_TRUE(SSL_CTX_use_certificate(ctx.get(), GetLeafPublic().get()));
  ASSERT_TRUE(SSL_CTX_use_PrivateKey(ctx.get(), GetLeafKey().get()));

  // Verification will fail because there is no valid root cert available.
  EXPECT_FALSE(SSL_CTX_build_cert_chain(ctx.get(), 0));
  ERR_clear_error();

  // Should return 2 when |SSL_BUILD_CHAIN_FLAG_IGNORE_ERROR| is set.
  EXPECT_EQ(
      SSL_CTX_build_cert_chain(ctx.get(), SSL_BUILD_CHAIN_FLAG_IGNORE_ERROR),
      2);
  EXPECT_TRUE(ExpectSingleError(ERR_LIB_SSL, SSL_R_CERTIFICATE_VERIFY_FAILED));
  ERR_clear_error();

  // Should return 2, but with no error on the stack when
  // |SSL_BUILD_CHAIN_FLAG_IGNORE_ERROR| and |SSL_BUILD_CHAIN_FLAG_CLEAR_ERROR|
  // are set.
  EXPECT_EQ(
      SSL_CTX_build_cert_chain(ctx.get(), SSL_BUILD_CHAIN_FLAG_IGNORE_ERROR |
                                              SSL_BUILD_CHAIN_FLAG_CLEAR_ERROR),
      2);
  EXPECT_FALSE(ERR_get_error());

  // Pass in the trust store. |SSL_CTX_build_cert_chain| should succeed now.
  ASSERT_TRUE(X509_STORE_add_cert(SSL_CTX_get_cert_store(ctx.get()),
                                  GetLeafRoot().get()));
  X509_VERIFY_PARAM_set_flags(SSL_CTX_get0_param(ctx.get()),
                              X509_V_FLAG_NO_CHECK_TIME);
  EXPECT_EQ(SSL_CTX_build_cert_chain(ctx.get(), 0), 1);
  STACK_OF(X509) *chain = nullptr;
  ASSERT_TRUE(SSL_CTX_get0_chain_certs(ctx.get(), &chain));
  EXPECT_TRUE(ChainsEqual(chain, {GetLeafRoot().get()}));

  // Root cert is self-signed, so |SSL_BUILD_CHAIN_FLAG_UNTRUSTED| will
  // still pass.
  ASSERT_TRUE(SSL_CTX_clear_chain_certs(ctx.get()));
  EXPECT_TRUE(
      SSL_CTX_build_cert_chain(ctx.get(), SSL_BUILD_CHAIN_FLAG_UNTRUSTED));
  ASSERT_TRUE(SSL_CTX_get0_chain_certs(ctx.get(), &chain));
  EXPECT_TRUE(ChainsEqual(chain, {GetLeafRoot().get()}));

  // |SSL_BUILD_CHAIN_FLAG_CHECK| uses the already built cert chain as the trust
  // store and verifies against it. If we clear the cert chain, there should be
  // no trust store to compare against if |SSL_BUILD_CHAIN_FLAG_CHECK| is still
  // set.
  EXPECT_EQ(SSL_CTX_build_cert_chain(ctx.get(), SSL_BUILD_CHAIN_FLAG_CHECK), 1);
  ASSERT_TRUE(SSL_CTX_clear_chain_certs(ctx.get()));
  EXPECT_FALSE(SSL_CTX_build_cert_chain(ctx.get(), SSL_BUILD_CHAIN_FLAG_CHECK));
  EXPECT_TRUE(ExpectSingleError(ERR_LIB_SSL, SSL_R_CERTIFICATE_VERIFY_FAILED));

  // |SSL_BUILD_CHAIN_FLAG_CHECK| and |SSL_BUILD_CHAIN_FLAG_UNTRUSTED| are
  // mutually exclusive, with |SSL_BUILD_CHAIN_FLAG_CHECK| taking priority.
  // The result with both set should be the same as only
  // |SSL_BUILD_CHAIN_FLAG_CHECK| being set.
  ASSERT_TRUE(SSL_CTX_clear_chain_certs(ctx.get()));
  EXPECT_FALSE(SSL_CTX_build_cert_chain(
      ctx.get(), SSL_BUILD_CHAIN_FLAG_CHECK | SSL_BUILD_CHAIN_FLAG_UNTRUSTED));
  EXPECT_FALSE(SSL_CTX_build_cert_chain(ctx.get(), SSL_BUILD_CHAIN_FLAG_CHECK));
  // First call with |SSL_BUILD_CHAIN_FLAG_CHECK| existing will fail, second
  // call with |SSL_BUILD_CHAIN_FLAG_UNTRUSTED| will succeed.
  EXPECT_FALSE(SSL_CTX_build_cert_chain(
      ctx.get(), SSL_BUILD_CHAIN_FLAG_CHECK | SSL_BUILD_CHAIN_FLAG_UNTRUSTED));
  EXPECT_EQ(SSL_CTX_build_cert_chain(ctx.get(), SSL_BUILD_CHAIN_FLAG_UNTRUSTED),
            1);
  // |SSL_BUILD_CHAIN_FLAG_CHECK| will succeed since we have a built chain now.
  EXPECT_EQ(SSL_CTX_build_cert_chain(ctx.get(), SSL_BUILD_CHAIN_FLAG_CHECK), 1);

  // Test that successful verification with |SSL_BUILD_CHAIN_FLAG_IGNORE_ERROR|
  // does not return 2.
  ASSERT_TRUE(SSL_CTX_clear_chain_certs(ctx.get()));
  EXPECT_EQ(
      SSL_CTX_build_cert_chain(ctx.get(), SSL_BUILD_CHAIN_FLAG_IGNORE_ERROR),
      1);

  // Test that successful verification with |SSL_BUILD_CHAIN_FLAG_NO_ROOT|
  // does include the root cert.
  ASSERT_TRUE(SSL_CTX_clear_chain_certs(ctx.get()));
  EXPECT_EQ(SSL_CTX_build_cert_chain(ctx.get(), SSL_BUILD_CHAIN_FLAG_NO_ROOT),
            1);
  ASSERT_TRUE(SSL_CTX_get0_chain_certs(ctx.get(), &chain));
  EXPECT_TRUE(ChainsEqual(chain, {}));
}

TEST(SSLTest, AddChainCertHack) {
  // Ensure that we don't accidently break the hack that we have in place to
  // keep curl and serf happy when they use an |X509| even after transfering
  // ownership.

  bssl::UniquePtr<SSL_CTX> ctx(SSL_CTX_new(TLS_method()));
  ASSERT_TRUE(ctx);
  X509 *cert = GetTestCertificate().release();
  ASSERT_TRUE(cert);
  SSL_CTX_add0_chain_cert(ctx.get(), cert);

  // This should not trigger a use-after-free.
  X509_cmp(cert, cert);
}

TEST(SSLTest, GetCertificate) {
  bssl::UniquePtr<SSL_CTX> ctx(SSL_CTX_new(TLS_method()));
  ASSERT_TRUE(ctx);
  bssl::UniquePtr<X509> cert = GetTestCertificate();
  ASSERT_TRUE(cert);
  ASSERT_TRUE(SSL_CTX_use_certificate(ctx.get(), cert.get()));
  bssl::UniquePtr<SSL> ssl(SSL_new(ctx.get()));
  ASSERT_TRUE(ssl);

  X509 *cert2 = SSL_CTX_get0_certificate(ctx.get());
  ASSERT_TRUE(cert2);

  X509 *cert3 = SSL_get_certificate(ssl.get());
  ASSERT_TRUE(cert3);

  // The old and new certificates must be identical.
  EXPECT_EQ(0, X509_cmp(cert.get(), cert2));
  EXPECT_EQ(0, X509_cmp(cert.get(), cert3));

  uint8_t *der = nullptr;
  long der_len = i2d_X509(cert.get(), &der);
  ASSERT_LT(0, der_len);
  bssl::UniquePtr<uint8_t> free_der(der);

  uint8_t *der2 = nullptr;
  long der2_len = i2d_X509(cert2, &der2);
  ASSERT_LT(0, der2_len);
  bssl::UniquePtr<uint8_t> free_der2(der2);

  uint8_t *der3 = nullptr;
  long der3_len = i2d_X509(cert3, &der3);
  ASSERT_LT(0, der3_len);
  bssl::UniquePtr<uint8_t> free_der3(der3);

  // They must also encode identically.
  EXPECT_EQ(Bytes(der, der_len), Bytes(der2, der2_len));
  EXPECT_EQ(Bytes(der, der_len), Bytes(der3, der3_len));
}

TEST(SSLTest, GetCertificateExData) {
  bssl::UniquePtr<SSL_CTX> ctx(SSL_CTX_new(TLS_method()));
  ASSERT_TRUE(ctx);
  bssl::UniquePtr<X509> cert = GetTestCertificate();
  ASSERT_TRUE(cert);

  int ex_data_index =
      X509_get_ex_new_index(0, nullptr, nullptr, nullptr, nullptr);
  const char ex_data[] = "AWS-LC external data";
  ASSERT_TRUE(X509_set_ex_data(cert.get(), ex_data_index, (void *)ex_data));
  ASSERT_TRUE(X509_get_ex_data(cert.get(), ex_data_index));

  ASSERT_TRUE(SSL_CTX_use_certificate(ctx.get(), cert.get()));
  bssl::UniquePtr<SSL> ssl(SSL_new(ctx.get()));
  ASSERT_TRUE(ssl);

  X509 *cert2 = SSL_CTX_get0_certificate(ctx.get());
  ASSERT_TRUE(cert2);
  const char *ex_data2 = (const char *)X509_get_ex_data(cert2, ex_data_index);
  EXPECT_TRUE(ex_data2);

  X509 *cert3 = SSL_get_certificate(ssl.get());
  ASSERT_TRUE(cert3);
  const char *ex_data3 = (const char *)X509_get_ex_data(cert3, ex_data_index);
  EXPECT_TRUE(ex_data3);

  // The external data extracted must be identical.
  EXPECT_EQ(ex_data2, ex_data);
  EXPECT_EQ(ex_data3, ex_data);
}

TEST(SSLTest, GetCertificateASN1) {
  bssl::UniquePtr<SSL_CTX> ctx(SSL_CTX_new(TLS_method()));
  ASSERT_TRUE(ctx);
  bssl::UniquePtr<X509> cert = GetTestCertificate();
  ASSERT_TRUE(cert);

  // Convert cert to ASN1 to pass in.
  uint8_t *der = nullptr;
  size_t der_len = i2d_X509(cert.get(), &der);
  bssl::UniquePtr<uint8_t> free_der(der);

  ASSERT_TRUE(SSL_CTX_use_certificate_ASN1(ctx.get(), der_len, der));
  bssl::UniquePtr<SSL> ssl(SSL_new(ctx.get()));
  ASSERT_TRUE(ssl);

  X509 *cert2 = SSL_CTX_get0_certificate(ctx.get());
  ASSERT_TRUE(cert2);

  X509 *cert3 = SSL_get_certificate(ssl.get());
  ASSERT_TRUE(cert3);

  // The old and new certificates must be identical.
  EXPECT_EQ(0, X509_cmp(cert.get(), cert2));
  EXPECT_EQ(0, X509_cmp(cert.get(), cert3));

  uint8_t *der2 = nullptr;
  long der2_len = i2d_X509(cert2, &der2);
  ASSERT_LT(0, der2_len);
  bssl::UniquePtr<uint8_t> free_der2(der2);

  uint8_t *der3 = nullptr;
  long der3_len = i2d_X509(cert3, &der3);
  ASSERT_LT(0, der3_len);
  bssl::UniquePtr<uint8_t> free_der3(der3);

  // They must also encode identically.
  EXPECT_EQ(Bytes(der, der_len), Bytes(der2, der2_len));
  EXPECT_EQ(Bytes(der, der_len), Bytes(der3, der3_len));
}

TEST(SSLTest, SetChainAndKeyMismatch) {
  bssl::UniquePtr<SSL_CTX> ctx(SSL_CTX_new(TLS_with_buffers_method()));
  ASSERT_TRUE(ctx);

  bssl::UniquePtr<EVP_PKEY> key = GetTestKey();
  ASSERT_TRUE(key);
  bssl::UniquePtr<CRYPTO_BUFFER> leaf = GetChainTestCertificateBuffer();
  ASSERT_TRUE(leaf);
  std::vector<CRYPTO_BUFFER *> chain = {
      leaf.get(),
  };

  // Should fail because |GetTestKey| doesn't match the chain-test certificate.
  ASSERT_FALSE(SSL_CTX_set_chain_and_key(ctx.get(), &chain[0], chain.size(),
                                         key.get(), nullptr));
  ERR_clear_error();

  // Ensure |SSL_CTX_use_cert_and_key| also fails
  bssl::UniquePtr<X509> x509_leaf =
      X509FromBuffer(GetChainTestCertificateBuffer());
  ASSERT_FALSE(
      SSL_CTX_use_cert_and_key(ctx.get(), x509_leaf.get(), key.get(), NULL, 1));
  ERR_clear_error();
}

TEST(SSLTest, SetChainAndKey) {
  bssl::UniquePtr<SSL_CTX> client_ctx(SSL_CTX_new(TLS_with_buffers_method()));
  ASSERT_TRUE(client_ctx);
  bssl::UniquePtr<SSL_CTX> server_ctx(SSL_CTX_new(TLS_with_buffers_method()));
  ASSERT_TRUE(server_ctx);

  ASSERT_EQ(nullptr, SSL_CTX_get0_chain(server_ctx.get()));

  bssl::UniquePtr<EVP_PKEY> key = GetChainTestKey();
  ASSERT_TRUE(key);
  bssl::UniquePtr<CRYPTO_BUFFER> leaf = GetChainTestCertificateBuffer();
  ASSERT_TRUE(leaf);
  bssl::UniquePtr<CRYPTO_BUFFER> intermediate =
      GetChainTestIntermediateBuffer();
  ASSERT_TRUE(intermediate);
  std::vector<CRYPTO_BUFFER *> chain = {
      leaf.get(),
      intermediate.get(),
  };
  ASSERT_TRUE(SSL_CTX_set_chain_and_key(server_ctx.get(), &chain[0],
                                        chain.size(), key.get(), nullptr));

  ASSERT_EQ(chain.size(),
            sk_CRYPTO_BUFFER_num(SSL_CTX_get0_chain(server_ctx.get())));

  SSL_CTX_set_custom_verify(
      client_ctx.get(), SSL_VERIFY_PEER,
      [](SSL *ssl, uint8_t *out_alert) -> ssl_verify_result_t {
        return ssl_verify_ok;
      });

  bssl::UniquePtr<SSL> client, server;
  ASSERT_TRUE(ConnectClientAndServer(&client, &server, client_ctx.get(),
                                     server_ctx.get()));
}

TEST(SSLTest, SetLeafChainAndKey) {
  bssl::UniquePtr<SSL_CTX> client_ctx(SSL_CTX_new(TLS_method()));
  ASSERT_TRUE(client_ctx);
  bssl::UniquePtr<SSL_CTX> server_ctx(SSL_CTX_new(TLS_method()));
  ASSERT_TRUE(server_ctx);

  ASSERT_EQ(nullptr, SSL_CTX_get0_chain(server_ctx.get()));

  bssl::UniquePtr<EVP_PKEY> key = GetChainTestKey();
  ASSERT_TRUE(key);
  bssl::UniquePtr<X509> leaf = GetChainTestCertificate();
  ASSERT_TRUE(leaf);
  bssl::UniquePtr<X509> intermediate = GetChainTestIntermediate();
  bssl::UniquePtr<STACK_OF(X509)> chain(sk_X509_new_null());
  ASSERT_TRUE(chain);
  ASSERT_TRUE(PushToStack(chain.get(), std::move(intermediate)));

  ASSERT_TRUE(SSL_CTX_use_cert_and_key(server_ctx.get(), leaf.get(), key.get(),
                                       chain.get(), 1));

  SSL_CTX_set_custom_verify(
      client_ctx.get(), SSL_VERIFY_PEER,
      [](SSL *ssl, uint8_t *out_alert) -> ssl_verify_result_t {
        return ssl_verify_ok;
      });

  bssl::UniquePtr<SSL> client, server;
  ASSERT_TRUE(ConnectClientAndServer(&client, &server, client_ctx.get(),
                                     server_ctx.get()));

  // Try setting on previously populated fields without an override
  ASSERT_FALSE(SSL_CTX_use_cert_and_key(server_ctx.get(), leaf.get(), key.get(),
                                        chain.get(), 0));
  ERR_clear_error();
}

TEST(SSLTest, BuffersFailWithoutCustomVerify) {
  bssl::UniquePtr<SSL_CTX> client_ctx(SSL_CTX_new(TLS_with_buffers_method()));
  ASSERT_TRUE(client_ctx);
  bssl::UniquePtr<SSL_CTX> server_ctx(SSL_CTX_new(TLS_with_buffers_method()));
  ASSERT_TRUE(server_ctx);

  bssl::UniquePtr<EVP_PKEY> key = GetChainTestKey();
  ASSERT_TRUE(key);
  bssl::UniquePtr<CRYPTO_BUFFER> leaf = GetChainTestCertificateBuffer();
  ASSERT_TRUE(leaf);
  std::vector<CRYPTO_BUFFER *> chain = {leaf.get()};
  ASSERT_TRUE(SSL_CTX_set_chain_and_key(server_ctx.get(), &chain[0],
                                        chain.size(), key.get(), nullptr));

  // Without SSL_CTX_set_custom_verify(), i.e. with everything in the default
  // configuration, certificate verification should fail.
  bssl::UniquePtr<SSL> client, server;
  ASSERT_FALSE(ConnectClientAndServer(&client, &server, client_ctx.get(),
                                      server_ctx.get()));

  // Whereas with a verifier, the connection should succeed.
  SSL_CTX_set_custom_verify(
      client_ctx.get(), SSL_VERIFY_PEER,
      [](SSL *ssl, uint8_t *out_alert) -> ssl_verify_result_t {
        return ssl_verify_ok;
      });
  ASSERT_TRUE(ConnectClientAndServer(&client, &server, client_ctx.get(),
                                     server_ctx.get()));
}

TEST(SSLTest, CustomVerify) {
  bssl::UniquePtr<SSL_CTX> client_ctx(SSL_CTX_new(TLS_with_buffers_method()));
  ASSERT_TRUE(client_ctx);
  bssl::UniquePtr<SSL_CTX> server_ctx(SSL_CTX_new(TLS_with_buffers_method()));
  ASSERT_TRUE(server_ctx);

  bssl::UniquePtr<EVP_PKEY> key = GetChainTestKey();
  ASSERT_TRUE(key);
  bssl::UniquePtr<CRYPTO_BUFFER> leaf = GetChainTestCertificateBuffer();
  ASSERT_TRUE(leaf);
  std::vector<CRYPTO_BUFFER *> chain = {leaf.get()};
  ASSERT_TRUE(SSL_CTX_set_chain_and_key(server_ctx.get(), &chain[0],
                                        chain.size(), key.get(), nullptr));

  SSL_CTX_set_custom_verify(
      client_ctx.get(), SSL_VERIFY_PEER,
      [](SSL *ssl, uint8_t *out_alert) -> ssl_verify_result_t {
        return ssl_verify_ok;
      });

  bssl::UniquePtr<SSL> client, server;
  ASSERT_TRUE(ConnectClientAndServer(&client, &server, client_ctx.get(),
                                     server_ctx.get()));

  // With SSL_VERIFY_PEER, ssl_verify_invalid should result in a dropped
  // connection.
  SSL_CTX_set_custom_verify(
      client_ctx.get(), SSL_VERIFY_PEER,
      [](SSL *ssl, uint8_t *out_alert) -> ssl_verify_result_t {
        return ssl_verify_invalid;
      });

  ASSERT_FALSE(ConnectClientAndServer(&client, &server, client_ctx.get(),
                                      server_ctx.get()));

  // But with SSL_VERIFY_NONE, ssl_verify_invalid should not cause a dropped
  // connection.
  SSL_CTX_set_custom_verify(
      client_ctx.get(), SSL_VERIFY_NONE,
      [](SSL *ssl, uint8_t *out_alert) -> ssl_verify_result_t {
        return ssl_verify_invalid;
      });

  ASSERT_TRUE(ConnectClientAndServer(&client, &server, client_ctx.get(),
                                     server_ctx.get()));
}

TEST(SSLTest, ClientCABuffers) {
  bssl::UniquePtr<SSL_CTX> client_ctx(SSL_CTX_new(TLS_with_buffers_method()));
  ASSERT_TRUE(client_ctx);
  bssl::UniquePtr<SSL_CTX> server_ctx(SSL_CTX_new(TLS_with_buffers_method()));
  ASSERT_TRUE(server_ctx);

  bssl::UniquePtr<EVP_PKEY> key = GetChainTestKey();
  ASSERT_TRUE(key);
  bssl::UniquePtr<CRYPTO_BUFFER> leaf = GetChainTestCertificateBuffer();
  ASSERT_TRUE(leaf);
  bssl::UniquePtr<CRYPTO_BUFFER> intermediate =
      GetChainTestIntermediateBuffer();
  ASSERT_TRUE(intermediate);
  std::vector<CRYPTO_BUFFER *> chain = {
      leaf.get(),
      intermediate.get(),
  };
  ASSERT_TRUE(SSL_CTX_set_chain_and_key(server_ctx.get(), &chain[0],
                                        chain.size(), key.get(), nullptr));

  bssl::UniquePtr<CRYPTO_BUFFER> ca_name(
      CRYPTO_BUFFER_new(kTestName, sizeof(kTestName), nullptr));
  ASSERT_TRUE(ca_name);
  bssl::UniquePtr<STACK_OF(CRYPTO_BUFFER)> ca_names(
      sk_CRYPTO_BUFFER_new_null());
  ASSERT_TRUE(ca_names);
  ASSERT_TRUE(PushToStack(ca_names.get(), std::move(ca_name)));
  SSL_CTX_set0_client_CAs(server_ctx.get(), ca_names.release());

  // Configure client and server to accept all certificates.
  SSL_CTX_set_custom_verify(
      client_ctx.get(), SSL_VERIFY_PEER,
      [](SSL *ssl, uint8_t *out_alert) -> ssl_verify_result_t {
        return ssl_verify_ok;
      });
  SSL_CTX_set_custom_verify(
      server_ctx.get(), SSL_VERIFY_PEER,
      [](SSL *ssl, uint8_t *out_alert) -> ssl_verify_result_t {
        return ssl_verify_ok;
      });

  bool cert_cb_called = false;
  SSL_CTX_set_cert_cb(
      client_ctx.get(),
      [](SSL *ssl, void *arg) -> int {
        const STACK_OF(CRYPTO_BUFFER) *peer_names =
            SSL_get0_server_requested_CAs(ssl);
        EXPECT_EQ(1u, sk_CRYPTO_BUFFER_num(peer_names));
        CRYPTO_BUFFER *peer_name = sk_CRYPTO_BUFFER_value(peer_names, 0);
        EXPECT_EQ(Bytes(kTestName), Bytes(CRYPTO_BUFFER_data(peer_name),
                                          CRYPTO_BUFFER_len(peer_name)));
        *reinterpret_cast<bool *>(arg) = true;
        return 1;
      },
      &cert_cb_called);

  bssl::UniquePtr<SSL> client, server;
  ASSERT_TRUE(ConnectClientAndServer(&client, &server, client_ctx.get(),
                                     server_ctx.get()));
  EXPECT_TRUE(cert_cb_called);
}

// Configuring the empty cipher list, though an error, should still modify the
// configuration.
TEST(SSLTest, EmptyCipherList) {
  bssl::UniquePtr<SSL_CTX> ctx(SSL_CTX_new(TLS_method()));
  ASSERT_TRUE(ctx);

  // Initially, the cipher list is not empty.
  EXPECT_NE(0u, sk_SSL_CIPHER_num(SSL_CTX_get_ciphers(ctx.get())));

  // Configuring the empty cipher list with |SSL_CTX_set_cipher_list|
  // succeeds.
  EXPECT_TRUE(SSL_CTX_set_cipher_list(ctx.get(), ""));
  // The cipher list should only be populated with default TLS 1.3 ciphersuites
  EXPECT_EQ(3u, sk_SSL_CIPHER_num(SSL_CTX_get_ciphers(ctx.get())));

  // Configuring the empty cipher list with |SSL_CTX_set_ciphersuites| works
  EXPECT_TRUE(SSL_CTX_set_ciphersuites(ctx.get(), ""));
  EXPECT_EQ(0u, sk_SSL_CIPHER_num(ctx->tls13_cipher_list->ciphers.get()));
  EXPECT_EQ(0u, sk_SSL_CIPHER_num(SSL_CTX_get_ciphers(ctx.get())));

  // Configuring the empty cipher list with |SSL_CTX_set_strict_cipher_list|
  // fails.
  EXPECT_FALSE(SSL_CTX_set_strict_cipher_list(ctx.get(), ""));
  ERR_clear_error();

  // But the cipher list is still updated to empty.
  EXPECT_EQ(0u, sk_SSL_CIPHER_num(SSL_CTX_get_ciphers(ctx.get())));
}

struct TLSVersionTestParams {
  uint16_t version;
};

const TLSVersionTestParams kTLSVersionTests[] = {
    {TLS1_VERSION},
    {TLS1_1_VERSION},
    {TLS1_2_VERSION},
    {TLS1_3_VERSION},
};

struct CertificateKeyTestParams {
  bssl::UniquePtr<X509> (*certificate)();
  bssl::UniquePtr<EVP_PKEY> (*key)();
  int slot_index;
  const char suite[50];
  uint16_t corresponding_sigalg;
};

const CertificateKeyTestParams kCertificateKeyTests[] = {
    {GetTestCertificate, GetTestKey, SSL_PKEY_RSA,
     "TLS_RSA_WITH_AES_256_CBC_SHA:", SSL_SIGN_RSA_PSS_RSAE_SHA256},
    {GetECDSATestCertificate, GetECDSATestKey, SSL_PKEY_ECC,
     "TLS_ECDHE_ECDSA_WITH_AES_256_CBC_SHA:", SSL_SIGN_ECDSA_SECP256R1_SHA256},
    {GetED25519TestCertificate, GetED25519TestKey, SSL_PKEY_ED25519,
     "TLS_ECDHE_ECDSA_WITH_AES_128_GCM_SHA256:", SSL_SIGN_ED25519},
};

class MultipleCertificateSlotTest
    : public testing::TestWithParam<
          std::tuple<TLSVersionTestParams, CertificateKeyTestParams>> {
 public:
  MultipleCertificateSlotTest() {
    this->version = version_param().version;
    this->slot_index = certificate_key_param().slot_index;
  }

  uint16_t version = 0;
  int slot_index = -1;

  static TLSVersionTestParams version_param() {
    return std::get<0>(GetParam());
  }
  static CertificateKeyTestParams certificate_key_param() {
    return std::get<1>(GetParam());
  }

  const void StandardCertificateSlotIndexTests(SSL_CTX *client_ctx,
                                               SSL_CTX *server_ctx,
                                               std::vector<uint16_t> sigalgs,
                                               int last_cert_type_set,
                                               int should_connect) {
    EXPECT_TRUE(SSL_CTX_set_signing_algorithm_prefs(client_ctx, sigalgs.data(),
                                                    sigalgs.size()));
    EXPECT_TRUE(SSL_CTX_set_verify_algorithm_prefs(client_ctx, sigalgs.data(),
                                                   sigalgs.size()));

    ASSERT_TRUE(SSL_CTX_set_min_proto_version(client_ctx, version));
    ASSERT_TRUE(SSL_CTX_set_max_proto_version(client_ctx, version));
    ASSERT_TRUE(SSL_CTX_set_min_proto_version(server_ctx, version));
    ASSERT_TRUE(SSL_CTX_set_max_proto_version(server_ctx, version));

    ClientConfig config;
    bssl::UniquePtr<SSL> client, server;

    EXPECT_EQ(ConnectClientAndServer(&client, &server, client_ctx, server_ctx,
                                     config, false),
              should_connect);
    if (!should_connect) {
      return;
    }

    ASSERT_TRUE(CompleteHandshakes(client.get(), server.get()));

    // Check the internal slot index to verify that the correct slot was set.
    // This should be the slot of the last certificate that was set in
    // |server_ctx|.
    EXPECT_EQ(server_ctx->cert->cert_private_key_idx, last_cert_type_set);
    EXPECT_EQ(server->ctx->cert->cert_private_key_idx, last_cert_type_set);

    // Check the internal slot index to verify that the correct slot was used
    // during the handshake.
    EXPECT_EQ(server->config->cert->cert_private_key_idx, slot_index);
  }
};

INSTANTIATE_TEST_SUITE_P(
    MultipleCertificateSlotAllTest, MultipleCertificateSlotTest,
    testing::Combine(testing::ValuesIn(kTLSVersionTests),
                     testing::ValuesIn(kCertificateKeyTests)));

// Sets up the |SSL_CTX| with |SSL_CTX_use_certificate| & |SSL_use_PrivateKey|.
TEST_P(MultipleCertificateSlotTest, CertificateSlotIndex) {
  if (version < TLS1_2_VERSION && slot_index == SSL_PKEY_ED25519) {
    // ED25519 is not supported in versions prior to TLS1.2.
    GTEST_SKIP();
  }
  bssl::UniquePtr<SSL_CTX> client_ctx(SSL_CTX_new(TLS_method()));
  bssl::UniquePtr<SSL_CTX> server_ctx(CreateContextWithCertificate(
      TLS_method(), certificate_key_param().certificate(),
      certificate_key_param().key()));

  StandardCertificateSlotIndexTests(
      client_ctx.get(), server_ctx.get(),
      {SSL_SIGN_ED25519, SSL_SIGN_ECDSA_SECP256R1_SHA256,
       SSL_SIGN_RSA_PSS_RSAE_SHA256},
      slot_index, true);
}

// Sets up the |SSL_CTX| with |SSL_CTX_set_chain_and_key|.
TEST_P(MultipleCertificateSlotTest, SetChainAndKeyIndex) {
  if (version < TLS1_2_VERSION && slot_index == SSL_PKEY_ED25519) {
    // ED25519 is not supported in versions prior to TLS1.2.
    GTEST_SKIP();
  }
  bssl::UniquePtr<SSL_CTX> client_ctx(SSL_CTX_new(TLS_method()));
  bssl::UniquePtr<SSL_CTX> server_ctx(SSL_CTX_new(TLS_method()));

  uint8_t *buf = nullptr;
  int cert_len = i2d_X509(certificate_key_param().certificate().get(), &buf);
  bssl::UniquePtr<uint8_t> free_buf(buf);

  bssl::UniquePtr<CRYPTO_BUFFER> leaf(
      CRYPTO_BUFFER_new(buf, cert_len, nullptr));
  ASSERT_TRUE(leaf);
  std::vector<CRYPTO_BUFFER *> chain = {leaf.get()};

  ASSERT_TRUE(
      SSL_CTX_set_chain_and_key(server_ctx.get(), &chain[0], chain.size(),
                                certificate_key_param().key().get(), nullptr));

  StandardCertificateSlotIndexTests(
      client_ctx.get(), server_ctx.get(),
      {SSL_SIGN_ED25519, SSL_SIGN_ECDSA_SECP256R1_SHA256,
       SSL_SIGN_RSA_PSS_RSAE_SHA256},
      slot_index, true);
}

TEST_P(MultipleCertificateSlotTest, AutomaticSelectionSigAlgs) {
  if (version < TLS1_2_VERSION && slot_index == SSL_PKEY_ED25519) {
    // ED25519 is not supported in versions prior to TLS1.2.
    GTEST_SKIP();
  }

  bssl::UniquePtr<SSL_CTX> client_ctx(SSL_CTX_new(TLS_method()));
  bssl::UniquePtr<SSL_CTX> server_ctx(SSL_CTX_new(TLS_method()));

  ASSERT_TRUE(
      SSL_CTX_use_certificate(server_ctx.get(), GetTestCertificate().get()));
  ASSERT_TRUE(SSL_CTX_use_PrivateKey(server_ctx.get(), GetTestKey().get()));
  ASSERT_TRUE(SSL_CTX_use_certificate(server_ctx.get(),
                                      GetECDSATestCertificate().get()));
  ASSERT_TRUE(
      SSL_CTX_use_PrivateKey(server_ctx.get(), GetECDSATestKey().get()));
  ASSERT_TRUE(SSL_CTX_use_certificate(server_ctx.get(),
                                      GetED25519TestCertificate().get()));
  ASSERT_TRUE(
      SSL_CTX_use_PrivateKey(server_ctx.get(), GetED25519TestKey().get()));


  // Versions prior to TLS1.3 need a valid authentication cipher suite to pair
  // with the certificate.
  if (version < TLS1_3_VERSION) {
    ASSERT_TRUE(SSL_CTX_set_cipher_list(client_ctx.get(),
                                        certificate_key_param().suite));
  }

  StandardCertificateSlotIndexTests(
      client_ctx.get(), server_ctx.get(),
      {certificate_key_param().corresponding_sigalg}, SSL_PKEY_ED25519, true);
}

TEST_P(MultipleCertificateSlotTest, AutomaticSelectionCipherAuth) {
  if ((version < TLS1_2_VERSION && slot_index == SSL_PKEY_ED25519) ||
      version >= TLS1_3_VERSION) {
    // ED25519 is not supported in versions prior to TLS1.2.
    // TLS 1.3 not have cipher-based authentication configuration.
    GTEST_SKIP();
  }

  bssl::UniquePtr<SSL_CTX> client_ctx(SSL_CTX_new(TLS_method()));
  bssl::UniquePtr<SSL_CTX> server_ctx(SSL_CTX_new(TLS_method()));

  ASSERT_TRUE(
      SSL_CTX_use_certificate(server_ctx.get(), GetTestCertificate().get()));
  ASSERT_TRUE(SSL_CTX_use_PrivateKey(server_ctx.get(), GetTestKey().get()));
  ASSERT_TRUE(SSL_CTX_use_certificate(server_ctx.get(),
                                      GetECDSATestCertificate().get()));
  ASSERT_TRUE(
      SSL_CTX_use_PrivateKey(server_ctx.get(), GetECDSATestKey().get()));
  ASSERT_TRUE(SSL_CTX_use_certificate(server_ctx.get(),
                                      GetED25519TestCertificate().get()));
  ASSERT_TRUE(
      SSL_CTX_use_PrivateKey(server_ctx.get(), GetED25519TestKey().get()));


  // Versions prior to TLS1.3 need a valid authentication cipher suite to pair
  // with the certificate.
  if (version < TLS1_3_VERSION) {
    ASSERT_TRUE(SSL_CTX_set_cipher_list(client_ctx.get(),
                                        certificate_key_param().suite));
  }

  // We allow all possible sigalgs in this test, but either the ECDSA or ED25519
  // certificate could be chosen when using an |SSL_aECDSA| ciphersuite.
  std::vector<uint16_t> sigalgs = {SSL_SIGN_RSA_PSS_RSAE_SHA256};
  if (slot_index == SSL_PKEY_ED25519) {
    sigalgs.push_back(SSL_SIGN_ED25519);
  } else {
    sigalgs.push_back(SSL_SIGN_ECDSA_SECP256R1_SHA256);
  }

  StandardCertificateSlotIndexTests(client_ctx.get(), server_ctx.get(), sigalgs,
                                    SSL_PKEY_ED25519, true);
}

TEST_P(MultipleCertificateSlotTest, MissingCertificate) {
  if (version < TLS1_2_VERSION && slot_index == SSL_PKEY_ED25519) {
    // ED25519 is not supported in versions prior to TLS1.2.
    GTEST_SKIP();
  }

  bssl::UniquePtr<SSL_CTX> client_ctx(SSL_CTX_new(TLS_method()));
  bssl::UniquePtr<SSL_CTX> server_ctx(SSL_CTX_new(TLS_method()));

  ASSERT_TRUE(SSL_CTX_use_PrivateKey(server_ctx.get(), GetTestKey().get()));
  ASSERT_TRUE(
      SSL_CTX_use_PrivateKey(server_ctx.get(), GetECDSATestKey().get()));
  ASSERT_TRUE(
      SSL_CTX_use_PrivateKey(server_ctx.get(), GetED25519TestKey().get()));

  // Versions prior to TLS1.3 need a valid authentication cipher suite to pair
  // with the certificate.
  if (version < TLS1_3_VERSION) {
    ASSERT_TRUE(SSL_CTX_set_cipher_list(client_ctx.get(),
                                        certificate_key_param().suite));
  }

  StandardCertificateSlotIndexTests(
      client_ctx.get(), server_ctx.get(),
      {certificate_key_param().corresponding_sigalg}, -1, false);
}

TEST_P(MultipleCertificateSlotTest, MissingPrivateKey) {
  if (version < TLS1_2_VERSION && slot_index == SSL_PKEY_ED25519) {
    // ED25519 is not supported in versions prior to TLS1.2.
    GTEST_SKIP();
  }

  bssl::UniquePtr<SSL_CTX> client_ctx(SSL_CTX_new(TLS_method()));
  bssl::UniquePtr<SSL_CTX> server_ctx(SSL_CTX_new(TLS_method()));

  ASSERT_TRUE(
      SSL_CTX_use_certificate(server_ctx.get(), GetTestCertificate().get()));
  ASSERT_TRUE(SSL_CTX_use_certificate(server_ctx.get(),
                                      GetECDSATestCertificate().get()));
  ASSERT_TRUE(SSL_CTX_use_certificate(server_ctx.get(),
                                      GetED25519TestCertificate().get()));

  // Versions prior to TLS1.3 need a valid authentication cipher suite to pair
  // with the certificate.
  if (version < TLS1_3_VERSION) {
    ASSERT_TRUE(SSL_CTX_set_cipher_list(client_ctx.get(),
                                        certificate_key_param().suite));
  }

  StandardCertificateSlotIndexTests(
      client_ctx.get(), server_ctx.get(),
      {certificate_key_param().corresponding_sigalg}, -1, false);
}


struct MultiTransferReadWriteTestParams {
  const char suite[50];
  bool tls13;
  bssl::UniquePtr<X509> (*certificate)();
  bssl::UniquePtr<EVP_PKEY> (*key)();
};

static const MultiTransferReadWriteTestParams kMultiTransferReadWriteTests[] = {
    {"TLS_AES_128_GCM_SHA256:", true, GetECDSATestCertificate, GetECDSATestKey},
    {"TLS_AES_256_GCM_SHA384:", true, GetECDSATestCertificate, GetECDSATestKey},
    {"TLS_CHACHA20_POLY1305_SHA256:", true, GetECDSATestCertificate,
     GetECDSATestKey},
    {"TLS_RSA_WITH_NULL_SHA:", false, GetTestCertificate, GetTestKey},
    {"TLS_RSA_WITH_3DES_EDE_CBC_SHA:", false, GetTestCertificate, GetTestKey},
    {"TLS_RSA_WITH_AES_128_CBC_SHA:", false, GetTestCertificate, GetTestKey},
    {"TLS_RSA_WITH_AES_256_CBC_SHA:", false, GetTestCertificate, GetTestKey},
    {"TLS_RSA_WITH_AES_128_CBC_SHA256:", false, GetTestCertificate, GetTestKey},
    {"TLS_RSA_WITH_AES_128_GCM_SHA256:", false, GetTestCertificate, GetTestKey},
    {"TLS_RSA_WITH_AES_256_GCM_SHA384:", false, GetTestCertificate, GetTestKey},
    {"TLS_ECDHE_ECDSA_WITH_AES_128_CBC_SHA:", false, GetECDSATestCertificate,
     GetECDSATestKey},
    {"TLS_ECDHE_ECDSA_WITH_AES_256_CBC_SHA:", false, GetECDSATestCertificate,
     GetECDSATestKey},
    {"TLS_ECDHE_RSA_WITH_AES_128_CBC_SHA:", false, GetTestCertificate,
     GetTestKey},
    {"TLS_ECDHE_RSA_WITH_AES_256_CBC_SHA:", false, GetTestCertificate,
     GetTestKey},
    {"TLS_ECDHE_RSA_WITH_AES_128_CBC_SHA256:", false, GetTestCertificate,
     GetTestKey},
    {"TLS_ECDHE_ECDSA_WITH_AES_128_GCM_SHA256:", false, GetECDSATestCertificate,
     GetECDSATestKey},
    {"TLS_ECDHE_ECDSA_WITH_AES_256_GCM_SHA384:", false, GetECDSATestCertificate,
     GetECDSATestKey},
    {"TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256:", false, GetTestCertificate,
     GetTestKey},
    {"TLS_ECDHE_RSA_WITH_AES_256_GCM_SHA384:", false, GetTestCertificate,
     GetTestKey},
    {"TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256:", false, GetTestCertificate,
     GetTestKey},
    {"TLS_ECDHE_ECDSA_WITH_CHACHA20_POLY1305_SHA256:", false,
     GetECDSATestCertificate, GetECDSATestKey}};

class MultiTransferReadWriteTest
    : public testing::TestWithParam<MultiTransferReadWriteTestParams> {};

INSTANTIATE_TEST_SUITE_P(SuiteTests, MultiTransferReadWriteTest,
                         testing::ValuesIn(kMultiTransferReadWriteTests));

TEST_P(MultiTransferReadWriteTest, SuiteTransfers) {
  auto params = GetParam();

  bssl::UniquePtr<SSL_CTX> client_ctx(SSL_CTX_new(TLS_method()));
  bssl::UniquePtr<SSL_CTX> server_ctx(CreateContextWithCertificate(
      TLS_method(), params.certificate(), params.key()));

  uint16_t version = TLS1_2_VERSION;
  int (*set_cipher_suites)(SSL_CTX *, const char *) = SSL_CTX_set_cipher_list;
  if (params.tls13) {
    version = TLS1_3_VERSION;
    set_cipher_suites = SSL_CTX_set_ciphersuites;
  }

  ASSERT_TRUE(set_cipher_suites(client_ctx.get(), params.suite));
  ASSERT_TRUE(set_cipher_suites(server_ctx.get(), params.suite));

  ASSERT_TRUE(SSL_CTX_set_min_proto_version(client_ctx.get(), version));
  ASSERT_TRUE(SSL_CTX_set_max_proto_version(client_ctx.get(), version));
  ASSERT_TRUE(SSL_CTX_set_min_proto_version(server_ctx.get(), version));
  ASSERT_TRUE(SSL_CTX_set_max_proto_version(server_ctx.get(), version));

  ClientConfig config;
  bssl::UniquePtr<SSL> client, server;

  ASSERT_TRUE(ConnectClientAndServer(&client, &server, client_ctx.get(),
                                     server_ctx.get(), config, true));

  ASSERT_TRUE(CompleteHandshakes(client.get(), server.get()));

  bssl::UniquePtr<SSL> transfer_server;
  TransferSSL(&server, server_ctx.get(), &transfer_server, false);

  // All Read/Write connection operations should fail now for a transferred server
  ASSERT_EQ(-1, SSL_connect(server.get()));
  ASSERT_EQ(ERR_GET_REASON(ERR_get_error()), ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
  ERR_clear_error();

  ASSERT_EQ(-1, SSL_accept(server.get()));
  ASSERT_EQ(ERR_GET_REASON(ERR_get_error()), ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
  ERR_clear_error();

  ASSERT_EQ(-1, SSL_shutdown(server.get()));
  ASSERT_EQ(ERR_GET_REASON(ERR_get_error()), ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
  ERR_clear_error();

  ASSERT_EQ(-1, SSL_do_handshake(server.get()));
  ASSERT_EQ(ERR_GET_REASON(ERR_get_error()), ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
  ERR_clear_error();

  ASSERT_EQ(-1, SSL_peek(server.get(), nullptr, 0));
  ASSERT_EQ(ERR_GET_REASON(ERR_get_error()), ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
  ERR_clear_error();

  ASSERT_EQ(0, SSL_peek_ex(server.get(), nullptr, 0, nullptr));
  ASSERT_EQ(ERR_GET_REASON(ERR_get_error()), ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
  ERR_clear_error();

  ASSERT_EQ(-1, SSL_read(server.get(), nullptr, 0));
  ASSERT_EQ(ERR_GET_REASON(ERR_get_error()), ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
  ERR_clear_error();

  ASSERT_EQ(0, SSL_read_ex(server.get(), nullptr, 0, nullptr));
  ASSERT_EQ(ERR_GET_REASON(ERR_get_error()), ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
  ERR_clear_error();

  ASSERT_EQ(-1, SSL_write(server.get(), nullptr, 0));
  ASSERT_EQ(ERR_GET_REASON(ERR_get_error()), ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
  ERR_clear_error();

  ASSERT_EQ(0, SSL_write_ex(server.get(), nullptr, 0, nullptr));
  ASSERT_EQ(ERR_GET_REASON(ERR_get_error()), ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
  ERR_clear_error();

  ASSERT_EQ(0, SSL_key_update(server.get(), SSL_KEY_UPDATE_REQUESTED));
  ASSERT_EQ(ERR_GET_REASON(ERR_get_error()), ERR_R_SHOULD_NOT_HAVE_BEEN_CALLED);
  ERR_clear_error();

  server = std::move(transfer_server);

  char buf[3];
  size_t buf_cap = sizeof(buf);

  size_t server_transfers = 5;
  size_t read_write_per_instance = 20;

  for (size_t t = 0; t < server_transfers; t++) {
    for (size_t i = 0; i < read_write_per_instance; i++) {
      std::string send_str = std::to_string(i);

      if(t > 0 && i == 0) {
        // Actually read the previously peeked data from the last transfer
        std::string peeked_str = std::to_string(read_write_per_instance-1);
        int read = SSL_read(server.get(), buf, buf_cap);
        ASSERT_TRUE(read);
        ASSERT_TRUE((size_t)read == peeked_str.length());
        std::string read_str(buf, read);
        ASSERT_EQ(peeked_str, read_str);
      }

      // Assert server open
      ASSERT_TRUE(SSL_write(client.get(), send_str.c_str(), send_str.length()));
      int read = SSL_read(server.get(), buf, buf_cap);
      ASSERT_TRUE(read);
      ASSERT_TRUE((size_t)read == send_str.length());
      std::string read_str(buf, read);
      ASSERT_EQ(send_str, read_str);

      // Assert server seal
      ASSERT_TRUE(SSL_write(server.get(), send_str.c_str(), send_str.length()));
      read = SSL_read(client.get(), buf, buf_cap);
      ASSERT_TRUE(read);
      ASSERT_TRUE((size_t)read == send_str.length());
      read_str = std::string(buf, read);
      ASSERT_EQ(send_str, read_str);

      if(i == read_write_per_instance - 1) {
        // Peek some client data before serialization
        ASSERT_TRUE(SSL_write(client.get(), send_str.c_str(), send_str.length()));
        read = SSL_peek(server.get(), buf, buf_cap);
        ASSERT_TRUE(read);
        ASSERT_TRUE((size_t)read == send_str.length());
        read_str = std::string(buf, read);
        ASSERT_EQ(send_str, read_str);
      }
    }
    if(version == TLS1_3_VERSION) {
      // Queue a Key Update to validate pending handshake flight messages are serialized
      SSL_key_update(server.get(), SSL_KEY_UPDATE_REQUESTED);
    }
    TransferSSL(&server, server_ctx.get(), &transfer_server);
    server = std::move(transfer_server);
  }
}

TEST(SSLTest, BIO) {
  bssl::UniquePtr<SSL_CTX> client_ctx(SSL_CTX_new(TLS_method()));
  bssl::UniquePtr<SSL_CTX> server_ctx(
      CreateContextWithTestCertificate(TLS_method()));
  ASSERT_TRUE(client_ctx);
  ASSERT_TRUE(server_ctx);

  for (bool take_ownership : {true, false}) {
    // For simplicity, get the handshake out of the way first.
    bssl::UniquePtr<SSL> client, server;
    ASSERT_TRUE(ConnectClientAndServer(&client, &server, client_ctx.get(),
                                       server_ctx.get()));

    // Wrap |client| in an SSL BIO.
    bssl::UniquePtr<BIO> client_bio(BIO_new(BIO_f_ssl()));
    ASSERT_TRUE(client_bio);
    ASSERT_EQ(1, BIO_set_ssl(client_bio.get(), client.get(), take_ownership));
    if (take_ownership) {
      client.release();
    }

    // Flushing the BIO should not crash.
    EXPECT_EQ(1, BIO_flush(client_bio.get()));

    // Exchange some data.
    EXPECT_EQ(5, BIO_write(client_bio.get(), "hello", 5));
    uint8_t buf[5];
    ASSERT_EQ(5, SSL_read(server.get(), buf, sizeof(buf)));
    EXPECT_EQ(Bytes("hello"), Bytes(buf));

    EXPECT_EQ(5, SSL_write(server.get(), "world", 5));
    ASSERT_EQ(5, BIO_read(client_bio.get(), buf, sizeof(buf)));
    EXPECT_EQ(Bytes("world"), Bytes(buf));

    // |BIO_should_read| should work.
    EXPECT_EQ(-1, BIO_read(client_bio.get(), buf, sizeof(buf)));
    EXPECT_TRUE(BIO_should_read(client_bio.get()));

    // Writing data should eventually exceed the buffer size and fail, reporting
    // |BIO_should_write|.
    int ret = 0;
    for (int i = 0; i < 1024; i++) {
      const uint8_t kZeros[1024] = {0};
      ret = BIO_write(client_bio.get(), kZeros, sizeof(kZeros));
      if (ret <= 0) {
        break;
      }
    }
    EXPECT_EQ(-1, ret);
    EXPECT_TRUE(BIO_should_write(client_bio.get()));
  }
}

TEST(SSLTest, BIO_2) {
  bssl::UniquePtr<SSL_CTX> client_ctx(SSL_CTX_new(TLS_method()));
  bssl::UniquePtr<SSL_CTX> server_ctx(
      CreateContextWithTestCertificate(TLS_method()));
  ASSERT_TRUE(client_ctx);
  ASSERT_TRUE(server_ctx);

  bssl::UniquePtr<BIO> server_bio(BIO_new_ssl(server_ctx.get(), 0));
  bssl::UniquePtr<BIO> client_bio(BIO_new_ssl_connect(client_ctx.get()));
  ASSERT_TRUE(server_bio);
  ASSERT_TRUE(client_bio);

  SSL *server_ssl_ptr = nullptr, *client_ssl_ptr = nullptr;
  ASSERT_TRUE(BIO_get_ssl(server_bio.get(), &server_ssl_ptr));
  ASSERT_TRUE(BIO_get_ssl(client_bio.get(), &client_ssl_ptr));
  ASSERT_TRUE(server_ssl_ptr);
  ASSERT_TRUE(client_ssl_ptr);

  // Client SSL BIOs typically establish connections to a host using
  // |BIO_do_connect|, which leverages the underlying connect |BIO| for socket
  // management. While OpenSSL provides |BIO_new_accept| and |BIO_s_accept| for
  // server-side socket setup, we haven't yet implemented this functionality.
  // For these tests, we opt for traditional SSL connection methods instead
  // until we have support for server-side socket management via |BIO|s.
  // Adding full socket management on the server side would exceed the scope of
  // testing |BIO_new_ssl(_connect)|, especially since we have dedicated tests
  // elsewhere that verify |BIO_do_connect|'s correctness.
  BIO *bio1 = nullptr, *bio2 = nullptr;
  ASSERT_TRUE(BIO_new_bio_pair(&bio1, 0, &bio2, 0));
  SSL_set_bio(client_ssl_ptr, bio1, bio1);
  SSL_set_bio(server_ssl_ptr, bio2, bio2);
  ASSERT_TRUE(CompleteHandshakes(client_ssl_ptr, server_ssl_ptr));
}

TEST(SSLTest, ALPNConfig) {
  bssl::UniquePtr<SSL_CTX> ctx(SSL_CTX_new(TLS_method()));
  ASSERT_TRUE(ctx);
  bssl::UniquePtr<X509> cert = GetTestCertificate();
  bssl::UniquePtr<EVP_PKEY> key = GetTestKey();
  ASSERT_TRUE(cert);
  ASSERT_TRUE(key);
  ASSERT_TRUE(SSL_CTX_use_certificate(ctx.get(), cert.get()));
  ASSERT_TRUE(SSL_CTX_use_PrivateKey(ctx.get(), key.get()));

  // Set up some machinery to check the configured ALPN against what is actually
  // sent over the wire. Note that the ALPN callback is only called when the
  // client offers ALPN.
  std::vector<uint8_t> observed_alpn;
  SSL_CTX_set_alpn_select_cb(
      ctx.get(),
      [](SSL *ssl, const uint8_t **out, uint8_t *out_len, const uint8_t *in,
         unsigned in_len, void *arg) -> int {
        std::vector<uint8_t> *observed_alpn_ptr =
            static_cast<std::vector<uint8_t> *>(arg);
        observed_alpn_ptr->assign(in, in + in_len);
        return SSL_TLSEXT_ERR_NOACK;
      },
      &observed_alpn);
  auto check_alpn_proto = [&](Span<const uint8_t> expected) {
    observed_alpn.clear();
    bssl::UniquePtr<SSL> client, server;
    ASSERT_TRUE(ConnectClientAndServer(&client, &server, ctx.get(), ctx.get()));
    EXPECT_EQ(Bytes(expected), Bytes(observed_alpn));
  };

  // Note that |SSL_CTX_set_alpn_protos|'s return value is reversed.
  static const uint8_t kValidList[] = {0x03, 'f', 'o', 'o',
                                       0x03, 'b', 'a', 'r'};
  EXPECT_EQ(0,
            SSL_CTX_set_alpn_protos(ctx.get(), kValidList, sizeof(kValidList)));
  check_alpn_proto(kValidList);

  // Invalid lists are rejected.
  static const uint8_t kInvalidList[] = {0x04, 'f', 'o', 'o'};
  EXPECT_EQ(1, SSL_CTX_set_alpn_protos(ctx.get(), kInvalidList,
                                       sizeof(kInvalidList)));

  // Empty lists are valid and are interpreted as disabling ALPN.
  EXPECT_EQ(0, SSL_CTX_set_alpn_protos(ctx.get(), nullptr, 0));
  check_alpn_proto({});
}

// Test that the key usage checker can correctly handle issuerUID and
// subjectUID. See https://crbug.com/1199744.
TEST(SSLTest, KeyUsageWithUIDs) {
  static const char kGoodKeyUsage[] = R"(
-----BEGIN CERTIFICATE-----
MIIB7DCCAZOgAwIBAgIJANlMBNpJfb/rMAoGCCqGSM49BAMCMEUxCzAJBgNVBAYT
AkFVMRMwEQYDVQQIDApTb21lLVN0YXRlMSEwHwYDVQQKDBhJbnRlcm5ldCBXaWRn
aXRzIFB0eSBMdGQwHhcNMTQwNDIzMjMyMTU3WhcNMTQwNTIzMjMyMTU3WjBFMQsw
CQYDVQQGEwJBVTETMBEGA1UECAwKU29tZS1TdGF0ZTEhMB8GA1UECgwYSW50ZXJu
ZXQgV2lkZ2l0cyBQdHkgTHRkMFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAE5itp
4r9ln5e+Lx4NlIpM1Zdrt6keDUb73ampHp3culoB59aXqAoY+cPEox5W4nyDSNsW
Ghz1HX7xlC1Lz3IiwYEEABI0VoIEABI0VqNgMF4wHQYDVR0OBBYEFKuE0qyrlfCC
ThZ4B1VXX+QmjYLRMB8GA1UdIwQYMBaAFKuE0qyrlfCCThZ4B1VXX+QmjYLRMA4G
A1UdDwEB/wQEAwIHgDAMBgNVHRMEBTADAQH/MAoGCCqGSM49BAMCA0cAMEQCIEWJ
34EcqW5MHwLIA1hZ2Tj/jV2QjN02KLxis9mFsqDKAiAMlMTkzsM51vVs9Ohqa+Rc
4Z7qDhjIhiF4dM0uEDYRVA==
-----END CERTIFICATE-----
)";
  static const char kBadKeyUsage[] = R"(
-----BEGIN CERTIFICATE-----
MIIB7jCCAZOgAwIBAgIJANlMBNpJfb/rMAoGCCqGSM49BAMCMEUxCzAJBgNVBAYT
AkFVMRMwEQYDVQQIDApTb21lLVN0YXRlMSEwHwYDVQQKDBhJbnRlcm5ldCBXaWRn
aXRzIFB0eSBMdGQwHhcNMTQwNDIzMjMyMTU3WhcNMTQwNTIzMjMyMTU3WjBFMQsw
CQYDVQQGEwJBVTETMBEGA1UECAwKU29tZS1TdGF0ZTEhMB8GA1UECgwYSW50ZXJu
ZXQgV2lkZ2l0cyBQdHkgTHRkMFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAE5itp
4r9ln5e+Lx4NlIpM1Zdrt6keDUb73ampHp3culoB59aXqAoY+cPEox5W4nyDSNsW
Ghz1HX7xlC1Lz3IiwYEEABI0VoIEABI0VqNgMF4wHQYDVR0OBBYEFKuE0qyrlfCC
ThZ4B1VXX+QmjYLRMB8GA1UdIwQYMBaAFKuE0qyrlfCCThZ4B1VXX+QmjYLRMA4G
A1UdDwEB/wQEAwIDCDAMBgNVHRMEBTADAQH/MAoGCCqGSM49BAMCA0kAMEYCIQC6
taYBUDu2gcZC6EMk79FBHArYI0ucF+kzvETegZCbBAIhANtObFec5gtso/47moPD
RHrQbWsFUakETXL9QMlegh5t
-----END CERTIFICATE-----
)";

  bssl::UniquePtr<X509> good = CertFromPEM(kGoodKeyUsage);
  ASSERT_TRUE(good);
  bssl::UniquePtr<X509> bad = CertFromPEM(kBadKeyUsage);
  ASSERT_TRUE(bad);

  // We check key usage when configuring EC certificates to distinguish ECDSA
  // and ECDH.
  bssl::UniquePtr<SSL_CTX> ctx(SSL_CTX_new(TLS_method()));
  ASSERT_TRUE(ctx);
  EXPECT_TRUE(SSL_CTX_use_certificate(ctx.get(), good.get()));
  EXPECT_FALSE(SSL_CTX_use_certificate(ctx.get(), bad.get()));
}


// Test that |SSL_ERROR_SYSCALL| continues to work after a close_notify.
TEST(SSLTest, ErrorSyscallAfterCloseNotify) {
  // Make a custom |BIO| where writes fail, but without pushing to the error
  // queue.
  bssl::UniquePtr<BIO_METHOD> method(BIO_meth_new(0, nullptr));
  ASSERT_TRUE(method);
  BIO_meth_set_create(method.get(), [](BIO *b) -> int {
    BIO_set_init(b, 1);
    return 1;
  });
  static bool write_failed = false;
  BIO_meth_set_write(method.get(), [](BIO *, const char *, int) -> int {
    // Fail the operation and don't add to the error queue.
    write_failed = true;
    return -1;
  });
  bssl::UniquePtr<BIO> wbio_silent_error(BIO_new(method.get()));
  ASSERT_TRUE(wbio_silent_error);

  bssl::UniquePtr<SSL_CTX> client_ctx(SSL_CTX_new(TLS_method()));
  bssl::UniquePtr<SSL_CTX> server_ctx =
      CreateContextWithTestCertificate(TLS_method());
  ASSERT_TRUE(client_ctx);
  ASSERT_TRUE(server_ctx);
  bssl::UniquePtr<SSL> client, server;
  ASSERT_TRUE(ConnectClientAndServer(&client, &server, client_ctx.get(),
                                     server_ctx.get()));

  // Replace the write |BIO| with |wbio_silent_error|.
  SSL_set0_wbio(client.get(), wbio_silent_error.release());

  // Writes should fail. There is nothing in the error queue, so
  // |SSL_ERROR_SYSCALL| indicates the caller needs to check out-of-band.
  const uint8_t data[1] = {0};
  int ret = SSL_write(client.get(), data, sizeof(data));
  EXPECT_EQ(ret, -1);
  EXPECT_EQ(SSL_get_error(client.get(), ret), SSL_ERROR_SYSCALL);
  EXPECT_TRUE(write_failed);
  write_failed = false;

  // Send a close_notify from the server. It should return 0 because
  // close_notify was sent, but not received. Confusingly, this is a success
  // output for |SSL_shutdown|'s API.
  EXPECT_EQ(SSL_shutdown(server.get()), 0);

  // Read the close_notify on the client.
  uint8_t buf[1];
  ret = SSL_read(client.get(), buf, sizeof(buf));
  EXPECT_EQ(ret, 0);
  EXPECT_EQ(SSL_get_error(client.get(), ret), SSL_ERROR_ZERO_RETURN);

  // Further calls to |SSL_read| continue to report |SSL_ERROR_ZERO_RETURN|.
  ret = SSL_read(client.get(), buf, sizeof(buf));
  EXPECT_EQ(ret, 0);
  EXPECT_EQ(SSL_get_error(client.get(), ret), SSL_ERROR_ZERO_RETURN);

  // Although the client has seen close_notify, it should continue to report
  // |SSL_ERROR_SYSCALL| when its writes fail.
  ret = SSL_write(client.get(), data, sizeof(data));
  EXPECT_EQ(ret, -1);
  EXPECT_EQ(SSL_get_error(client.get(), ret), SSL_ERROR_SYSCALL);
  EXPECT_TRUE(write_failed);
  write_failed = false;

  // Cause |BIO_write| to fail with a return value of zero instead.
  // |SSL_get_error| should not misinterpret this as a close_notify.
  //
  // This is not actually a correct implementation of |BIO_write|, but the rest
  // of the code treats zero from |BIO_write| as an error, so ensure it does so
  // correctly. Fixing https://crbug.com/boringssl/503 will make this case moot.
  BIO_meth_set_write(method.get(), [](BIO *, const char *, int) -> int {
    write_failed = true;
    return 0;
  });
  ret = SSL_write(client.get(), data, sizeof(data));
  EXPECT_EQ(ret, 0);
  EXPECT_EQ(SSL_get_error(client.get(), ret), SSL_ERROR_SYSCALL);
  EXPECT_TRUE(write_failed);
  write_failed = false;
}

}  // namespace
BSSL_NAMESPACE_END
