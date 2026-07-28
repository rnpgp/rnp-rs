# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Workspace split — rnp-src crate + botan-src integration ([#53](https://github.com/rnpgp/rnp-rs/pull/53)) by @[object]

### Fixed

- Correct botan source path + use prefix= for install ([#56](https://github.com/rnpgp/rnp-rs/pull/56)) by @[object]
- Wrap env::set_var in unsafe (Rust 2024 edition) ([#55](https://github.com/rnpgp/rnp-rs/pull/55)) by @[object]
- Use correct botan-src API + make install for cmake prefix ([#54](https://github.com/rnpgp/rnp-rs/pull/54)) by @[object]
