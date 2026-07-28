# TODO.refactor/05-macos-deployment-target.md

## Set macOS deployment target to 11.0

**Priority:** P0
**Status:** TODO

## Goal

Built artifacts must link for consumers targeting macOS 11+. Currently the
build may target the SDK version on the CI runner (macOS 14/15), producing
binaries that won't load on macOS 11/12/13.

## Fix

Set `MACOSX_DEPLOYMENT_TARGET=11.0` in all cmake/make invocations on macOS:

```rust
if cfg!(target_os = "macos") {
    env::set_var("MACOSX_DEPLOYMENT_TARGET", "11.0");
}
```

And pass to CMake:
```
-DCMAKE_OSX_DEPLOYMENT_TARGET=11.0
```

And to Botan configure (if building manually):
```
--os-flags="-mmacosx-version-min=11.0"
```

## Considerations

- This only affects macOS builds.
- 11.0 is the oldest supported macOS for Apple Silicon (M1).
- Intel macOS goes back further (10.13) but 11.0 is the practical floor.
- The deployment target must be consistent across Botan, json-c, zlib, bzip2,
  and librnp — otherwise the linker may reject mismatched targets.

## Tasks

- [ ] Set MACOSX_DEPLOYMENT_TARGET=11.0 in build.rs for macOS
- [ ] Pass -DCMAKE_OSX_DEPLOYMENT_TARGET=11.0 to all cmake invocations
- [ ] Verify: otool -l librnp.a shows LC_BUILD_VERSION with minos 11.0
