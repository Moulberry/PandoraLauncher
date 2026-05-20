#include <stdio.h>
#include <stdlib.h>

#include <openssl/crypto.h>
/*
 * This program is used during the FIPS libcrypto build on Windows. It is the
 * smallest possible executable that links with libcrypto and can trigger the
 * power-on self tests.
 */
int main(int argc, char *argv[]) {
  fprintf(stderr, "This will only print if the power-on self-tests pass.\n");
  // To ensure the linker links libcrypto call something
  fprintf(stderr, "FIPS mode is %d\n", FIPS_mode());

  exit(1);
}

