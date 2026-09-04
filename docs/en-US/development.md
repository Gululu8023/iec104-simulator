# Development

## Windows prerequisites

- Windows 10/11 x64
- Node.js 20 LTS
- pnpm 9.15
- Rust 1.88 or newer
- Microsoft Edge WebView2
- Microsoft C++ Build Tools with the “Desktop development with C++” workload

```powershell
corepack enable
pnpm install --frozen-lockfile
pnpm tauri:dev:master
pnpm tauri:dev:slave
```

Static checks:

```powershell
pnpm verify:i18n
pnpm verify:release
pnpm exec vue-tsc -p frontend/master/tsconfig.json --noEmit
pnpm exec vue-tsc -p frontend/slave/tsconfig.json --noEmit
cargo check --workspace --manifest-path src-tauri/Cargo.toml
```

The product version is shared by the npm package, Cargo workspace, backend, Master/Slave crates, and both Tauri configurations. `iec60870-parser` keeps an independent component version of 0.1.0; changing it does not change the application version automatically.

All workspace crates inherit the Apache-2.0 license and public repository metadata. They are not published independently to crates.io.
