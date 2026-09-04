# 源码构建与开发

## Windows 环境

- Windows 10/11 x64
- Node.js 20 LTS
- pnpm 9.15（建议通过 Corepack）
- Rust 1.88 或更高版本
- Microsoft Edge WebView2
- Visual Studio Build Tools，“使用 C++ 的桌面开发”工作负载

## 安装依赖

```powershell
corepack enable
pnpm install --frozen-lockfile
```

## 开发运行

```powershell
pnpm tauri:dev:master
pnpm tauri:dev:slave
```

Master 和 Slave 使用不同的开发端口，可以分别启动。前端独立调试命令为 `pnpm dev:master` 和 `pnpm dev:slave`。

## 静态检查

```powershell
pnpm verify:i18n
pnpm verify:release
pnpm exec vue-tsc -p frontend/master/tsconfig.json --noEmit
pnpm exec vue-tsc -p frontend/slave/tsconfig.json --noEmit
cargo check --workspace --manifest-path src-tauri/Cargo.toml
```

## 版本规则

应用产品版本由根 `package.json`、Cargo workspace、Master/Slave Tauri 配置共同声明，发布前必须一致。`iec60870-parser` 是内部组件，独立保持 0.1.0，不作为应用版本展示，也不单独发布到 crates.io。

所有 workspace crate 依据 Apache License 2.0 发布，仓库地址统一指向公开项目。
