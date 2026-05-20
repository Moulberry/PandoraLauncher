#ifndef __USTR_H__
#define __USTR_H__

#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct {
    const char* ptr;
} ustr_t;

/*
    Create a new ustr_t from the given char*.
    It is assumed that `str` is a valid, non-null pointer. Passing anything else
    will result in undefined behaviour.
    Any invlid UTF-8 in `str` will be replaced by U+FFFD REPLACEMENT CHARACTER
*/
ustr_t ustr(const char* str);

/*
    Returns the length of the given ustr_t in bytes.
*/
size_t ustr_len(ustr_t u);

/*
    Returns the precomputed hash for the given ustr_t.
*/
uint64_t ustr_hash(ustr_t u);

#ifdef __cplusplus
}
#endif

#endif