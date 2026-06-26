# IEC104 Simulator

[简体中文](./README.md) | [English](../en-US/README.md)

IEC104 Simulator 是一款面向 IEC 60870-5-104 协议测试、调试和验证场景的模拟器工具。

本仓库用于发布 IEC104 Simulator 的公开安装包、版本说明和使用文档。

![IEC104 Simulator 主站首页](../../assets/screenshots/master-home-linked.png)

## 功能概览

IEC104 Simulator 当前提供 Master 模拟器和 Slave 模拟器两个独立程序，可用于主站、从站、规约开发、设备联调、功能验证和报文分析等场景。

### Master 模拟器

- 支持连接 IEC104 从站设备。
- 支持发送总召、控制、设点、时钟同步等命令。
- 支持查看通信状态、收发报文和解析结果。
- 支持对从站数据进行调试和验证。

### Slave 模拟器

- 支持监听 IEC104 主站连接。
- 支持响应总召、控制等主站命令。
- 支持导入点表并模拟遥信、遥测、遥控等数据。
- 支持模拟数据变化和事件上送。

### 通用功能

- 支持实时报文监控。
- 支持报文结构化解析。
- 支持 SOE 事件记录。
- 支持 PCAP/PCAPNG 报文导出。
- 支持本地数据持久化。
- 支持 i18n 国际化和界面语言切换。

## 软件截图

### 主站首页

![主站首页](../../assets/screenshots/master-home-linked.png)

### 从站首页

![从站首页](../../assets/screenshots/slave-home-linked.png)

### 主站报文监控

![主站报文监控](../../assets/screenshots/master-message-panel.png)

### 从站数据模拟

![从站数据模拟](../../assets/screenshots/slave-data-simulated.png)

## 下载

请前往 Releases 页面下载最新版本：

- [Releases](https://github.com/Gululu8023/iec104-simulator/releases)

常见安装包如下：

- `iec104-simulator-master_x.x.x_x64-setup.exe`：IEC104 Master 模拟器安装包。
- `iec104-simulator-slave_x.x.x_x64-setup.exe`：IEC104 Slave 模拟器安装包。

如果 Release 同时提供 Master 和 Slave 安装包，请根据实际测试场景选择下载。

## 如何选择安装包

- 如果需要模拟主站，请下载 `master` 安装包。
- 如果需要模拟从站，请下载 `slave` 安装包。
- 如果需要在本机进行主从联调，可以同时安装 Master 和 Slave。

## 文档

- [安装说明](./install.md)
- [使用说明](./user-guide.md)
- [版本变更记录](./changelog.md)

## 系统要求

当前公开发布版本主要支持：

- Windows 10 x64
- Windows 11 x64

如后续提供 Linux 或 macOS 版本，将在 Release 页面和文档中另行说明。

## 版本说明

本仓库仅用于发布公开安装包和用户文档。

实际版本内容以各个 GitHub Release 页面为准。