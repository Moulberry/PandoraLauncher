#include <errno.h>

static _Thread_local int __errno_storage = 0;

int *__errno_location(void)
{
	return &__errno_storage;
}
