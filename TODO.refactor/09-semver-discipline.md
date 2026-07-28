# TODO.refactor/09-semver-discipline.md

## Semver discipline

**Priority:** P0
**Status:** POLICY

## Rules

1. **Distribution model changes** (vendored build mechanism, linking flags,
   included files) are **breaking changes** — bump minor or major version.
   Example: v0.1.7 (prebuilt-in-crate) → v0.1.9 (compile-from-source) should
   have been v0.2.0.

2. **New Cargo features** are additive — patch or minor bump.
   Example: adding `vendored-minimal` = minor bump.

3. **Changed feature semantics** (e.g., `vendored` now compiles from source
   instead of looking for prebuilts) — minor bump with loud release notes.

4. **MSRV bumps** — minor bump. Document in CHANGELOG.

5. **Release titles** must name consumer-visible changes:
   - Bad: `chore: release v0.1.9`
   - Good: `release v0.1.9: vendored now compiles from source`

6. **CHANGELOG.md** must have a consumer-facing summary, not just a commit log.
   Release-plz generates the commit log; we add the summary manually.

## Tasks

- [ ] Audit v0.1.7→v0.1.8→v0.1.9 for semver violations; document
- [ ] Adopt a RELEASE.md process document
- [ ] Consider switching from 0.1.x to 0.2.x after the rnp-src refactor
