# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

<!-- next-header -->

## [Unreleased] - ReleaseDate

## [0.4.0] - 2026-11-22

### Changed

- **Breaking:** Duplicate object keys return a data error.
- **Breaking:** Non-finite floats and integers that serialize differently as an `f64` return a data error instead of an I/O error.
- Integers outside ±`MAX_SAFE_INTEGER` that serialize the same as an `f64` no longer return an error (#3).
- Require `serde` 1.0.166 and `serde_json` 1.0.105.

### Fixed

- Non-finite floats nested in arrays, objects and options return an error instead of serializing as `null`.
- Integer map keys outside ±`MAX_SAFE_INTEGER` serialize as decimal strings instead of returning an error.
- A `Serialize` impl that ignores an error returns an error instead of producing invalid JSON.
- With serde_json's `arbitrary_precision` feature, integers that serialize differently as an `f64` return an error instead of rounding.
- A `RawValue` with duplicate object keys returns an error instead of keeping the last value.

## [0.3.0] - 2026-11-19

### Added

- `MAX_SAFE_INTEGER`.

### Changed

- Integers outside ±`MAX_SAFE_INTEGER` return an error (#3).
- Update `ryu-js` to 1.0.

### Fixed

- `i128` and `u128` within that range serialize instead of returning an error.

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
[Unreleased]: https://github.com/l1h3r/serde_jcs/compare/v0.4.0...HEAD
[0.4.0]: https://github.com/l1h3r/serde_jcs/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/l1h3r/serde_jcs/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/l1h3r/serde_jcs/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/l1h3r/serde_jcs/releases/tag/v0.1.0
