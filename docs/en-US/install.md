# Installation Guide

[Back to Home](./README.md) | [简体中文](../zh-CN/install.md)

This document describes how to download, install, and start IEC104 Simulator.

## 1. Download the Installer

Please download the latest version from the GitHub Releases page:

- [Releases](https://github.com/Gululu8023/iec104-simulator/releases)

The release assets usually include the following installer packages:

- `iec104-simulator-master_x.x.x_x64-setup.exe`
- `iec104-simulator-slave_x.x.x_x64-setup.exe`

Where:

- The `master` installer is used to install IEC104 Master Simulator.
- The `slave` installer is used to install IEC104 Slave Simulator.
- `x.x.x` represents the version number.
- `x64` represents the 64-bit Windows installer.

## 2. Choose an Installer

Choose the installer according to your use case:

| Use Case | Recommended Installer |
| --- | --- |
| Simulate an IEC104 master station | `iec104-simulator-master_x.x.x_x64-setup.exe` |
| Simulate an IEC104 slave station | `iec104-simulator-slave_x.x.x_x64-setup.exe` |
| Local master-slave integration testing | Install both Master and Slave |

## 3. Install the Application

Double-click the downloaded `.exe` installer and follow the installation wizard.

The default installation path is recommended.

If Windows shows a security warning, make sure the installer was downloaded from this repository's Release page before continuing.

## 4. Start the Application

After installation, you can start the application in one of the following ways:

- Start it from the Windows Start menu.
- Start it from the desktop shortcut.
- Start it directly from the installation directory.

## 5. Connect to a Slave Device with Master Simulator

After starting Master Simulator, you usually need to configure the following information:

- Slave IP address.
- Slave port.
- Common address.
- Cause of transmission length.
- Common address length.
- Information object address length.
- Other communication parameters.

After configuration, click the connect button to establish an IEC104 connection with the slave device.

## 6. Listen for Master Connections with Slave Simulator

After starting Slave Simulator, you usually need to configure the following information:

- Local listening IP.
- Local listening port.
- Common address.
- Point table data.
- Simulated status, measurement, and control data.

After configuration, start listening and wait for master connections.

## 7. Language Switching

Starting from `v0.2.0`, IEC104 Simulator supports i18n.

You can switch the UI language from the settings or language menu.

Currently supported languages include:

- Simplified Chinese.
- English.

If some UI text is not refreshed immediately after switching the language, try restarting the application.

## 8. Uninstall the Application

You can uninstall the application from Windows Settings:

1. Open Windows Settings.
2. Go to Apps.
3. Find IEC104 Simulator Master or IEC104 Simulator Slave.
4. Click Uninstall.

You can also uninstall it from Control Panel > Programs and Features.

## 9. FAQ

### 9.1 The installer cannot be started

Make sure your system is Windows 10 x64 or Windows 11 x64.

If the installer is blocked by security software, make sure it was downloaded from this repository's Release page.

### 9.2 The application cannot connect to the device

Please check:

- Whether the target IP address is correct.
- Whether the target port is correct.
- Whether the firewall allows the connection.
- Whether the slave device is listening.
- Whether the IEC104 parameters are consistent with the peer device.

### 9.3 Slave Simulator cannot be connected by the master

Please check:

- Whether the local listening port is already occupied.
- Whether Windows Firewall allows the application to communicate.
- Whether the IP and port used by the master are correct.
- Whether Slave Simulator has started listening.

### 9.4 The UI language does not change

Please check:

- Whether the current version is `v0.2.0` or later.
- Whether the target language has been selected in language settings.
- Whether the application needs to be restarted after switching the language.