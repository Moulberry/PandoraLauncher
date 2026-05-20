#include <stdlib.h>
#include "../internal/shgetc.h"
#include "../internal/floatscan.h"

static long double strtox(const char *s, char **p, int prec)
{
	FILE f;
	sh_fromstring(&f, s);
	shlim(&f, 0);
	long double y = __floatscan(&f, prec, 1);
	off_t cnt = shcnt(&f);
	if (p) *p = cnt ? (char *)s + cnt : (char *)s;
	return y;
}

double strtod(const char *restrict s, char **restrict p)
{
	return strtox(s, p, 1);
}
