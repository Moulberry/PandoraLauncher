/* Copyright (C) 1995-1998 Eric Young (eay@cryptsoft.com)
 * All rights reserved.
 *
 * This package is an SSL implementation written
 * by Eric Young (eay@cryptsoft.com).
 * The implementation was written so as to conform with Netscapes SSL.
 *
 * This library is free for commercial and non-commercial use as long as
 * the following conditions are aheared to.  The following conditions
 * apply to all code found in this distribution, be it the RC4, RSA,
 * lhash, DES, etc., code; not just the SSL code.  The SSL documentation
 * included with this distribution is covered by the same copyright terms
 * except that the holder is Tim Hudson (tjh@cryptsoft.com).
 *
 * Copyright remains Eric Young's, and as such any Copyright notices in
 * the code are not to be removed.
 * If this package is used in a product, Eric Young should be given attribution
 * as the author of the parts of the library used.
 * This can be in the form of a textual message at program startup or
 * in documentation (online or textual) provided with the package.
 *
 * Redistribution and use in source and binary forms, with or without
 * modification, are permitted provided that the following conditions
 * are met:
 * 1. Redistributions of source code must retain the copyright
 *    notice, this list of conditions and the following disclaimer.
 * 2. Redistributions in binary form must reproduce the above copyright
 *    notice, this list of conditions and the following disclaimer in the
 *    documentation and/or other materials provided with the distribution.
 * 3. All advertising materials mentioning features or use of this software
 *    must display the following acknowledgement:
 *    "This product includes cryptographic software written by
 *     Eric Young (eay@cryptsoft.com)"
 *    The word 'cryptographic' can be left out if the rouines from the library
 *    being used are not cryptographic related :-).
 * 4. If you include any Windows specific code (or a derivative thereof) from
 *    the apps directory (application code) you must include an acknowledgement:
 *    "This product includes software written by Tim Hudson (tjh@cryptsoft.com)"
 *
 * THIS SOFTWARE IS PROVIDED BY ERIC YOUNG ``AS IS'' AND
 * ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
 * IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE
 * ARE DISCLAIMED.  IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS BE LIABLE
 * FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
 * DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS
 * OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION)
 * HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT
 * LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY
 * OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF
 * SUCH DAMAGE.
 *
 * The licence and distribution terms for any publically available version or
 * derivative of this code cannot be changed.  i.e. this code cannot simply be
 * copied and put under another distribution licence
 * [including the GNU Public Licence.] */

#ifndef OPENSSL_HEADER_BIO_INTERNAL_H
#define OPENSSL_HEADER_BIO_INTERNAL_H

#include <openssl/bio.h>

#if !defined(OPENSSL_NO_SOCK)
#if !defined(OPENSSL_WINDOWS)
#if defined(OPENSSL_PNACL)
// newlib uses u_short in socket.h without defining it.
typedef unsigned short u_short;
#endif
#include <sys/types.h>
#include <sys/socket.h>
#include <netinet/in.h>
#include <sys/un.h>
#include <unistd.h>
#else
OPENSSL_MSVC_PRAGMA(warning(push, 3))
#include <winsock2.h>
#include <ws2ipdef.h>
OPENSSL_MSVC_PRAGMA(warning(pop))
typedef int socklen_t;
#if !defined(_SSIZE_T_DEFINED)
typedef SSIZE_T ssize_t;
#endif
#endif
#endif  // !OPENSSL_NO_SOCK

#if defined(__cplusplus)
extern "C" {
#endif


#if !defined(OPENSSL_NO_SOCK)

// bio_ip_and_port_to_socket_and_addr creates a socket and fills in |*out_addr|
// and |*out_addr_length| with the correct values for connecting to |hostname|
// on |port_str|. It returns one on success or zero on error.
int bio_ip_and_port_to_socket_and_addr(int *out_sock,
                                       struct sockaddr_storage *out_addr,
                                       socklen_t *out_addr_length,
                                       const char *hostname,
                                       const char *port_str);

// bio_socket_nbio sets whether |sock| is non-blocking. It returns one on
// success and zero otherwise.
int bio_socket_nbio(int sock, int on);

// bio_clear_socket_error clears the last socket error on |sock|.
void bio_clear_socket_error(int sock);

// bio_sock_error_get_and_clear clears and returns the last socket error on |sock|.
int bio_sock_error_get_and_clear(int sock);

// bio_socket_should_retry returns non-zero if |return_value| indicates an error
// and the last socket error indicates that it's non-fatal.
int bio_socket_should_retry(int return_value);

#if defined(AF_UNIX) && !defined(OPENSSL_WINDOWS) && !defined(OPENSSL_ANDROID)
  // Winsock2 APIs don't support AF_UNIX.
  // > The values currently supported are AF_INET or AF_INET6, which are the
  // > Internet address family formats for IPv4 and IPv6.
  // https://learn.microsoft.com/en-us/windows/win32/api/winsock2/nf-winsock2-socket
#define AWS_LC_HAS_AF_UNIX 1
#endif

union bio_addr_st {
    struct sockaddr sa;
#ifdef AF_INET6
    struct sockaddr_in6 s_in6;
#endif
    struct sockaddr_in s_in;
#if AWS_LC_HAS_AF_UNIX
    struct sockaddr_un s_un;
#endif
};

#endif  // !OPENSSL_NO_SOCK

// bio_errno_should_retry returns non-zero if |return_value| indicates an error
// and |errno| indicates that it's non-fatal.
int bio_errno_should_retry(int return_value);

#if defined(__cplusplus)
}  // extern C
#endif

#endif  // OPENSSL_HEADER_BIO_INTERNAL_H
