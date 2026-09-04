# 变更记录

本项目采用面向用户的变更记录格式。应用产品版本与 `iec60870-parser` 组件版本独立管理。

## [Unreleased] - v1.0.0

### Added

- 首次公开完整 Rust、Tauri 和 Vue 源码，提供独立 Master/Slave 应用。
- 增加站总召、1–16 组召、电度召唤及冻结/复位操作说明。
- 增加控制、设点、位串、读、测试、时钟同步、复位和文件传输的完整界面流程。
- 增加从站 SBO/直接执行、命令映射、SQ=1、SOE、15 种波形、强制上送和雪崩测试。
- 增加 CSV/JSON/XML 点表预检与导入、树形报文解析和 CSV/TXT/PCAP/PCAPNG 报文导出。
- 增加中英文 Help、协议支持矩阵、已知限制、安装升级和开发文档。

### Changed

- 重构为 Master/Slave 双应用与共享 backend/parser 的 Cargo workspace。
- 网络接收采用有界可靠背压，协议链路故障统一进入自动重连状态机。
- 热点 SQLite 写入改为异步线程外批量事务。
- 项目许可证由 MIT 切换为 Apache License 2.0。
- 公开仓库调整为源码、文档和安装包一体化仓库。

### Fixed

- 修复发送窗口在 ACK 竞态下永久等待的风险。
- 修复断链、STARTDT/TESTFR 超时和 t1 耗尽恢复路径不一致的问题。
- 修复后台连接、监控和模拟任务未完整停止与回收的问题。
- 修复映射控制结果重复持久化及协议循环被同步数据库写阻塞的问题。
- 修复并发建立相同目标连接的准入竞态。
- 修复前端运行时同步循环启动/停止竞态。

### Compatibility

- 产品版本为 1.0.0；内部 `iec60870-parser` 组件保持 0.1.0。
- v1 数据库只接受 schema version 1，不自动迁移 pre-v1 数据库。升级前必须备份；不兼容时应重命名旧库后让 v1 创建新库。
- 官方安装与验证范围为 Windows 10/11 x64。

### Known Issues

- 参数类和安全扩展 ASDU 仅有部分解析能力，不提供完整主站/从站业务流程。
- 点表不支持导出，SOE 面板不提供独立 CSV 导出。
- 文件传输为单节模式，大小受最大 ASDU 配置限制。

## [0.2.0] - 2026-06-26

### Added

- 增加简体中文和 English 界面资源及语言切换。

### Changed

- 统一主要菜单、按钮、状态和错误文案的本地化管理。

## [0.1.0] - 2026-04-13

### Added

- 建立公开发布仓库及 Master/Slave Windows 安装包发布结构。
- 提供基础连接、监听、点表导入、报文查看与协议联调能力。

[Unreleased]: https://github.com/Gululu8023/iec104-simulator/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/Gululu8023/iec104-simulator/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/Gululu8023/iec104-simulator/releases/tag/v0.1.0
