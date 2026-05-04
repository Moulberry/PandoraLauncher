#ifndef __USTR_HPP__
#define __USTR_HPP__

#include "ustr.h"
#include <string>

/// A class representing an interned string.
class Ustr {
    ustr_t _u;

public:
    /// Creates the empty string
    Ustr() { _u = ustr(""); }

    /// Create a new Ustr from a const char*
    /// It is assumed that `str` is a valid, non-null pointer. Passing anything
    /// else will result in undefined behaviour.
    /// Any invlid UTF-8 in `str` will be replaced by U+FFFD REPLACEMENT
    /// CHARACTER
    Ustr(const char* ptr) { _u = ustr(ptr); }

    /// Create a new Ustr from a std::string
    Ustr(const std::string& s) { _u = ustr(s.c_str()); }

    /// Returns true if the string is empty
    bool is_empty() const { return len() == 0; }

    /// Returns the length of the string, in bytes.
    size_t len() const { return ustr_len(_u); }

    /// Returns the precomputed hash of the string
    size_t hash() const { return ustr_hash(_u); }

    /// Easy conversion to the underlying C struct
    operator ustr_t() const { return _u; }

    /// Get the interned chars
    const char* c_str() const { return _u.ptr; }
};

#endif