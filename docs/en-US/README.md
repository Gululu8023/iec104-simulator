# IEC104 Simulator

[简体中文](../zh-CN/README.md) | [English](./README.md)

IEC104 Simulator is a simulator tool for IEC 60870-5-104 protocol testing, debugging, and verification.

This repository is used to publish public installers, release notes, and user documentation for IEC104 Simulator.

![IEC104 Simulator Master Home](../../assets/screenshots/master-home-linked.png)

## Features

IEC104 Simulator currently provides two standalone applications: Master Simulator and Slave Simulator. It can be used for master station simulation, slave station simulation, protocol development, device integration, functional verification, and message analysis.

### Master Simulator

- Connect to IEC104 slave devices.
- Send general interrogation, control, setpoint, and clock synchronization commands.
- View communication status, sent and received messages, and parsed results.
- Debug and verify slave device data.

### Slave Simulator

- Listen for IEC104 master connections.
- Respond to general interrogation and control commands.
- Import point tables and simulate status, measurement, and control data.
- Simulate data changes and event uploads.

### Common Features

- Real-time message monitoring.
- Structured message parsing.
- SOE event records.
- PCAP/PCAPNG capture export.
- Local data persistence.
- i18n support and UI language switching.

## Screenshots

### Master Home

![Master Home](../../assets/screenshots/master-home-linked.png)

### Slave Home

![Slave Home](../../assets/screenshots/slave-home-linked.png)

### Master Message Monitoring

![Master Message Monitoring](../../assets/screenshots/master-message-panel.png)

### Slave Data Simulation

![Slave Data Simulation](../../assets/screenshots/slave-data-simulated.png)

## Downloads

Please download the latest version from the GitHub Releases page:

- [Releases](https://github.com/Gululu8023/iec104-simulator/releases)

Common installer packages:

- `iec104-simulator-master_x.x.x_x64-setup.exe`: IEC104 Master Simulator installer.
- `iec104-simulator-slave_x.x.x_x64-setup.exe`: IEC104 Slave Simulator installer.

If a release provides both Master and Slave installers, download the one you need.

## How to Choose an Installer

- Download the `master` installer if you need to simulate a master station.
- Download the `slave` installer if you need to simulate a slave station.
- Install both packages if you need local master-slave integration testing.

## Documentation

- [Installation Guide](./install.md)
- [User Guide](./user-guide.md)
- [Changelog](./changelog.md)

## System Requirements

Current public releases mainly support:

- Windows 10 x64
- Windows 11 x64

Linux and macOS builds will be documented separately if they are provided in future releases.

## Version Notes

This repository is only used to publish public installers and user documentation.

The actual release content is subject to each GitHub Release page.