#include <stddef.h>
#include <stdint.h>
#include <time.h>

/* string */
#define strcmp rust_sqlite_wasm_strcmp
int rust_sqlite_wasm_strcmp(const char *l, const char *r);
#define strcpy rust_sqlite_wasm_strcpy
char *rust_sqlite_wasm_strcpy(char *dest, const char *src);
#define strncpy rust_sqlite_wasm_strncpy
char *rust_sqlite_wasm_strncpy(char *d, const char *s, size_t n);
#define strcat rust_sqlite_wasm_strcat
char *rust_sqlite_wasm_strcat(char *dest, const char *src);
#define strncat rust_sqlite_wasm_strncat
char *rust_sqlite_wasm_strncat(char *d, const char *s, size_t n);
#define strcspn rust_sqlite_wasm_strcspn
size_t rust_sqlite_wasm_strcspn(const char *s, const char *c);
#define strspn rust_sqlite_wasm_strspn
size_t rust_sqlite_wasm_strspn(const char *s, const char *c);
#define strncmp rust_sqlite_wasm_strncmp
int rust_sqlite_wasm_strncmp(const char *l, const char *r, size_t n);
#define strrchr rust_sqlite_wasm_strrchr
char *rust_sqlite_wasm_strrchr(const char *s, int c);
#define strchr rust_sqlite_wasm_strchr
char *rust_sqlite_wasm_strchr(const char *s, int c);
#define memchr rust_sqlite_wasm_memchr
void *rust_sqlite_wasm_memchr(const void *src, int c, size_t n);
#define strlen rust_sqlite_wasm_strlen
size_t rust_sqlite_wasm_strlen(const char *s);
#define __memrchr rust_sqlite_wasm_memrchr
void *rust_sqlite_wasm_memrchr(const void *m, int c, size_t n);
#define __stpcpy rust_sqlite_wasm_stpcpy
char *rust_sqlite_wasm_stpcpy(char *d, const char *s);
#define __stpncpy rust_sqlite_wasm_stpncpy
char *rust_sqlite_wasm_stpncpy(char *d, const char *s, size_t n);
#define __strchrnul rust_sqlite_wasm_strchrnul
char *rust_sqlite_wasm_strchrnul(const char *s, int c);

/* math */
#define __fpclassifyl rust_sqlite_wasm_fpclassifyl
int rust_sqlite_wasm_fpclassifyl(long double x);
#define acosh rust_sqlite_wasm_acosh
double rust_sqlite_wasm_acosh(double x);
#define asinh rust_sqlite_wasm_asinh
double rust_sqlite_wasm_asinh(double x);
#define atanh rust_sqlite_wasm_atanh
double rust_sqlite_wasm_atanh(double x);
#define trunc rust_sqlite_wasm_trunc
double rust_sqlite_wasm_trunc(double x);
#define sqrt rust_sqlite_wasm_sqrt
double rust_sqlite_wasm_sqrt(double x);
#define fmodl rust_sqlite_wasm_fmodl
long double rust_sqlite_wasm_fmodl(long double x, long double y);
#define scalbn rust_sqlite_wasm_scalbn
double rust_sqlite_wasm_scalbn(double x, int n);
#define scalbnl rust_sqlite_wasm_scalbnl
long double rust_sqlite_wasm_scalbnl(long double x, int n);

/* stdlib */
#define atoi rust_sqlite_wasm_atoi
int rust_sqlite_wasm_atoi(const char *s);
#define strtol rust_sqlite_wasm_strtol
long rust_sqlite_wasm_strtol(const char *s, char **p, int base);
#define strtod rust_sqlite_wasm_strtod
double rust_sqlite_wasm_strtod(const char *s, char **p);
#define bsearch rust_sqlite_wasm_bsearch
void *rust_sqlite_wasm_bsearch(const void *key, const void *base, size_t nel,
                               size_t width,
                               int (*cmp)(const void *, const void *));
#define qsort rust_sqlite_wasm_qsort
void rust_sqlite_wasm_qsort(void *base, size_t nel, size_t width,
                            int (*cmp)(const void *, const void *));
#define __qsort_r rust_sqlite_wasm_qsort_r
void rust_sqlite_wasm_qsort_r(void *base, size_t nel, size_t width,
                              int (*cmpfun)(const void *, const void *, void *),
                              void *arg);

/* errno */
#define __errno_location rust_sqlite_wasm_errno_location
int *rust_sqlite_wasm_errno_location(void);

/* stdio */
#define sprintf rust_sqlite_wasm_sprintf
int rust_sqlite_wasm_sprintf(char *buffer, const char *format, ...);

/* malloc */
#define malloc rust_sqlite_wasm_malloc
void *rust_sqlite_wasm_malloc(size_t size);
#define realloc rust_sqlite_wasm_realloc
void *rust_sqlite_wasm_realloc(void *ptr, size_t size);
#define free rust_sqlite_wasm_free
void rust_sqlite_wasm_free(void *ptr);
#define calloc rust_sqlite_wasm_calloc
void *rust_sqlite_wasm_calloc(size_t num, size_t size);

/* time */
#define localtime rust_sqlite_wasm_localtime
struct tm *rust_sqlite_wasm_localtime(const time_t *t);

/* misc */
#define getentropy rust_sqlite_wasm_getentropy
int rust_sqlite_wasm_getentropy(void *buffer, size_t len);

/* exit */
#define abort rust_sqlite_wasm_abort
[[noreturn]] void rust_sqlite_wasm_abort();
#define __assert_fail rust_sqlite_wasm_assert_fail
[[noreturn]] void rust_sqlite_wasm_assert_fail(const char *expr,
                                               const char *file, int line,
                                               const char *func);
