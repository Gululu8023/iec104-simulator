# Changelog

This project keeps user-facing release notes. The application product version and the `iec60870-parser` component version are managed independently.

## [Unreleased] - v1.0.0

### Added

- Publish the complete Rust, Tauri, and Vue source for separate Master and Slave applications.
- Document station/group interrogation, counter interrogation and freeze/reset operations.
- Provide UI workflows for controls, setpoints, bitstring, read, test, clock sync, reset, and file transfer.
- Provide Slave SBO/direct execution, command mapping, SQ=1, SOE, 15 waveforms, force upload, and avalanche testing.
- Provide CSV/JSON/XML point-table preview/import, tree parsing, and CSV/TXT/PCAP/PCAPNG frame export.
- Add bilingual Help, protocol support, limitations, installation, upgrade, and development documentation.

### Changed

- Restructure the project as Master/Slave applications with shared backend/parser workspace crates.
- Apply bounded reliable receive backpressure and a unified reconnect state machine.
- Move hot SQLite writes to batched transactions outside asynchronous protocol workers.
- Change the project license from MIT to Apache License 2.0.
- Convert the public repository into a combined source, documentation, and release repository.

### Fixed

- Prevent an ACK race from leaving the outbound window waiting permanently.
- Unify recovery from disconnect, STARTDT/TESTFR timeout, and exhausted t1 retries.
- Close and reap connection, monitor, and simulation tasks during lifecycle transitions.
- Remove duplicate mapped-control persistence and synchronous database blocking in protocol loops.
- Make duplicate connection admission atomic.
- Fix start/stop races in the frontend runtime synchronization loop.

### Compatibility

- The product version is 1.0.0; the internal `iec60870-parser` component remains 0.1.0.
- v1 accepts schema version 1 and does not migrate pre-v1 databases. Back up data and rename an incompatible old database before allowing v1 to create a new one.
- Official installation and verification cover Windows 10/11 x64.

### Known Issues

- Parameter and security extension ASDUs have partial parsing only, without complete Master/Slave workflows.
- Point-table export and independent SOE CSV export are unavailable.
- File transfer is single-section and limited by the maximum ASDU configuration.

## [0.2.0] - 2026-06-26

### Added

- Add Simplified Chinese and English UI resources and language switching.

### Changed

- Centralize localization for major menus, actions, status text, and errors.

## [0.1.0] - 2026-04-13

### Added

- Establish the public release repository and Windows Master/Slave package structure.
- Provide initial connection, listener, point-table import, frame inspection, and protocol integration features.

[Unreleased]: https://github.com/Gululu8023/iec104-simulator/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/Gululu8023/iec104-simulator/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/Gululu8023/iec104-simulator/releases/tag/v0.1.0
