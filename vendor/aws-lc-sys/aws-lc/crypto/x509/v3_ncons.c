/*
 * Written by Dr Stephen N Henson (steve@openssl.org) for the OpenSSL
 * project.
 */
/* ====================================================================
 * Copyright (c) 2003 The OpenSSL Project.  All rights reserved.
 *
 * Redistribution and use in source and binary forms, with or without
 * modification, are permitted provided that the following conditions
 * are met:
 *
 * 1. Redistributions of source code must retain the above copyright
 *    notice, this list of conditions and the following disclaimer.
 *
 * 2. Redistributions in binary form must reproduce the above copyright
 *    notice, this list of conditions and the following disclaimer in
 *    the documentation and/or other materials provided with the
 *    distribution.
 *
 * 3. All advertising materials mentioning features or use of this
 *    software must display the following acknowledgment:
 *    "This product includes software developed by the OpenSSL Project
 *    for use in the OpenSSL Toolkit. (http://www.OpenSSL.org/)"
 *
 * 4. The names "OpenSSL Toolkit" and "OpenSSL Project" must not be used to
 *    endorse or promote products derived from this software without
 *    prior written permission. For written permission, please contact
 *    licensing@OpenSSL.org.
 *
 * 5. Products derived from this software may not be called "OpenSSL"
 *    nor may "OpenSSL" appear in their names without prior written
 *    permission of the OpenSSL Project.
 *
 * 6. Redistributions of any form whatsoever must retain the following
 *    acknowledgment:
 *    "This product includes software developed by the OpenSSL Project
 *    for use in the OpenSSL Toolkit (http://www.OpenSSL.org/)"
 *
 * THIS SOFTWARE IS PROVIDED BY THE OpenSSL PROJECT ``AS IS'' AND ANY
 * EXPRESSED OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
 * IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR
 * PURPOSE ARE DISCLAIMED.  IN NO EVENT SHALL THE OpenSSL PROJECT OR
 * ITS CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
 * SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT
 * NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES;
 * LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION)
 * HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
 * STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
 * ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED
 * OF THE POSSIBILITY OF SUCH DAMAGE.
 * ====================================================================
 *
 * This product includes cryptographic software written by Eric Young
 * (eay@cryptsoft.com).  This product includes software written by Tim
 * Hudson (tjh@cryptsoft.com). */

#include <stdio.h>
#include <string.h>

#include <openssl/asn1t.h>
#include <openssl/bytestring.h>
#include <openssl/conf.h>
#include <openssl/err.h>
#include <openssl/mem.h>
#include <openssl/obj.h>
#include <openssl/x509.h>

#include "../internal.h"
#include "internal.h"


static void *v2i_NAME_CONSTRAINTS(const X509V3_EXT_METHOD *method,
                                  const X509V3_CTX *ctx,
                                  const STACK_OF(CONF_VALUE) *nval);
static int i2r_NAME_CONSTRAINTS(const X509V3_EXT_METHOD *method, void *a,
                                BIO *bp, int ind);
static int do_i2r_name_constraints(const X509V3_EXT_METHOD *method,
                                   STACK_OF(GENERAL_SUBTREE) *trees, BIO *bp,
                                   int ind, const char *name);
static int print_nc_ipadd(BIO *bp, const ASN1_OCTET_STRING *ip);

static int nc_match(GENERAL_NAME *gen, NAME_CONSTRAINTS *nc);
static int nc_match_single(GENERAL_NAME *sub, GENERAL_NAME *gen);
static int nc_dn(X509_NAME *sub, X509_NAME *nm);
static int nc_dns(const ASN1_IA5STRING *sub, const ASN1_IA5STRING *dns);
static int nc_email(const ASN1_IA5STRING *sub, const ASN1_IA5STRING *eml);
static int nc_uri(const ASN1_IA5STRING *uri, const ASN1_IA5STRING *base);
static int nc_ip(const ASN1_OCTET_STRING *ip, const ASN1_OCTET_STRING *base);

const X509V3_EXT_METHOD v3_name_constraints = {
    NID_name_constraints,
    0,
    ASN1_ITEM_ref(NAME_CONSTRAINTS),
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    v2i_NAME_CONSTRAINTS,
    i2r_NAME_CONSTRAINTS,
    0,
    NULL,
};

ASN1_SEQUENCE(GENERAL_SUBTREE) = {
    ASN1_SIMPLE(GENERAL_SUBTREE, base, GENERAL_NAME),
    ASN1_IMP_OPT(GENERAL_SUBTREE, minimum, ASN1_INTEGER, 0),
    ASN1_IMP_OPT(GENERAL_SUBTREE, maximum, ASN1_INTEGER, 1),
} ASN1_SEQUENCE_END(GENERAL_SUBTREE)

ASN1_SEQUENCE(NAME_CONSTRAINTS) = {
    ASN1_IMP_SEQUENCE_OF_OPT(NAME_CONSTRAINTS, permittedSubtrees,
                             GENERAL_SUBTREE, 0),
    ASN1_IMP_SEQUENCE_OF_OPT(NAME_CONSTRAINTS, excludedSubtrees,
                             GENERAL_SUBTREE, 1),
} ASN1_SEQUENCE_END(NAME_CONSTRAINTS)


IMPLEMENT_ASN1_ALLOC_FUNCTIONS(GENERAL_SUBTREE)
IMPLEMENT_ASN1_ALLOC_FUNCTIONS(NAME_CONSTRAINTS)

static void *v2i_NAME_CONSTRAINTS(const X509V3_EXT_METHOD *method,
                                  const X509V3_CTX *ctx,
                                  const STACK_OF(CONF_VALUE) *nval) {
  STACK_OF(GENERAL_SUBTREE) **ptree = NULL;
  NAME_CONSTRAINTS *ncons = NULL;
  GENERAL_SUBTREE *sub = NULL;
  ncons = NAME_CONSTRAINTS_new();
  if (!ncons) {
    goto err;
  }
  for (size_t i = 0; i < sk_CONF_VALUE_num(nval); i++) {
    const CONF_VALUE *val = sk_CONF_VALUE_value(nval, i);
    CONF_VALUE tval;
    if (!strncmp(val->name, "permitted", 9) && val->name[9]) {
      ptree = &ncons->permittedSubtrees;
      tval.name = val->name + 10;
    } else if (!strncmp(val->name, "excluded", 8) && val->name[8]) {
      ptree = &ncons->excludedSubtrees;
      tval.name = val->name + 9;
    } else {
      OPENSSL_PUT_ERROR(X509V3, X509V3_R_INVALID_SYNTAX);
      goto err;
    }
    tval.value = val->value;
    sub = GENERAL_SUBTREE_new();
    if (!v2i_GENERAL_NAME_ex(sub->base, method, ctx, &tval, 1)) {
      goto err;
    }
    if (!*ptree) {
      *ptree = sk_GENERAL_SUBTREE_new_null();
    }
    if (!*ptree || !sk_GENERAL_SUBTREE_push(*ptree, sub)) {
      goto err;
    }
    sub = NULL;
  }

  return ncons;

err:
  NAME_CONSTRAINTS_free(ncons);
  GENERAL_SUBTREE_free(sub);
  return NULL;
}

static int i2r_NAME_CONSTRAINTS(const X509V3_EXT_METHOD *method, void *a,
                                BIO *bp, int ind) {
  NAME_CONSTRAINTS *ncons = a;
  do_i2r_name_constraints(method, ncons->permittedSubtrees, bp, ind,
                          "Permitted");
  do_i2r_name_constraints(method, ncons->excludedSubtrees, bp, ind, "Excluded");
  return 1;
}

static int do_i2r_name_constraints(const X509V3_EXT_METHOD *method,
                                   STACK_OF(GENERAL_SUBTREE) *trees, BIO *bp,
                                   int ind, const char *name) {
  GENERAL_SUBTREE *tree;
  size_t i;
  if (sk_GENERAL_SUBTREE_num(trees) > 0) {
    BIO_printf(bp, "%*s%s:\n", ind, "", name);
  }
  for (i = 0; i < sk_GENERAL_SUBTREE_num(trees); i++) {
    tree = sk_GENERAL_SUBTREE_value(trees, i);
    BIO_printf(bp, "%*s", ind + 2, "");
    if (tree == NULL) {
      return 0;
    }
    if (tree->base->type == GEN_IPADD) {
      print_nc_ipadd(bp, tree->base->d.ip);
    } else {
      GENERAL_NAME_print(bp, tree->base);
    }
    BIO_puts(bp, "\n");
  }
  return 1;
}

static int print_nc_ipadd(BIO *bp, const ASN1_OCTET_STRING *ip) {
  int i, len;
  unsigned char *p;
  p = ip->data;
  len = ip->length;
  BIO_puts(bp, "IP:");
  if (len == 8) {
    BIO_printf(bp, "%d.%d.%d.%d/%d.%d.%d.%d", p[0], p[1], p[2], p[3], p[4],
               p[5], p[6], p[7]);
  } else if (len == 32) {
    for (i = 0; i < 16; i++) {
      uint16_t v = ((uint16_t)p[0] << 8) | p[1];
      BIO_printf(bp, "%X", v);
      p += 2;
      if (i == 7) {
        BIO_puts(bp, "/");
      } else if (i != 15) {
        BIO_puts(bp, ":");
      }
    }
  } else {
    BIO_printf(bp, "IP Address:<invalid>");
  }
  return 1;
}

//-
// Check a certificate conforms to a specified set of constraints.
// Return values:
//   X509_V_OK: All constraints obeyed.
//   X509_V_ERR_PERMITTED_VIOLATION: Permitted subtree violation.
//   X509_V_ERR_EXCLUDED_VIOLATION: Excluded subtree violation.
//   X509_V_ERR_SUBTREE_MINMAX: Min or max values present and matching type.
//   X509_V_ERR_UNSPECIFIED: Unspecified error.
//   X509_V_ERR_UNSUPPORTED_CONSTRAINT_TYPE: Unsupported constraint type.
//   X509_V_ERR_UNSUPPORTED_CONSTRAINT_SYNTAX: Bad or unsupported constraint
//     syntax.
//   X509_V_ERR_UNSUPPORTED_NAME_SYNTAX: Bad or unsupported syntax of name.

int NAME_CONSTRAINTS_check(X509 *x, NAME_CONSTRAINTS *nc) {
  int r, i;
  size_t j;
  X509_NAME *nm;

  nm = X509_get_subject_name(x);

  // Guard against certificates with an excessive number of names or
  // constraints causing a computationally expensive name constraints
  // check.
  size_t name_count =
      X509_NAME_entry_count(nm) + sk_GENERAL_NAME_num(x->altname);
  size_t constraint_count = sk_GENERAL_SUBTREE_num(nc->permittedSubtrees) +
                            sk_GENERAL_SUBTREE_num(nc->excludedSubtrees);
  size_t check_count = constraint_count * name_count;
  if (name_count < (size_t)X509_NAME_entry_count(nm) ||
      constraint_count < sk_GENERAL_SUBTREE_num(nc->permittedSubtrees) ||
      (constraint_count && check_count / constraint_count != name_count) ||
      check_count > 1 << 20) {
    return X509_V_ERR_UNSPECIFIED;
  }

  if (X509_NAME_entry_count(nm) > 0) {
    GENERAL_NAME gntmp;
    gntmp.type = GEN_DIRNAME;
    gntmp.d.directoryName = nm;

    r = nc_match(&gntmp, nc);

    if (r != X509_V_OK) {
      return r;
    }

    gntmp.type = GEN_EMAIL;

    // Process any email address attributes in subject name

    for (i = -1;;) {
      i = X509_NAME_get_index_by_NID(nm, NID_pkcs9_emailAddress, i);
      if (i == -1) {
        break;
      }
      const X509_NAME_ENTRY *ne = X509_NAME_get_entry(nm, i);
      gntmp.d.rfc822Name = X509_NAME_ENTRY_get_data(ne);
      if (gntmp.d.rfc822Name->type != V_ASN1_IA5STRING) {
        return X509_V_ERR_UNSUPPORTED_NAME_SYNTAX;
      }

      r = nc_match(&gntmp, nc);

      if (r != X509_V_OK) {
        return r;
      }
    }
  }

  for (j = 0; j < sk_GENERAL_NAME_num(x->altname); j++) {
    GENERAL_NAME *gen = sk_GENERAL_NAME_value(x->altname, j);
    r = nc_match(gen, nc);
    if (r != X509_V_OK) {
      return r;
    }
  }

  return X509_V_OK;
}

int cn2dnsid(ASN1_STRING *cn, unsigned char **dnsid, size_t *idlen) {
  assert(dnsid != NULL && idlen != NULL);

  // Don't leave outputs uninitialized
  *dnsid = NULL;
  *idlen = 0;
  
  // Per RFC 6125, DNS-IDs representing internationalized domain names appear
  // in certificates in A-label encoded form:
  //
  // https://tools.ietf.org/html/rfc6125#section-6.4.2
  //
  // The same applies to CNs which are intended to represent DNS names.
  // However, while in the SAN DNS-IDs are IA5Strings, as CNs they may be
  // needlessly encoded in 16-bit Unicode.  We perform a conversion to UTF-8
  // to ensure that we get an ASCII representation of any CNs that are
  // representable as ASCII, but just not encoded as ASCII.  The UTF-8 form
  // may contain some non-ASCII octets, and that's fine, such CNs are not
  // valid legacy DNS names.
  //
  // Note, 'int' is the return type of ASN1_STRING_to_UTF8() so that's what
  // we must use for 'utf8_length'.
  unsigned char *utf8_value = NULL;
  int utf8_length = ASN1_STRING_to_UTF8(&utf8_value, cn);
  if (utf8_length < 0) {
    return X509_V_ERR_OUT_OF_MEM;
  }
  
  // Some certificates have had names that include a *trailing* NUL byte.
  // Remove these harmless NUL characters. They would otherwise yield false
  // alarms with the following embedded NUL check.
  while (utf8_length > 0 && utf8_value[utf8_length - 1] == '\0') {
    --utf8_length;
  }

  // Reject *embedded* NULs
  if (OPENSSL_memchr(utf8_value, 0, utf8_length) != NULL) {
    OPENSSL_free(utf8_value);
    return X509_V_ERR_UNSUPPORTED_NAME_SYNTAX;
  }

  int isdnsname = 0;
  
  // XXX: Deviation from strict DNS name syntax, also check names with '_'
  // Check DNS name syntax, any '-' or '.' must be internal,
  // and on either side of each '.' we can't have a '-' or '.'.
  //
  // If the name has just one label, we don't consider it a DNS name.  This
  // means that "CN=sometld" cannot be precluded by DNS name constraints, but
  // that is not a problem.
  for (int i = 0; i < utf8_length; ++i) {
    const unsigned char c = utf8_value[i];

    if ((c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') ||
        (c >= '0' && c <= '9') || c == '_') {
      continue;
    }

    // Dot and hyphen cannot be first or last.
    if (i > 0 && i < utf8_length - 1) {
      if (c == '-') {
        continue;
      }
      
      // Next to a dot the preceding and following characters must not be
      // another dot or a hyphen.  Otherwise, record that the name is
      // plausible, since it has two or more labels.
      if (c == '.' && utf8_value[i + 1] != '.' && utf8_value[i - 1] != '-' &&
          utf8_value[i + 1] != '-') {
        isdnsname = 1;
        continue;
      }
    }
    isdnsname = 0;
    break;
  }

  if (isdnsname) {
    *dnsid = utf8_value;
    *idlen = (size_t)utf8_length;
    return X509_V_OK;
  }
  OPENSSL_free(utf8_value);
  return X509_V_OK;
}

// Check CN against DNS-ID name constraints.
int NAME_CONSTRAINTS_check_CN(X509 *x, NAME_CONSTRAINTS *nc) {
  int ret = 0;
  const X509_NAME *nm = X509_get_subject_name(x);
  ASN1_STRING stmp = {.length = 0, .type = V_ASN1_IA5STRING, .data = NULL, .flags = 0};
  GENERAL_NAME gntmp = {.type = GEN_DNS, .d = {.dNSName = &stmp}};

  // Process any commonName attributes in subject name
  for (int i = -1;;) {
    X509_NAME_ENTRY *ne = NULL;
    ASN1_STRING *cn = NULL;
    unsigned char *idval = NULL;
    size_t idlen = 0;

    i = X509_NAME_get_index_by_NID(nm, NID_commonName, i);
    if (i == -1) {
      break;
    }
    ne = X509_NAME_get_entry(nm, i);
    cn = X509_NAME_ENTRY_get_data(ne);

    // Only process attributes that look like host names
    if ((ret = cn2dnsid(cn, &idval, &idlen)) != X509_V_OK) {
      return ret;
    }
    if (idlen == 0) {
      continue;
    }

    stmp.length = idlen;
    stmp.data = idval;
    ret = nc_match(&gntmp, nc);
    OPENSSL_free(idval);
    if (ret != X509_V_OK) {
      return ret;
    }
  }
  return X509_V_OK;
}

static int nc_match(GENERAL_NAME *gen, NAME_CONSTRAINTS *nc) {
  GENERAL_SUBTREE *sub;
  int r, match = 0;
  size_t i;

  // Permitted subtrees: if any subtrees exist of matching the type at
  // least one subtree must match.

  for (i = 0; i < sk_GENERAL_SUBTREE_num(nc->permittedSubtrees); i++) {
    sub = sk_GENERAL_SUBTREE_value(nc->permittedSubtrees, i);
    if (gen->type != sub->base->type) {
      continue;
    }
    if (sub->minimum || sub->maximum) {
      return X509_V_ERR_SUBTREE_MINMAX;
    }
    // If we already have a match don't bother trying any more
    if (match == 2) {
      continue;
    }
    if (match == 0) {
      match = 1;
    }
    r = nc_match_single(gen, sub->base);
    if (r == X509_V_OK) {
      match = 2;
    } else if (r != X509_V_ERR_PERMITTED_VIOLATION) {
      return r;
    }
  }

  if (match == 1) {
    return X509_V_ERR_PERMITTED_VIOLATION;
  }

  // Excluded subtrees: must not match any of these

  for (i = 0; i < sk_GENERAL_SUBTREE_num(nc->excludedSubtrees); i++) {
    sub = sk_GENERAL_SUBTREE_value(nc->excludedSubtrees, i);
    if (gen->type != sub->base->type) {
      continue;
    }
    if (sub->minimum || sub->maximum) {
      return X509_V_ERR_SUBTREE_MINMAX;
    }

    r = nc_match_single(gen, sub->base);
    if (r == X509_V_OK) {
      return X509_V_ERR_EXCLUDED_VIOLATION;
    } else if (r != X509_V_ERR_PERMITTED_VIOLATION) {
      return r;
    }
  }

  return X509_V_OK;
}

static int nc_match_single(GENERAL_NAME *gen, GENERAL_NAME *base) {
  switch (base->type) {
    case GEN_DIRNAME:
      return nc_dn(gen->d.directoryName, base->d.directoryName);

    case GEN_DNS:
      return nc_dns(gen->d.dNSName, base->d.dNSName);

    case GEN_EMAIL:
      return nc_email(gen->d.rfc822Name, base->d.rfc822Name);

    case GEN_URI:
      return nc_uri(gen->d.uniformResourceIdentifier,
                    base->d.uniformResourceIdentifier);

    case GEN_IPADD:
      return nc_ip(gen->d.iPAddress, base->d.iPAddress);

    default:
      return X509_V_ERR_UNSUPPORTED_CONSTRAINT_TYPE;
  }
}

// directoryName name constraint matching. The canonical encoding of
// X509_NAME makes this comparison easy. It is matched if the subtree is a
// subset of the name.

static int nc_dn(X509_NAME *nm, X509_NAME *base) {
  // Ensure canonical encodings are up to date.
  if (nm->modified && i2d_X509_NAME(nm, NULL) < 0) {
    return X509_V_ERR_OUT_OF_MEM;
  }
  if (base->modified && i2d_X509_NAME(base, NULL) < 0) {
    return X509_V_ERR_OUT_OF_MEM;
  }
  if (base->canon_enclen > nm->canon_enclen) {
    return X509_V_ERR_PERMITTED_VIOLATION;
  }
  if (OPENSSL_memcmp(base->canon_enc, nm->canon_enc, base->canon_enclen)) {
    return X509_V_ERR_PERMITTED_VIOLATION;
  }
  return X509_V_OK;
}

static int starts_with(const CBS *cbs, uint8_t c) {
  return CBS_len(cbs) > 0 && CBS_data(cbs)[0] == c;
}

static int equal_case(const CBS *a, const CBS *b) {
  if (CBS_len(a) != CBS_len(b)) {
    return 0;
  }
  // Note we cannot use |OPENSSL_strncasecmp| because that would stop
  // iterating at NUL.
  const uint8_t *a_data = CBS_data(a), *b_data = CBS_data(b);
  for (size_t i = 0; i < CBS_len(a); i++) {
    if (OPENSSL_tolower(a_data[i]) != OPENSSL_tolower(b_data[i])) {
      return 0;
    }
  }
  return 1;
}

static int has_suffix_case(const CBS *a, const CBS *b) {
  if (CBS_len(a) < CBS_len(b)) {
    return 0;
  }
  CBS copy = *a;
  CBS_skip(&copy, CBS_len(a) - CBS_len(b));
  return equal_case(&copy, b);
}

static int nc_dns(const ASN1_IA5STRING *dns, const ASN1_IA5STRING *base) {
  CBS dns_cbs, base_cbs;
  CBS_init(&dns_cbs, dns->data, dns->length);
  CBS_init(&base_cbs, base->data, base->length);

  // Empty matches everything
  if (CBS_len(&base_cbs) == 0) {
    return X509_V_OK;
  }

  // If |base_cbs| begins with a '.', do a simple suffix comparison. This is
  // not part of RFC5280, but is part of OpenSSL's original behavior.
  if (starts_with(&base_cbs, '.')) {
    if (has_suffix_case(&dns_cbs, &base_cbs)) {
      return X509_V_OK;
    }
    return X509_V_ERR_PERMITTED_VIOLATION;
  }

  // Otherwise can add zero or more components on the left so compare RHS
  // and if dns is longer and expect '.' as preceding character.
  if (CBS_len(&dns_cbs) > CBS_len(&base_cbs)) {
    uint8_t dot;
    if (!CBS_skip(&dns_cbs, CBS_len(&dns_cbs) - CBS_len(&base_cbs) - 1) ||
        !CBS_get_u8(&dns_cbs, &dot) || dot != '.') {
      return X509_V_ERR_PERMITTED_VIOLATION;
    }
  }

  if (!equal_case(&dns_cbs, &base_cbs)) {
    return X509_V_ERR_PERMITTED_VIOLATION;
  }

  return X509_V_OK;
}

static int nc_email(const ASN1_IA5STRING *eml, const ASN1_IA5STRING *base) {
  CBS eml_cbs, base_cbs;
  CBS_init(&eml_cbs, eml->data, eml->length);
  CBS_init(&base_cbs, base->data, base->length);

  // TODO(davidben): In OpenSSL 1.1.1, this switched from the first '@' to the
  // last one. Match them here, or perhaps do an actual parse. Looks like
  // multiple '@'s may be allowed in quoted strings.
  CBS eml_local, base_local;
  if (!CBS_get_until_first(&eml_cbs, &eml_local, '@')) {
    return X509_V_ERR_UNSUPPORTED_NAME_SYNTAX;
  }
  int base_has_at = CBS_get_until_first(&base_cbs, &base_local, '@');

  // Special case: initial '.' is RHS match
  if (!base_has_at && starts_with(&base_cbs, '.')) {
    if (has_suffix_case(&eml_cbs, &base_cbs)) {
      return X509_V_OK;
    }
    return X509_V_ERR_PERMITTED_VIOLATION;
  }

  // If we have anything before '@' match local part
  if (base_has_at) {
    // TODO(davidben): This interprets a constraint of "@example.com" as
    // "example.com", which is not part of RFC5280.
    if (CBS_len(&base_local) > 0) {
      // Case sensitive match of local part
      if (!CBS_mem_equal(&base_local, CBS_data(&eml_local),
                         CBS_len(&eml_local))) {
        return X509_V_ERR_PERMITTED_VIOLATION;
      }
    }
    // Position base after '@'
    assert(starts_with(&base_cbs, '@'));
    CBS_skip(&base_cbs, 1);
  }

  // Just have hostname left to match: case insensitive
  assert(starts_with(&eml_cbs, '@'));
  CBS_skip(&eml_cbs, 1);
  if (!equal_case(&base_cbs, &eml_cbs)) {
    return X509_V_ERR_PERMITTED_VIOLATION;
  }

  return X509_V_OK;
}

static int nc_uri(const ASN1_IA5STRING *uri, const ASN1_IA5STRING *base) {
  CBS uri_cbs, base_cbs;
  CBS_init(&uri_cbs, uri->data, uri->length);
  CBS_init(&base_cbs, base->data, base->length);

  // Check for foo:// and skip past it
  CBS scheme;
  uint8_t byte;
  if (!CBS_get_until_first(&uri_cbs, &scheme, ':') ||
      !CBS_skip(&uri_cbs, 1) ||  // Skip the colon
      !CBS_get_u8(&uri_cbs, &byte) || byte != '/' ||
      !CBS_get_u8(&uri_cbs, &byte) || byte != '/') {
    return X509_V_ERR_UNSUPPORTED_NAME_SYNTAX;
  }

  // Look for a port indicator as end of hostname first. Otherwise look for
  // trailing slash, or the end of the string.
  // TODO(davidben): This is not a correct URI parser and mishandles IPv6
  // literals.
  CBS host;
  if (!CBS_get_until_first(&uri_cbs, &host, ':') &&
      !CBS_get_until_first(&uri_cbs, &host, '/')) {
    host = uri_cbs;
  }

  if (CBS_len(&host) == 0) {
    return X509_V_ERR_UNSUPPORTED_NAME_SYNTAX;
  }

  // Special case: initial '.' is RHS match
  if (starts_with(&base_cbs, '.')) {
    if (has_suffix_case(&host, &base_cbs)) {
      return X509_V_OK;
    }
    return X509_V_ERR_PERMITTED_VIOLATION;
  }

  if (!equal_case(&base_cbs, &host)) {
    return X509_V_ERR_PERMITTED_VIOLATION;
  }

  return X509_V_OK;
}

#define IPV4_ADDR_LEN 4
#define IPV4_CIDR_LEN (IPV4_ADDR_LEN * 2)
#define IPV6_ADDR_LEN 16
#define IPV6_CIDR_LEN (IPV6_ADDR_LEN * 2)

static int validate_ipv4_cidr_mask(const uint8_t *mask_ptr, size_t mask_len) {
  assert(mask_len == IPV4_ADDR_LEN);

  uint32_t mask = ((uint32_t)mask_ptr[0] << 24) |
                  ((uint32_t)mask_ptr[1] << 16) | ((uint32_t)mask_ptr[2] << 8) |
                  ((uint32_t)mask_ptr[3]);

  if (mask != 0 && ((~mask + 1) & ~mask)) {
    return 0;
  }

  return 1;
}

static int validate_ipv6_cidr_mask(const uint8_t *mask_ptr, size_t mask_len) {
  assert(mask_len == IPV6_ADDR_LEN);

  uint64_t mask_high =
      (((uint64_t)mask_ptr[0]) << 56) | ((uint64_t)mask_ptr[1] << 48) |
      ((uint64_t)mask_ptr[2] << 40) | ((uint64_t)mask_ptr[3] << 32) |
      ((uint64_t)mask_ptr[4] << 24) | ((uint64_t)mask_ptr[5] << 16) |
      ((uint64_t)mask_ptr[6] << 8) | ((uint64_t)mask_ptr[7]);

  uint64_t mask_low =
      (((uint64_t)mask_ptr[8]) << 56) | ((uint64_t)mask_ptr[9] << 48) |
      ((uint64_t)mask_ptr[10] << 40) | ((uint64_t)mask_ptr[11] << 32) |
      ((uint64_t)mask_ptr[12] << 24) | ((uint64_t)mask_ptr[13] << 16) |
      ((uint64_t)mask_ptr[14] << 8) | ((uint64_t)mask_ptr[15]);

  if(mask_high == 0) {
    // if the high bits are all 0, then low bits must be all 0.
    return mask_low == 0;
  }

  // Check that all the 1's are contiguous from left to right
  uint64_t inv_mask_high = ~mask_high;
  if ((inv_mask_high + 1) & inv_mask_high) {
    return 0;
  }

  // If there was one or more 0's then low has to be all zeros
  if(inv_mask_high) {
    return mask_low == 0;
  }

  // Otherwise low may be all zero's, and if not then the 1's must be contiguous
  // from left to right
  return (mask_low == 0 || ((~mask_low + 1) & ~mask_low) == 0);
}

int validate_cidr_mask(CBS *cidr_mask) {
  size_t mask_len = CBS_len(cidr_mask);
  switch (mask_len) {
    case IPV4_ADDR_LEN:
      return validate_ipv4_cidr_mask(CBS_data(cidr_mask), mask_len);
    case IPV6_ADDR_LEN:
      return validate_ipv6_cidr_mask(CBS_data(cidr_mask), mask_len);
    default:
      return 0;
  }
}

static int nc_ip(const ASN1_OCTET_STRING *ip, const ASN1_OCTET_STRING *base) {
  CBS ip_cbs, cidr_cbs, cidr_addr, cidr_mask;

  CBS_init(&ip_cbs, ip->data, ip->length);
  CBS_init(&cidr_cbs, base->data, base->length);

  size_t ip_len = CBS_len(&ip_cbs);
  size_t cidr_len = CBS_len(&cidr_cbs);

  // Next the IP length should be either the length of an IPv4 or IPv6 address.
  // Finally the CIDR length should either be the length of an IPv4 address+mask
  // or IPv6 address+mask.
  if (!((ip_len == IPV4_ADDR_LEN) || (ip_len == IPV6_ADDR_LEN)) ||
      !((cidr_len == IPV4_CIDR_LEN) || (cidr_len == IPV6_CIDR_LEN))) {
    return X509_V_ERR_UNSUPPORTED_NAME_SYNTAX;
  }

  // Validate that the CIDR length is twice the size of the provided IP (address
  // + mask).
  if (ip_len * 2 != cidr_len) {
    return X509_V_ERR_PERMITTED_VIOLATION;
  }

  if (!CBS_get_bytes(&cidr_cbs, &cidr_addr, ip_len) ||
      !CBS_get_bytes(&cidr_cbs, &cidr_mask, ip_len) || CBS_len(&cidr_cbs) > 0) {
    return X509_V_ERR_UNSUPPORTED_NAME_SYNTAX;
  }

  // Checking for wrong mask definition that are not valid CIDR prefixes.
  // For example: 255.0.255.0
  if (!validate_cidr_mask(&cidr_mask)) {
    return X509_V_ERR_UNSUPPORTED_NAME_SYNTAX;
  }

  uint8_t ip_byte = 0, cidr_addr_byte = 0, cidr_mask_byte = 0;

  for (size_t i = 0; i < ip_len; i++) {
    if (!CBS_get_u8(&ip_cbs, &ip_byte) ||
        !CBS_get_u8(&cidr_addr, &cidr_addr_byte) ||
        !CBS_get_u8(&cidr_mask, &cidr_mask_byte) ||
        ((ip_byte & cidr_mask_byte) != (cidr_addr_byte & cidr_mask_byte))) {
      return X509_V_ERR_PERMITTED_VIOLATION;
    }
  }

  return X509_V_OK;
}
