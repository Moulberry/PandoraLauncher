// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0 OR ISC

#include "ssl_common_test.h"
#include <openssl/ssl.h>

#include <gtest/gtest.h>
#include "../crypto/test/test_util.h"

BSSL_NAMESPACE_BEGIN

UniquePtr<SSL_SESSION> g_last_session;

bool GetClientHello(SSL *ssl, std::vector<uint8_t> *out) {
  bssl::UniquePtr<BIO> bio(BIO_new(BIO_s_mem()));
  if (!bio) {
    return false;
  }
  // Do not configure a reading BIO, but record what's written to a memory BIO.
  BIO_up_ref(bio.get());
  SSL_set_bio(ssl, nullptr /* rbio */, bio.get());
  int ret = SSL_connect(ssl);
  if (ret > 0) {
    // SSL_connect should fail without a BIO to write to.
    return false;
  }
  ERR_clear_error();

  const uint8_t *client_hello = nullptr;
  size_t client_hello_len = 0;
  if (!BIO_mem_contents(bio.get(), &client_hello, &client_hello_len)) {
    return false;
  }

  // We did not get far enough to write a ClientHello.
  if (client_hello_len == 0) {
    return false;
  }

  *out = std::vector<uint8_t>(client_hello, client_hello + client_hello_len);
  return true;
}


// These test certificates generated with the following Go program.
/* clang-format off
func main() {
  notBefore := time.Date(2000, time.January, 1, 0, 0, 0, 0, time.UTC)
  notAfter := time.Date(2099, time.January, 1, 0, 0, 0, 0, time.UTC)
  rootKey, _ := ecdsa.GenerateKey(elliptic.P256(), rand.Reader)
  rootTemplate := &x509.Certificate{
    SerialNumber:          big.NewInt(1),
    Subject:               pkix.Name{CommonName: "Test CA"},
    NotBefore:             notBefore,
    NotAfter:              notAfter,
    BasicConstraintsValid: true,
    IsCA:                  true,
  }
  rootDER, _ := x509.CreateCertificate(rand.Reader, rootTemplate, rootTemplate, &rootKey.PublicKey, rootKey)
  root, _ := x509.ParseCertificate(rootDER)
  pem.Encode(os.Stdout, &pem.Block{Type: "CERTIFICATE", Bytes: rootDER})
  leafKey, _ := ecdsa.GenerateKey(elliptic.P256(), rand.Reader)
  leafKeyDER, _ := x509.MarshalPKCS8PrivateKey(leafKey)
  pem.Encode(os.Stdout, &pem.Block{Type: "PRIVATE KEY", Bytes: leafKeyDER})
  for i, name := range []string{"public.example", "secret.example"} {
    leafTemplate := &x509.Certificate{
      SerialNumber:          big.NewInt(int64(i) + 2),
      Subject:               pkix.Name{CommonName: name},
      NotBefore:             notBefore,
      NotAfter:              notAfter,
      BasicConstraintsValid: true,
      DNSNames:              []string{name},
    }
    leafDER, _ := x509.CreateCertificate(rand.Reader, leafTemplate, root, &leafKey.PublicKey, rootKey)
    pem.Encode(os.Stdout, &pem.Block{Type: "CERTIFICATE", Bytes: leafDER})
  }
}
clang-format on */
UniquePtr<X509> GetLeafRoot() {
  bssl::UniquePtr<X509> root = CertFromPEM(R"(
-----BEGIN CERTIFICATE-----
MIIBRzCB7aADAgECAgEBMAoGCCqGSM49BAMCMBIxEDAOBgNVBAMTB1Rlc3QgQ0Ew
IBcNMDAwMTAxMDAwMDAwWhgPMjA5OTAxMDEwMDAwMDBaMBIxEDAOBgNVBAMTB1Rl
c3QgQ0EwWTATBgcqhkjOPQIBBggqhkjOPQMBBwNCAAT5JUjrI1DAxSpEl88UkmJw
tAJqxo/YrSFo9V3MkcNkfTixi5p6MUtO8DazhEgekBcd2+tBAWtl7dy0qpvTqx92
ozIwMDAPBgNVHRMBAf8EBTADAQH/MB0GA1UdDgQWBBTw6ftkexAI6o4r5FntJIfL
GU5F4zAKBggqhkjOPQQDAgNJADBGAiEAiiNowddQeHZaZFIygwe6RW5/WG4sUXWC
dkyl9CQzRaYCIQCFS1EvwZbZtMny27fYm1eeYciY0TkJTEi34H1KwyzzIA==
-----END CERTIFICATE-----
)");
  EXPECT_TRUE(root);
  return root;
}

UniquePtr<EVP_PKEY> GetLeafKey() {
  bssl::UniquePtr<EVP_PKEY> leaf_key = KeyFromPEM(R"(
-----BEGIN PRIVATE KEY-----
MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgj5WKHwHnziiyPauf
7QukxTwtTyGZkk8qNdms4puJfxqhRANCAARNrkhxabALDlJrHtvkuDwvCWUF/oVC
hr6PDITHi1lDlJzvVT4aXBH87sH2n2UV5zpx13NHkq1bIC8eRT8eOIe0
-----END PRIVATE KEY-----
)");
  EXPECT_TRUE(leaf_key);
  return leaf_key;
}

UniquePtr<X509> GetLeafPublic() {
  bssl::UniquePtr<X509> leaf_public = CertFromPEM(R"(
-----BEGIN CERTIFICATE-----
MIIBaDCCAQ6gAwIBAgIBAjAKBggqhkjOPQQDAjASMRAwDgYDVQQDEwdUZXN0IENB
MCAXDTAwMDEwMTAwMDAwMFoYDzIwOTkwMTAxMDAwMDAwWjAZMRcwFQYDVQQDEw5w
dWJsaWMuZXhhbXBsZTBZMBMGByqGSM49AgEGCCqGSM49AwEHA0IABE2uSHFpsAsO
Umse2+S4PC8JZQX+hUKGvo8MhMeLWUOUnO9VPhpcEfzuwfafZRXnOnHXc0eSrVsg
Lx5FPx44h7SjTDBKMAwGA1UdEwEB/wQCMAAwHwYDVR0jBBgwFoAU8On7ZHsQCOqO
K+RZ7SSHyxlOReMwGQYDVR0RBBIwEIIOcHVibGljLmV4YW1wbGUwCgYIKoZIzj0E
AwIDSAAwRQIhANqZRhDR/+QL05hsWXMYEwaiHifd9iakKoFEhKFchcF3AiBRAeXw
wRGGT6+iPmTYM6N5/IDyAb5B9Ke38O6lLEsUwA==
-----END CERTIFICATE-----
)");
  EXPECT_TRUE(leaf_public);
  return leaf_public;
}

UniquePtr<X509> GetLeafSecret() {
  bssl::UniquePtr<X509> leaf_secret = CertFromPEM(R"(
-----BEGIN CERTIFICATE-----
MIIBaTCCAQ6gAwIBAgIBAzAKBggqhkjOPQQDAjASMRAwDgYDVQQDEwdUZXN0IENB
MCAXDTAwMDEwMTAwMDAwMFoYDzIwOTkwMTAxMDAwMDAwWjAZMRcwFQYDVQQDEw5z
ZWNyZXQuZXhhbXBsZTBZMBMGByqGSM49AgEGCCqGSM49AwEHA0IABE2uSHFpsAsO
Umse2+S4PC8JZQX+hUKGvo8MhMeLWUOUnO9VPhpcEfzuwfafZRXnOnHXc0eSrVsg
Lx5FPx44h7SjTDBKMAwGA1UdEwEB/wQCMAAwHwYDVR0jBBgwFoAU8On7ZHsQCOqO
K+RZ7SSHyxlOReMwGQYDVR0RBBIwEIIOc2VjcmV0LmV4YW1wbGUwCgYIKoZIzj0E
AwIDSQAwRgIhAPQdIz1xCFkc9WuSkxOxJDpywZiEp9SnKcxJ9nwrlRp3AiEA+O3+
XRqE7XFhHL+7TNC2a9OOAjQsEF137YPWo+rhgko=
-----END CERTIFICATE-----
)");
  EXPECT_TRUE(leaf_secret);
  return leaf_secret;
}

UniquePtr<X509> GetED25519TestCertificate() {
  static const char kCertPEM[] =
      "-----BEGIN CERTIFICATE-----\n"
      "MIIBRDCB9wIUKI+32tShPulvafJa3xZvj29Z9xgwBQYDK2VwMEUxCzAJBgNVBAYT\n"
      "AkFVMRMwEQYDVQQIDApTb21lLVN0YXRlMSEwHwYDVQQKDBhJbnRlcm5ldCBXaWRn\n"
      "aXRzIFB0eSBMdGQwHhcNMjMwNzE4MTg0NzU4WhcNMjMwNzE5MTg0NzU4WjBFMQsw\n"
      "CQYDVQQGEwJBVTETMBEGA1UECAwKU29tZS1TdGF0ZTEhMB8GA1UECgwYSW50ZXJu\n"
      "ZXQgV2lkZ2l0cyBQdHkgTHRkMCowBQYDK2VwAyEAprAzqgxux8R4ZXaxn5mM/5E9\n"
      "0RNE59r47BJikdOoeUwwBQYDK2VwA0EAMELt0XRGFYo4qkWwOsoSYcdGYqlxVlf9\n"
      "AhTPaJ6SSzjv3n4r60wfe8Z2OPn415tcj2IIm42T64itI4OAX0aTCg==\n"
      "-----END CERTIFICATE-----\n";
  return CertFromPEM(kCertPEM);
}

UniquePtr<EVP_PKEY> GetED25519TestKey() {
  static const char kKeyPEM[] =
      "-----BEGIN PRIVATE KEY-----\n"
      "MC4CAQAwBQYDK2VwBCIEIGPkz4xAobc5gtRidkHl+fxNLHfiWo3efRG2G8Z617yk\n"
      "-----END PRIVATE KEY-----\n";
  return KeyFromPEM(kKeyPEM);
}

// Functions used by SSL encode/decode tests.
static void EncodeAndDecodeSSL(SSL *in, SSL_CTX *ctx,
                               bssl::UniquePtr<SSL> *out) {
  // Encoding SSL to bytes.
  size_t encoded_len = 0;
  bssl::UniquePtr<uint8_t> encoded;
  uint8_t *encoded_raw = nullptr;
  ASSERT_TRUE(SSL_to_bytes(in, &encoded_raw, &encoded_len));
  ASSERT_TRUE(encoded_len) << "SSL_to_bytes failed. Error code: "
                           << ERR_reason_error_string(ERR_get_error());
  encoded.reset(encoded_raw);
  // Decoding SSL from the bytes.
  const uint8_t *ptr2 = encoded.get();
  SSL *server2_ = SSL_from_bytes(ptr2, encoded_len, ctx);
  ASSERT_TRUE(server2_) << "SSL_from_bytes failed. Error code: "
                        << ERR_reason_error_string(ERR_get_error());
  out->reset(server2_);
}

static void TransferBIOs(bssl::UniquePtr<SSL> *from, SSL *to, bool free_from) {
  // Fetch the bio.
  BIO *rbio = SSL_get_rbio(from->get());
  ASSERT_TRUE(rbio) << "rbio is not set"
                    << ERR_reason_error_string(ERR_get_error());
  BIO *wbio = SSL_get_wbio(from->get());
  ASSERT_TRUE(wbio) << "wbio is not set"
                    << ERR_reason_error_string(ERR_get_error());
  // Move the bio.
  // Increase ref count of |rbio|.
  // |SSL_set_bio(to, rbio, wbio)| only increments the references of |rbio| by 1
  // when |rbio == wbio|. But |SSL_free| decreases the reference of |rbio| and
  // |wbio|.
  if (rbio == wbio) {
    BIO_up_ref(rbio);
  }
  SSL_set_bio(to, rbio, wbio);
  // TODO: test half read and write hold by SSL.
  // TODO: add a test to check error code?
  // e.g. ASSERT_EQ(SSL_get_error(server1_, 0), SSL_ERROR_ZERO_RETURN);
  if(free_from) {
    SSL_free(from->release());
  }
}

// TransferSSL performs SSL transfer by
// 1. Encode the SSL of |in| into bytes.
// 2. Decode the bytes into a new SSL.
// 3. Free the SSL of |in|.
// 4. If |out| is not nullptr, |out| will hold the decoded SSL.
//    Else, |in| will get reset to hold the decoded SSL.
void TransferSSL(UniquePtr<SSL> *in, SSL_CTX *in_ctx,
                 bssl::UniquePtr<SSL> *out, bool free_in) {
  bssl::UniquePtr<SSL> decoded_ssl;
  EncodeAndDecodeSSL(in->get(), in_ctx, &decoded_ssl);
  if (!decoded_ssl) {
    return;
  }
  // Transfer the bio.
  TransferBIOs(in, decoded_ssl.get(), free_in);
  if (out == nullptr) {
    in->reset(decoded_ssl.release());
  } else {
    out->reset(decoded_ssl.release());
  }
}

bool ConnectClientAndServer(UniquePtr<SSL> *out_client,
                            UniquePtr<SSL> *out_server, SSL_CTX *client_ctx,
                            SSL_CTX *server_ctx, const ClientConfig &config,
                            bool shed_handshake_config) {
  bssl::UniquePtr<SSL> client, server;
  if (!CreateClientAndServer(&client, &server, client_ctx, server_ctx)) {
    return false;
  }
  if (config.early_data) {
    SSL_set_early_data_enabled(client.get(), 1);
  }
  if (config.session) {
    SSL_set_session(client.get(), config.session);
  }
  if (!config.servername.empty() &&
      !SSL_set_tlsext_host_name(client.get(), config.servername.c_str())) {
    return false;
  }
  if (!config.verify_hostname.empty()) {
    if (!SSL_set1_host(client.get(), config.verify_hostname.c_str())) {
      return false;
    }
    SSL_set_hostflags(client.get(), config.hostflags);
  }

  SSL_set_shed_handshake_config(client.get(), shed_handshake_config);
  SSL_set_shed_handshake_config(server.get(), shed_handshake_config);

  if (!CompleteHandshakes(client.get(), server.get())) {
    return false;
  }

  *out_client = std::move(client);
  *out_server = std::move(server);
  return true;
}

void ExpectSessionReused(SSL_CTX *client_ctx, SSL_CTX *server_ctx,
                         SSL_SESSION *session, bool want_reused) {
  UniquePtr<SSL> client, server;
  ClientConfig config;
  config.session = session;
  ASSERT_TRUE(
      ConnectClientAndServer(&client, &server, client_ctx, server_ctx, config));

  EXPECT_EQ(SSL_session_reused(client.get()), SSL_session_reused(server.get()));

  bool was_reused = !!SSL_session_reused(client.get());
  EXPECT_EQ(was_reused, want_reused);
}

UniquePtr<SSL_SESSION> CreateClientSession(SSL_CTX *client_ctx,
                                           SSL_CTX *server_ctx,
                                           const ClientConfig &config) {
  g_last_session = nullptr;
  SSL_CTX_sess_set_new_cb(client_ctx, SaveLastSession);

  // Connect client and server to get a session.
  bssl::UniquePtr<SSL> client, server;
  if (!ConnectClientAndServer(&client, &server, client_ctx, server_ctx,
                              config) ||
      !FlushNewSessionTickets(client.get(), server.get())) {
    fprintf(stderr, "Failed to connect client and server.\n");
    return nullptr;
  }

  SSL_CTX_sess_set_new_cb(client_ctx, nullptr);

  if (!g_last_session) {
    fprintf(stderr, "Client did not receive a session.\n");
    return nullptr;
  }
  return std::move(g_last_session);
}

bool FlushNewSessionTickets(SSL *client, SSL *server) {
  // NewSessionTickets are deferred on the server to |SSL_write|, and clients do
  // not pick them up until |SSL_read|.
  for (;;) {
    int server_ret = SSL_write(server, nullptr, 0);
    int server_err = SSL_get_error(server, server_ret);
    // The server may either succeed (|server_ret| is zero) or block on write
    // (|server_ret| is -1 and |server_err| is |SSL_ERROR_WANT_WRITE|).
    if (server_ret > 0 ||
        (server_ret < 0 && server_err != SSL_ERROR_WANT_WRITE)) {
      fprintf(stderr, "Unexpected server result: %d %d\n", server_ret,
              server_err);
      return false;
    }

    int client_ret = SSL_read(client, nullptr, 0);
    int client_err = SSL_get_error(client, client_ret);
    // The client must always block on read.
    if (client_ret != -1 || client_err != SSL_ERROR_WANT_READ) {
      fprintf(stderr, "Unexpected client result: %d %d\n", client_ret,
              client_err);
      return false;
    }

    // The server flushed everything it had to write.
    if (server_ret == 0) {
      return true;
    }
  }
}

uint16_t EpochFromSequence(uint64_t seq) {
  return static_cast<uint16_t>(seq >> 48);
}

int SaveLastSession(SSL *ssl, SSL_SESSION *session) {
  // Save the most recent session.
  g_last_session.reset(session);
  return 1;
}

UniquePtr<SSL_CTX> CreateContextWithTestCertificate(const SSL_METHOD *method) {
  bssl::UniquePtr<SSL_CTX> ctx(SSL_CTX_new(method));
  bssl::UniquePtr<X509> cert = GetTestCertificate();
  bssl::UniquePtr<EVP_PKEY> key = GetTestKey();
  if (!ctx || !cert || !key ||
      !SSL_CTX_use_certificate(ctx.get(), cert.get()) ||
      !SSL_CTX_use_PrivateKey(ctx.get(), key.get())) {
    return nullptr;
  }
  return ctx;
}

void SetUpExpectedNewCodePoint(SSL_CTX *ctx) {
  SSL_CTX_set_select_certificate_cb(
      ctx,
      [](const SSL_CLIENT_HELLO *client_hello) -> ssl_select_cert_result_t {
        const uint8_t *data = nullptr;
        size_t len = 0;
        if (!SSL_early_callback_ctx_extension_get(
                client_hello, TLSEXT_TYPE_application_settings, &data, &len)) {
          ADD_FAILURE() << "Could not find alps new codepoint.";
          return ssl_select_cert_error;
        }
        return ssl_select_cert_success;
      });
}

UniquePtr<X509> GetTestCertificate() {
  static const char kCertPEM[] =
      "-----BEGIN CERTIFICATE-----\n"
      "MIICWDCCAcGgAwIBAgIJAPuwTC6rEJsMMA0GCSqGSIb3DQEBBQUAMEUxCzAJBgNV\n"
      "BAYTAkFVMRMwEQYDVQQIDApTb21lLVN0YXRlMSEwHwYDVQQKDBhJbnRlcm5ldCBX\n"
      "aWRnaXRzIFB0eSBMdGQwHhcNMTQwNDIzMjA1MDQwWhcNMTcwNDIyMjA1MDQwWjBF\n"
      "MQswCQYDVQQGEwJBVTETMBEGA1UECAwKU29tZS1TdGF0ZTEhMB8GA1UECgwYSW50\n"
      "ZXJuZXQgV2lkZ2l0cyBQdHkgTHRkMIGfMA0GCSqGSIb3DQEBAQUAA4GNADCBiQKB\n"
      "gQDYK8imMuRi/03z0K1Zi0WnvfFHvwlYeyK9Na6XJYaUoIDAtB92kWdGMdAQhLci\n"
      "HnAjkXLI6W15OoV3gA/ElRZ1xUpxTMhjP6PyY5wqT5r6y8FxbiiFKKAnHmUcrgfV\n"
      "W28tQ+0rkLGMryRtrukXOgXBv7gcrmU7G1jC2a7WqmeI8QIDAQABo1AwTjAdBgNV\n"
      "HQ4EFgQUi3XVrMsIvg4fZbf6Vr5sp3Xaha8wHwYDVR0jBBgwFoAUi3XVrMsIvg4f\n"
      "Zbf6Vr5sp3Xaha8wDAYDVR0TBAUwAwEB/zANBgkqhkiG9w0BAQUFAAOBgQA76Hht\n"
      "ldY9avcTGSwbwoiuIqv0jTL1fHFnzy3RHMLDh+Lpvolc5DSrSJHCP5WuK0eeJXhr\n"
      "T5oQpHL9z/cCDLAKCKRa4uV0fhEdOWBqyR9p8y5jJtye72t6CuFUV5iqcpF4BH4f\n"
      "j2VNHwsSrJwkD4QUGlUtH7vwnQmyCFxZMmWAJg==\n"
      "-----END CERTIFICATE-----\n";
  return CertFromPEM(kCertPEM);
}

UniquePtr<EVP_PKEY> GetTestKey() {
  static const char kKeyPEM[] =
      "-----BEGIN RSA PRIVATE KEY-----\n"
      "MIICXgIBAAKBgQDYK8imMuRi/03z0K1Zi0WnvfFHvwlYeyK9Na6XJYaUoIDAtB92\n"
      "kWdGMdAQhLciHnAjkXLI6W15OoV3gA/ElRZ1xUpxTMhjP6PyY5wqT5r6y8FxbiiF\n"
      "KKAnHmUcrgfVW28tQ+0rkLGMryRtrukXOgXBv7gcrmU7G1jC2a7WqmeI8QIDAQAB\n"
      "AoGBAIBy09Fd4DOq/Ijp8HeKuCMKTHqTW1xGHshLQ6jwVV2vWZIn9aIgmDsvkjCe\n"
      "i6ssZvnbjVcwzSoByhjN8ZCf/i15HECWDFFh6gt0P5z0MnChwzZmvatV/FXCT0j+\n"
      "WmGNB/gkehKjGXLLcjTb6dRYVJSCZhVuOLLcbWIV10gggJQBAkEA8S8sGe4ezyyZ\n"
      "m4e9r95g6s43kPqtj5rewTsUxt+2n4eVodD+ZUlCULWVNAFLkYRTBCASlSrm9Xhj\n"
      "QpmWAHJUkQJBAOVzQdFUaewLtdOJoPCtpYoY1zd22eae8TQEmpGOR11L6kbxLQsk\n"
      "aMly/DOnOaa82tqAGTdqDEZgSNmCeKKknmECQAvpnY8GUOVAubGR6c+W90iBuQLj\n"
      "LtFp/9ihd2w/PoDwrHZaoUYVcT4VSfJQog/k7kjE4MYXYWL8eEKg3WTWQNECQQDk\n"
      "104Wi91Umd1PzF0ijd2jXOERJU1wEKe6XLkYYNHWQAe5l4J4MWj9OdxFXAxIuuR/\n"
      "tfDwbqkta4xcux67//khAkEAvvRXLHTaa6VFzTaiiO8SaFsHV3lQyXOtMrBpB5jd\n"
      "moZWgjHvB2W9Ckn7sDqsPB+U2tyX0joDdQEyuiMECDY8oQ==\n"
      "-----END RSA PRIVATE KEY-----\n";
  return KeyFromPEM(kKeyPEM);
}

static bssl::UniquePtr<CRYPTO_BUFFER> BufferFromPEM(const char *pem) {
  bssl::UniquePtr<BIO> bio(BIO_new_mem_buf(pem, strlen(pem)));
  char *name = nullptr, *header = nullptr;
  uint8_t *data = nullptr;
  long data_len = 0;
  if (!PEM_read_bio(bio.get(), &name, &header, &data, &data_len)) {
    return nullptr;
  }
  OPENSSL_free(name);
  OPENSSL_free(header);

  auto ret = bssl::UniquePtr<CRYPTO_BUFFER>(
      CRYPTO_BUFFER_new(data, data_len, nullptr));
  OPENSSL_free(data);
  return ret;
}



UniquePtr<CRYPTO_BUFFER> GetChainTestCertificateBuffer() {
  static const char kCertPEM[] =
      "-----BEGIN CERTIFICATE-----\n"
      "MIIC0jCCAbqgAwIBAgICEAAwDQYJKoZIhvcNAQELBQAwDzENMAsGA1UEAwwEQiBD\n"
      "QTAeFw0xNjAyMjgyMDI3MDNaFw0yNjAyMjUyMDI3MDNaMBgxFjAUBgNVBAMMDUNs\n"
      "aWVudCBDZXJ0IEEwggEiMA0GCSqGSIb3DQEBAQUAA4IBDwAwggEKAoIBAQDRvaz8\n"
      "CC/cshpCafJo4jLkHEoBqDLhdgFelJoAiQUyIqyWl2O7YHPnpJH+TgR7oelzNzt/\n"
      "kLRcH89M/TszB6zqyLTC4aqmvzKL0peD/jL2LWBucR0WXIvjA3zoRuF/x86+rYH3\n"
      "tHb+xs2PSs8EGL/Ev+ss+qTzTGEn26fuGNHkNw6tOwPpc+o8+wUtzf/kAthamo+c\n"
      "IDs2rQ+lP7+aLZTLeU/q4gcLutlzcK5imex5xy2jPkweq48kijK0kIzl1cPlA5d1\n"
      "z7C8jU50Pj9X9sQDJTN32j7UYRisJeeYQF8GaaN8SbrDI6zHgKzrRLyxDt/KQa9V\n"
      "iLeXANgZi+Xx9KgfAgMBAAGjLzAtMAwGA1UdEwEB/wQCMAAwHQYDVR0lBBYwFAYI\n"
      "KwYBBQUHAwEGCCsGAQUFBwMCMA0GCSqGSIb3DQEBCwUAA4IBAQBFEVbmYl+2RtNw\n"
      "rDftRDF1v2QUbcN2ouSnQDHxeDQdSgasLzT3ui8iYu0Rw2WWcZ0DV5e0ztGPhWq7\n"
      "AO0B120aFRMOY+4+bzu9Q2FFkQqc7/fKTvTDzIJI5wrMnFvUfzzvxh3OHWMYSs/w\n"
      "giq33hTKeHEq6Jyk3btCny0Ycecyc3yGXH10sizUfiHlhviCkDuESk8mFDwDDzqW\n"
      "ZF0IipzFbEDHoIxLlm3GQxpiLoEV4k8KYJp3R5KBLFyxM6UGPz8h72mIPCJp2RuK\n"
      "MYgF91UDvVzvnYm6TfseM2+ewKirC00GOrZ7rEcFvtxnKSqYf4ckqfNdSU1Y+RRC\n"
      "1ngWZ7Ih\n"
      "-----END CERTIFICATE-----\n";
  return BufferFromPEM(kCertPEM);
}

UniquePtr<EVP_PKEY> GetChainTestKey() {
  static const char kKeyPEM[] =
      "-----BEGIN PRIVATE KEY-----\n"
      "MIIEvgIBADANBgkqhkiG9w0BAQEFAASCBKgwggSkAgEAAoIBAQDRvaz8CC/cshpC\n"
      "afJo4jLkHEoBqDLhdgFelJoAiQUyIqyWl2O7YHPnpJH+TgR7oelzNzt/kLRcH89M\n"
      "/TszB6zqyLTC4aqmvzKL0peD/jL2LWBucR0WXIvjA3zoRuF/x86+rYH3tHb+xs2P\n"
      "Ss8EGL/Ev+ss+qTzTGEn26fuGNHkNw6tOwPpc+o8+wUtzf/kAthamo+cIDs2rQ+l\n"
      "P7+aLZTLeU/q4gcLutlzcK5imex5xy2jPkweq48kijK0kIzl1cPlA5d1z7C8jU50\n"
      "Pj9X9sQDJTN32j7UYRisJeeYQF8GaaN8SbrDI6zHgKzrRLyxDt/KQa9ViLeXANgZ\n"
      "i+Xx9KgfAgMBAAECggEBAK0VjSJzkyPaamcyTVSWjo7GdaBGcK60lk657RjR+lK0\n"
      "YJ7pkej4oM2hdsVZFsP8Cs4E33nXLa/0pDsRov/qrp0WQm2skwqGMC1I/bZ0WRPk\n"
      "wHaDrBBfESWnJDX/AGpVtlyOjPmgmK6J2usMPihQUDkKdAYrVWJePrMIxt1q6BMe\n"
      "iczs3qriMmtY3bUc4UyUwJ5fhDLjshHvfuIpYQyI6EXZM6dZksn9LylXJnigY6QJ\n"
      "HxOYO0BDwOsZ8yQ8J8afLk88i0GizEkgE1z3REtQUwgWfxr1WV/ud+T6/ZhSAgH9\n"
      "042mQvSFZnIUSEsmCvjhWuAunfxHKCTcAoYISWfzWpkCgYEA7gpf3HHU5Tn+CgUn\n"
      "1X5uGpG3DmcMgfeGgs2r2f/IIg/5Ac1dfYILiybL1tN9zbyLCJfcbFpWBc9hJL6f\n"
      "CPc5hUiwWFJqBJewxQkC1Ae/HakHbip+IZ+Jr0842O4BAArvixk4Lb7/N2Ct9sTE\n"
      "NJO6RtK9lbEZ5uK61DglHy8CS2UCgYEA4ZC1o36kPAMQBggajgnucb2yuUEelk0f\n"
      "AEr+GI32MGE+93xMr7rAhBoqLg4AITyIfEnOSQ5HwagnIHonBbv1LV/Gf9ursx8Z\n"
      "YOGbvT8zzzC+SU1bkDzdjAYnFQVGIjMtKOBJ3K07++ypwX1fr4QsQ8uKL8WSOWwt\n"
      "Z3Bym6XiZzMCgYADnhy+2OwHX85AkLt+PyGlPbmuelpyTzS4IDAQbBa6jcuW/2wA\n"
      "UE2km75VUXmD+u2R/9zVuLm99NzhFhSMqlUxdV1YukfqMfP5yp1EY6m/5aW7QuIP\n"
      "2MDa7TVL9rIFMiVZ09RKvbBbQxjhuzPQKL6X/PPspnhiTefQ+dl2k9xREQKBgHDS\n"
      "fMfGNEeAEKezrfSVqxphE9/tXms3L+ZpnCaT+yu/uEr5dTIAawKoQ6i9f/sf1/Sy\n"
      "xedsqR+IB+oKrzIDDWMgoJybN4pkZ8E5lzhVQIjFjKgFdWLzzqyW9z1gYfABQPlN\n"
      "FiS20WX0vgP1vcKAjdNrHzc9zyHBpgQzDmAj3NZZAoGBAI8vKCKdH7w3aL5CNkZQ\n"
      "2buIeWNA2HZazVwAGG5F2TU/LmXfRKnG6dX5bkU+AkBZh56jNZy//hfFSewJB4Kk\n"
      "buB7ERSdaNbO21zXt9FEA3+z0RfMd/Zv2vlIWOSB5nzl/7UKti3sribK6s9ZVLfi\n"
      "SxpiPQ8d/hmSGwn4ksrWUsJD\n"
      "-----END PRIVATE KEY-----\n";
  return KeyFromPEM(kKeyPEM);
}

UniquePtr<X509> GetChainTestIntermediate() {
  return X509FromBuffer(GetChainTestIntermediateBuffer());
}

UniquePtr<X509> GetChainTestCertificate() {
  return X509FromBuffer(GetChainTestCertificateBuffer());
}

UniquePtr<X509> X509FromBuffer(UniquePtr<CRYPTO_BUFFER> buffer) {
  if (!buffer) {
    return nullptr;
  }
  const uint8_t *derp = CRYPTO_BUFFER_data(buffer.get());
  return bssl::UniquePtr<X509>(
      d2i_X509(NULL, &derp, CRYPTO_BUFFER_len(buffer.get())));
}

UniquePtr<CRYPTO_BUFFER> GetChainTestIntermediateBuffer() {
  static const char kCertPEM[] =
      "-----BEGIN CERTIFICATE-----\n"
      "MIICwjCCAaqgAwIBAgICEAEwDQYJKoZIhvcNAQELBQAwFDESMBAGA1UEAwwJQyBS\n"
      "b290IENBMB4XDTE2MDIyODIwMjcwM1oXDTI2MDIyNTIwMjcwM1owDzENMAsGA1UE\n"
      "AwwEQiBDQTCCASIwDQYJKoZIhvcNAQEBBQADggEPADCCAQoCggEBALsSCYmDip2D\n"
      "GkjFxw7ykz26JSjELkl6ArlYjFJ3aT/SCh8qbS4gln7RH8CPBd78oFdfhIKQrwtZ\n"
      "3/q21ykD9BAS3qHe2YdcJfm8/kWAy5DvXk6NXU4qX334KofBAEpgdA/igEFq1P1l\n"
      "HAuIfZCpMRfT+i5WohVsGi8f/NgpRvVaMONLNfgw57mz1lbtFeBEISmX0kbsuJxF\n"
      "Qj/Bwhi5/0HAEXG8e7zN4cEx0yPRvmOATRdVb/8dW2pwOHRJq9R5M0NUkIsTSnL7\n"
      "6N/z8hRAHMsV3IudC5Yd7GXW1AGu9a+iKU+Q4xcZCoj0DC99tL4VKujrV1kAeqsM\n"
      "cz5/dKzi6+cCAwEAAaMjMCEwDwYDVR0TAQH/BAUwAwEB/zAOBgNVHQ8BAf8EBAMC\n"
      "AQYwDQYJKoZIhvcNAQELBQADggEBAIIeZiEeNhWWQ8Y4D+AGDwqUUeG8NjCbKrXQ\n"
      "BlHg5wZ8xftFaiP1Dp/UAezmx2LNazdmuwrYB8lm3FVTyaPDTKEGIPS4wJKHgqH1\n"
      "QPDhqNm85ey7TEtI9oYjsNim/Rb+iGkIAMXaxt58SzxbjvP0kMr1JfJIZbic9vye\n"
      "NwIspMFIpP3FB8ywyu0T0hWtCQgL4J47nigCHpOu58deP88fS/Nyz/fyGVWOZ76b\n"
      "WhWwgM3P3X95fQ3d7oFPR/bVh0YV+Cf861INwplokXgXQ3/TCQ+HNXeAMWn3JLWv\n"
      "XFwk8owk9dq/kQGdndGgy3KTEW4ctPX5GNhf3LJ9Q7dLji4ReQ4=\n"
      "-----END CERTIFICATE-----\n";
  return BufferFromPEM(kCertPEM);
}

UniquePtr<X509> GetECDSATestCertificate() {
  static const char kCertPEM[] =
      "-----BEGIN CERTIFICATE-----\n"
      "MIIBzzCCAXagAwIBAgIJANlMBNpJfb/rMAkGByqGSM49BAEwRTELMAkGA1UEBhMC\n"
      "QVUxEzARBgNVBAgMClNvbWUtU3RhdGUxITAfBgNVBAoMGEludGVybmV0IFdpZGdp\n"
      "dHMgUHR5IEx0ZDAeFw0xNDA0MjMyMzIxNTdaFw0xNDA1MjMyMzIxNTdaMEUxCzAJ\n"
      "BgNVBAYTAkFVMRMwEQYDVQQIDApTb21lLVN0YXRlMSEwHwYDVQQKDBhJbnRlcm5l\n"
      "dCBXaWRnaXRzIFB0eSBMdGQwWTATBgcqhkjOPQIBBggqhkjOPQMBBwNCAATmK2ni\n"
      "v2Wfl74vHg2UikzVl2u3qR4NRvvdqakendy6WgHn1peoChj5w8SjHlbifINI2xYa\n"
      "HPUdfvGULUvPciLBo1AwTjAdBgNVHQ4EFgQUq4TSrKuV8IJOFngHVVdf5CaNgtEw\n"
      "HwYDVR0jBBgwFoAUq4TSrKuV8IJOFngHVVdf5CaNgtEwDAYDVR0TBAUwAwEB/zAJ\n"
      "BgcqhkjOPQQBA0gAMEUCIQDyoDVeUTo2w4J5m+4nUIWOcAZ0lVfSKXQA9L4Vh13E\n"
      "BwIgfB55FGohg/B6dGh5XxSZmmi08cueFV7mHzJSYV51yRQ=\n"
      "-----END CERTIFICATE-----\n";
  return CertFromPEM(kCertPEM);
}

UniquePtr<EVP_PKEY> GetECDSATestKey() {
  static const char kKeyPEM[] =
      "-----BEGIN PRIVATE KEY-----\n"
      "MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgBw8IcnrUoEqc3VnJ\n"
      "TYlodwi1b8ldMHcO6NHJzgqLtGqhRANCAATmK2niv2Wfl74vHg2UikzVl2u3qR4N\n"
      "Rvvdqakendy6WgHn1peoChj5w8SjHlbifINI2xYaHPUdfvGULUvPciLB\n"
      "-----END PRIVATE KEY-----\n";
  return KeyFromPEM(kKeyPEM);
}

bool ExpectSingleError(int lib, int reason) {
  const char *expected = ERR_reason_error_string(ERR_PACK(lib, reason));
  int err = ERR_get_error();
  if (ERR_GET_LIB(err) != lib || ERR_GET_REASON(err) != reason) {
    char buf[ERR_ERROR_STRING_BUF_LEN];
    ERR_error_string_n(err, buf, sizeof(buf));
    fprintf(stderr, "Wanted %s, got: %s.\n", expected, buf);
    return false;
  }

  if (ERR_peek_error() != 0) {
    fprintf(stderr, "Unexpected error following %s.\n", expected);
    return false;
  }

  return true;
}

const char *GetVersionName(uint16_t version) {
  switch (version) {
    case TLS1_VERSION:
      return "TLSv1";
    case TLS1_1_VERSION:
      return "TLSv1.1";
    case TLS1_2_VERSION:
      return "TLSv1.2";
    case TLS1_3_VERSION:
      return "TLSv1.3";
    case DTLS1_VERSION:
      return "DTLSv1";
    case DTLS1_2_VERSION:
      return "DTLSv1.2";
    default:
      return "???";
  }
}

void FrozenTimeCallback(const SSL *ssl, timeval *out_clock) {
  out_clock->tv_sec = 1000;
  out_clock->tv_usec = 0;
}

bool ChainsEqual(STACK_OF(X509) *chain, const std::vector<X509 *> &expected) {
  if (sk_X509_num(chain) != expected.size()) {
    return false;
  }

  for (size_t i = 0; i < expected.size(); i++) {
    if (X509_cmp(sk_X509_value(chain, i), expected[i]) != 0) {
      return false;
    }
  }

  return true;
}

UniquePtr<EVP_PKEY> KeyFromPEM(const char *pem) {
  UniquePtr<BIO> bio(BIO_new_mem_buf(pem, strlen(pem)));
  if (!bio) {
    return nullptr;
  }
  return bssl::UniquePtr<EVP_PKEY>(
      PEM_read_bio_PrivateKey(bio.get(), nullptr, nullptr, nullptr));
}

// CreateClientAndServer creates a client and server |SSL| objects whose |BIO|s
// are paired with each other. It does not run the handshake. The caller is
// expected to configure the objects and drive the handshake as needed.
bool CreateClientAndServer(bssl::UniquePtr<SSL> *out_client,
                           bssl::UniquePtr<SSL> *out_server,
                           SSL_CTX *client_ctx, SSL_CTX *server_ctx) {
  UniquePtr<SSL> client(SSL_new(client_ctx)), server(SSL_new(server_ctx));
  if (!client || !server) {
    return false;
  }
  SSL_set_connect_state(client.get());
  SSL_set_accept_state(server.get());

  BIO *bio1 = nullptr, *bio2 = nullptr;
  if (!BIO_new_bio_pair(&bio1, 0, &bio2, 0)) {
    return false;
  }
  // SSL_set_bio takes ownership.
  SSL_set_bio(client.get(), bio1, bio1);
  SSL_set_bio(server.get(), bio2, bio2);

  *out_client = std::move(client);
  *out_server = std::move(server);
  return true;
}

bool CompleteHandshakes(SSL *client, SSL *server) {
  // Drive both their handshakes to completion.
  for (;;) {
    int client_ret = SSL_do_handshake(client);
    int client_err = SSL_get_error(client, client_ret);
    if (client_err != SSL_ERROR_NONE && client_err != SSL_ERROR_WANT_READ &&
        client_err != SSL_ERROR_WANT_WRITE &&
        client_err != SSL_ERROR_PENDING_TICKET) {
      fprintf(stderr, "Client error: %s\n", SSL_error_description(client_err));
      return false;
    }

    int server_ret = SSL_do_handshake(server);
    int server_err = SSL_get_error(server, server_ret);
    if (server_err != SSL_ERROR_NONE && server_err != SSL_ERROR_WANT_READ &&
        server_err != SSL_ERROR_WANT_WRITE &&
        server_err != SSL_ERROR_PENDING_TICKET) {
      fprintf(stderr, "Server error: %s\n", SSL_error_description(server_err));
      return false;
    }

    if (client_ret == 1 && server_ret == 1) {
      break;
    }
  }

  return true;
}

void SetUpExpectedOldCodePoint(SSL_CTX *ctx) {
  SSL_CTX_set_select_certificate_cb(
      ctx,
      [](const SSL_CLIENT_HELLO *client_hello) -> ssl_select_cert_result_t {
        const uint8_t *data = nullptr;
        size_t len = 0;
        if (!SSL_early_callback_ctx_extension_get(
                client_hello, TLSEXT_TYPE_application_settings_old, &data,
                &len)) {
          ADD_FAILURE() << "Could not find alps old codepoint.";
          return ssl_select_cert_error;
        }
        return ssl_select_cert_success;
      });
}

BSSL_NAMESPACE_END
