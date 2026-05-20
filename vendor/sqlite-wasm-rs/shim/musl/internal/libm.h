#include <stdint.h>
#include <float.h>
#include <math.h>

#define FORCE_EVAL(x)

union ldshape {
	long double f;
	struct {
		uint64_t lo;
		uint32_t mid;
		uint16_t top;
		uint16_t se;
	} i;
	struct {
		uint64_t lo;
		uint64_t hi;
	} i2;
};
