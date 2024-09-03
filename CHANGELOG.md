# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.2] - 2024-09-04

### Fixed
- Fixed an issue where noise would occur when a track starts with a rest.

## [0.2.1] - 2024-08-14

### Fixed
- Fixed an issue where the pitch and volume were not updated on the last clock during the note on event in a musical scale.

## [0.2.0] - 2024-07-02

### Added
- N/A

### Changed
- Removed the binary component from this crate and separated it into its own crate to enhance modularity and focus on library functionality.

### Fixed
- N/A

## [0.1.0] - 2024-06-16
### Added
- Initial release of the project.
- Support for the following platforms:
  - x86_64-unknown-linux-gnu
  - aarch64-apple-darwin
  - x86_64-pc-windows-msvc
  - aarch64-pc-windows-msvc

### Changed
- N/A

### Fixed
- N/A
