# TODO.refactor/04-bz2-internal-error.md

## Fix bzip2 bz_internal_error hole

**Priority:** P0
**Status:** TODO

## Goal

bzip2 1.0.8's Makefile builds libbz2.a without defining `bz_internal_error`,
leaving an undefined symbol. Consumers must shim it. We should define it in
our bzip2 build so consumers don't have to.

## Problem

```c
// bzip2 source: bzlib.c
void bz_internal_error(int errcode) {
    // This function is declared in bzlib_private.h but has no default
    // implementation. It's intended for the caller to override.
    // Without a definition, libbz2.a has an undefined symbol.
}
```

tebako currently shims this with:
```c
void bz_internal_error() { abort(); }
```

## Fix

Add a one-line C file to our bzip2 build that defines the function:

```c
// rnp-src/src/bz_internal_error.c
#include <stdlib.h>
void bz_internal_error(int errcode) {
    // bzip2 calls this on internal errors. Abort is the safest default.
    abort();
}
```

Compile and include it in libbz2.a:
```sh
gcc -c bz_internal_error.c -o bz_internal_error.o
ar rcs libbz2.a ... bz_internal_error.o
```

Or use `ar` to append to the existing libbz2.a after the standard build.

## Tasks

- [ ] Create rnp-src/src/bz_internal_error.c
- [ ] Compile and append to libbz2.a in build_bzip2()
- [ ] Verify: nm libbz2.a | grep bz_internal_error shows a T (defined) symbol
- [ ] Test: link without the tebako shim
