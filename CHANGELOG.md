# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

<!-- next-header -->

## [Unreleased] - ReleaseDate

## [0.2.0] - 2026-03-25

### Changed

- Integers are serialized as IEEE 754 doubles (#3).
- Non-finite floats return an error instead of serializing as `null`.
- `i128` and `u128` return an error.
- Edition 2024; minimum supported Rust version is 1.85.

### Fixed

- Panic on numbers when serde_json's `arbitrary_precision` feature is enabled.

## [0.1.0] - 2020-11-13

### Added

- `to_string`, `to_vec` and `to_writer`.

<!-- next-url -->
[Unreleased]: https://github.com/l1h3r/serde_jcs/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/l1h3r/serde_jcs/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/l1h3r/serde_jcs/releases/tag/v0.1.0
