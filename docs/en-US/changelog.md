# Changelog

[Back to Home](./README.md) | [简体中文](../zh-CN/changelog.md)

This document records the change history of public releases of IEC104 Simulator.

The format is based on Keep a Changelog and follows the idea of semantic versioning.

The actual release content is subject to each GitHub Release page.

## [0.2.0] - 2026-06-26

### Added

- Added i18n support.
- Added multilingual resource files to manage UI text.
- Added UI language switching support.
- Added English UI text to improve usability for non-Chinese users.

### Changed

- Changed UI text loading from hard-coded strings to language resource files.
- Improved the text organization of Master and Slave modules for future language expansion.
- Unified the wording style of some menus, buttons, prompts, and status messages.

### Notes

- This is a feature update. Users are recommended to upgrade from `v0.1.0` to `v0.2.0`.
- Current public releases are still mainly provided as Windows x64 installers.
- Future versions will continue to improve multilingual coverage and UI text consistency.

## [0.1.0] - 2026-04-13

### Added

- Released the first public installation version of IEC104 Simulator.
- Provided separate installers for Master and Slave simulators.
- Master Simulator supports connecting to slave devices.
- Master Simulator supports general interrogation, control, setpoint, and clock synchronization commands.
- Slave Simulator supports listening for master connections.
- Slave Simulator supports responding to general interrogation and control commands.
- Added point table import support.
- Added message viewing and structured parsing.
- Added SOE event records.
- Added PCAP/PCAPNG message export.
- Added local data persistence.

### Notes

- This is the first public release.
- Current public releases mainly target Windows x64.