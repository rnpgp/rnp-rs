# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed

- Remove is_packaging() — always compile, no heuristics by @[object]
- Is_packaging() via OUT_DIR path, not Cargo.toml.orig by @[object]
- Declare ws2_32 + crypt32 as Botan's INTERFACE_LINK_LIBRARIES by @[object]
