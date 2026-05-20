#include "../internal/libm.h"

long double fmodl(long double x, long double y)
{
	union ldshape ux = {x}, uy = {y};
	int ex = ux.i.se & 0x7fff;
	int ey = uy.i.se & 0x7fff;
	int sx = ux.i.se & 0x8000;

	if (y == 0 || isnan(y) || ex == 0x7fff)
		return (x*y)/(x*y);
	ux.i.se = ex;
	uy.i.se = ey;
	if (ux.f <= uy.f) {
		if (ux.f == uy.f)
			return 0*x;
		return x;
	}

	/* normalize x and y */
	if (!ex) {
		ux.f *= 0x1p120f;
		ex = ux.i.se - 120;
	}
	if (!ey) {
		uy.f *= 0x1p120f;
		ey = uy.i.se - 120;
	}

	uint64_t hi, lo, xhi, xlo, yhi, ylo;
	xhi = (ux.i2.hi & -1ULL>>16) | 1ULL<<48;
	yhi = (uy.i2.hi & -1ULL>>16) | 1ULL<<48;
	xlo = ux.i2.lo;
	ylo = uy.i2.lo;
	for (; ex > ey; ex--) {
		hi = xhi - yhi;
		lo = xlo - ylo;
		if (xlo < ylo)
			hi -= 1;
		if (hi >> 63 == 0) {
			if ((hi|lo) == 0)
				return 0*x;
			xhi = 2*hi + (lo>>63);
			xlo = 2*lo;
		} else {
			xhi = 2*xhi + (xlo>>63);
			xlo = 2*xlo;
		}
	}
	hi = xhi - yhi;
	lo = xlo - ylo;
	if (xlo < ylo)
		hi -= 1;
	if (hi >> 63 == 0) {
		if ((hi|lo) == 0)
			return 0*x;
		xhi = hi;
		xlo = lo;
	}
	for (; xhi >> 48 == 0; xhi = 2*xhi + (xlo>>63), xlo = 2*xlo, ex--);
	ux.i2.hi = xhi;
	ux.i2.lo = xlo;

	/* scale result */
	if (ex <= 0) {
		ux.i.se = (ex+120)|sx;
		ux.f *= 0x1p-120f;
	} else
		ux.i.se = ex|sx;
	return ux.f;
}
