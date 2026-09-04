# Contributing

Thank you for contributing to IEC104 Simulator.

## Before opening an issue

- Search existing issues.
- Include the application version, Windows version, Master or Slave role, and reproducible steps.
- Remove credentials, private addresses, point names, file contents, and other sensitive information from logs and captures.
- Report security issues privately as described in [SECURITY.md](./SECURITY.md).

## Development

Follow [the development guide](./docs/en-US/development.md). Keep changes focused, preserve the Master/Slave boundary, and do not change IEC104 wire behavior without documenting the affected type IDs and interoperability impact.

Before submitting a pull request, run:

```powershell
pnpm verify:i18n
pnpm verify:release
pnpm exec vue-tsc -p frontend/master/tsconfig.json --noEmit
pnpm exec vue-tsc -p frontend/slave/tsconfig.json --noEmit
cargo check --workspace --manifest-path src-tauri/Cargo.toml
```

Do not include generated bundles, databases, logs, packet captures containing private traffic, or dependency caches.

By submitting a contribution, you agree that it is licensed under the Apache License 2.0 as described in [LICENSE](./LICENSE).
