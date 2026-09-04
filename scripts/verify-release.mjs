import { createHash } from 'node:crypto'
import { existsSync, readFileSync, readdirSync } from 'node:fs'
import path from 'node:path'
import process from 'node:process'

const root = process.cwd()
const read = (relativePath) => readFileSync(path.join(root, relativePath), 'utf8')
const issues = []
const productRepository = 'https://github.com/Gululu8023/iec104-simulator'
const productLicense = 'Apache-2.0'
const parserVersion = '0.1.0'
const apacheLicenseSha256 = 'c71d239df91726fc519c6eb72d318ec65820627232b2f796219e87dcf35d0ab4'

function fail(message) {
  issues.push(message)
}

function expectMatch(source, pattern, message) {
  if (!pattern.test(source)) fail(message)
}

function normalizeNewlines(source) {
  return `${source.replace(/\r\n?/gu, '\n').trimEnd()}\n`
}

function listMarkdownFiles(relativeDirectory) {
  const directory = path.join(root, relativeDirectory)
  if (!existsSync(directory)) return []
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const relativePath = path.join(relativeDirectory, entry.name)
    if (entry.isDirectory()) return listMarkdownFiles(relativePath)
    return entry.isFile() && entry.name.endsWith('.md') ? [relativePath] : []
  })
}

function checkMarkdownLinks(relativePath) {
  const source = read(relativePath)
  for (const match of source.matchAll(/!?\[[^\]]*\]\(([^)]+)\)/gu)) {
    let target = match[1].trim()
    if (target.startsWith('<') && target.endsWith('>')) target = target.slice(1, -1)
    target = target.split(/\s+["']/u, 1)[0]
    if (!target || target.startsWith('#') || /^(?:https?:|mailto:)/iu.test(target)) continue
    const fileTarget = target.split('#', 1)[0]
    let decodedTarget = fileTarget
    try {
      decodedTarget = decodeURIComponent(fileTarget)
    } catch {
      fail(`${relativePath} contains an invalid encoded link: ${target}`)
      continue
    }
    const resolved = path.resolve(path.dirname(path.join(root, relativePath)), decodedTarget)
    if (!existsSync(resolved)) fail(`${relativePath} contains a broken link: ${target}`)
  }
}

function checkWorkspaceDependencies(relativePath, source) {
  let dependencySection = false
  for (const rawLine of source.split(/\r?\n/u)) {
    const line = rawLine.trim()
    const section = line.match(/^\[([^\]]+)\]$/u)?.[1]
    if (section) {
      dependencySection = /^(?:build-|dev-)?dependencies$/u.test(section)
      continue
    }
    if (!dependencySection || !line || line.startsWith('#')) continue
    if (!/^[A-Za-z0-9_-]+(?:\.workspace)?\s*=/u.test(line)) continue
    if (!/\.workspace\s*=\s*true\b|\bworkspace\s*=\s*true\b/u.test(line)) {
      fail(`${relativePath} dependency must come from workspace: ${line}`)
    }
  }
}

function checkWorkspacePackageMetadata(relativePath, source) {
  for (const field of ['authors', 'edition', 'rust-version', 'license', 'repository']) {
    const escapedField = field.replace('-', '\\-')
    if (!new RegExp(`^${escapedField}\\.workspace\\s*=\\s*true`, 'm').test(source)) {
      fail(`${relativePath} must inherit workspace ${field}`)
    }
  }
}

function readTomlStringArray(source, key) {
  const body = source.match(new RegExp(`^${key}\\s*=\\s*\\[([\\s\\S]*?)\\]`, 'm'))?.[1]
  return Array.from(body?.matchAll(/"([^"]+)"/gu) ?? [], (match) => match[1])
}

const packageJson = JSON.parse(read('package.json'))
const packageVersion = packageJson.version
const workspaceManifest = read('src-tauri/Cargo.toml')
const workspaceVersion = workspaceManifest.match(/^version\s*=\s*"([^"]+)"/m)?.[1]

if (!workspaceVersion) fail('src-tauri/Cargo.toml [workspace.package] has no version')
if (packageVersion !== workspaceVersion) {
  fail(`version mismatch: package=${packageVersion}, workspace=${workspaceVersion ?? 'missing'}`)
}
expectMatch(workspaceManifest, /^resolver\s*=\s*"3"/m, 'Cargo workspace resolver must be 3')
expectMatch(workspaceManifest, /^authors\s*=\s*\[\s*"[^"]+"\s*\]/m, 'Cargo workspace authors are missing')
expectMatch(workspaceManifest, /^rust-version\s*=\s*"1\.88"/m, 'Cargo workspace rust-version must satisfy locked dependencies')
expectMatch(workspaceManifest, new RegExp(`^license\\s*=\\s*"${productLicense}"`, 'm'), 'Cargo workspace license is invalid')
expectMatch(
  workspaceManifest,
  new RegExp(`^repository\\s*=\\s*"${productRepository.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}"`, 'm'),
  'Cargo workspace repository is invalid',
)
if (/^\[package\]/m.test(workspaceManifest)) fail('src-tauri/Cargo.toml must be a virtual workspace')

if (packageJson.license !== productLicense) fail(`npm package license must be ${productLicense}`)
if (packageJson.author !== 'Gululu8023') fail('npm package author is invalid')
if (packageJson.repository?.url !== `${productRepository}.git`) fail('npm package repository is invalid')
if (packageJson.packageManager !== 'pnpm@9.15.0') fail('npm packageManager must be pnpm@9.15.0')

for (const role of ['master', 'slave']) {
  for (const command of ['dev', 'build']) {
    const scriptName = `tauri:${command}:${role}`
    const expected = `cd src-tauri/apps/${role} && tauri ${command}`
    if (packageJson.scripts?.[scriptName] !== expected) {
      fail(`${scriptName} must run the Tauri CLI from its application directory`)
    }
  }
}

const expectedWorkspaceMembers = ['apps/master', 'apps/slave', 'libs/backend', 'libs/iec60870-parser']
const workspaceMembers = readTomlStringArray(workspaceManifest, 'members')
if (workspaceMembers.length !== expectedWorkspaceMembers.length) fail('Cargo workspace member count is invalid')
for (const member of expectedWorkspaceMembers) {
  if (!workspaceMembers.includes(member)) {
    fail(`workspace member is missing: ${member}`)
  }
}
const defaultMembers = readTomlStringArray(workspaceManifest, 'default-members')
if (defaultMembers.length !== 2 || !defaultMembers.includes('apps/master') || !defaultMembers.includes('apps/slave')) {
  fail('Cargo workspace default-members must contain only the two applications')
}

const backendManifestPath = 'src-tauri/libs/backend/Cargo.toml'
const backendManifest = read(backendManifestPath)
const backendApi = read('src-tauri/libs/backend/src/api/mod.rs')
expectMatch(backendManifest, /^name\s*=\s*"iec104-simulator-backend"/m, 'backend package name is invalid')
checkWorkspacePackageMetadata(backendManifestPath, backendManifest)
expectMatch(backendManifest, /^crate-type\s*=\s*\["rlib"\]/m, 'backend must only expose an rlib')
if (/^\[\[bin\]\]/m.test(backendManifest)) fail('backend still defines a binary')
if (/^\[build-dependencies\]/m.test(backendManifest)) fail('backend still has build dependencies')

const apps = [
  {
    role: 'master',
    packageName: 'iec104-simulator-master',
    identifier: 'com.iec104-simulator.master',
    roleVariant: 'Master',
    forbiddenHandler: 'api::slave_invoke_handler()',
  },
  {
    role: 'slave',
    packageName: 'iec104-simulator-slave',
    identifier: 'com.iec104-simulator.slave',
    roleVariant: 'Slave',
    forbiddenHandler: 'api::master_invoke_handler()',
  },
]

const sharedCommandSets = []
const appManifests = []
for (const app of apps) {
  const base = `src-tauri/apps/${app.role}`
  const manifest = read(`${base}/Cargo.toml`)
  const config = JSON.parse(read(`${base}/tauri.conf.json`))
  const capability = JSON.parse(read(`${base}/capabilities/default.json`))
  const appLib = read(`${base}/src/lib.rs`)
  appManifests.push(manifest)

  expectMatch(manifest, new RegExp(`^name\\s*=\\s*"${app.packageName}"`, 'm'), `${app.role} package name is invalid`)
  expectMatch(manifest, /^version\.workspace\s*=\s*true/m, `${app.role} package must use the workspace version`)
  expectMatch(manifest, /^publish\s*=\s*false/m, `${app.role} package must not be published separately`)
  checkWorkspacePackageMetadata(`${base}/Cargo.toml`, manifest)
  expectMatch(manifest, new RegExp(`\\[\\[bin\\]\\][\\s\\S]*?^name\\s*=\\s*"${app.packageName}"`, 'm'), `${app.role} binary name is invalid`)
  expectMatch(manifest, /^iec104-simulator-backend\.workspace\s*=\s*true/m, `${app.role} does not use the workspace backend dependency`)

  if (config.version !== packageVersion) fail(`${app.role} Tauri version is ${config.version}, expected ${packageVersion}`)
  if (config.bundle?.licenseFile !== '../../../LICENSE') fail(`${app.role} bundle does not include the root LICENSE`)
  const bundledLicense = path.resolve(root, base, config.bundle?.licenseFile ?? '')
  if (!existsSync(bundledLicense)) fail(`${app.role} bundle licenseFile does not exist`)
  if (config.productName !== app.packageName) fail(`${app.role} productName is invalid`)
  if (config.mainBinaryName !== app.packageName) fail(`${app.role} mainBinaryName is invalid`)
  if (config.identifier !== app.identifier) fail(`${app.role} identifier is invalid`)
  if (config.build?.beforeDevCommand !== `pnpm --dir ../.. dev:${app.role}`) fail(`${app.role} beforeDevCommand is invalid`)
  if (config.build?.beforeBuildCommand !== `pnpm --dir ../.. build:${app.role}`) fail(`${app.role} beforeBuildCommand is invalid`)
  if (config.build?.frontendDist !== `../../../dist/${app.role}`) fail(`${app.role} frontendDist is invalid`)
  if (!config.app?.security?.capabilities?.includes('default')) fail(`${app.role} does not enable the default capability`)
  if (capability.identifier !== 'default') fail(`${app.role} capability identifier is invalid`)

  if (!appLib.includes(`setup_app(app, AppRole::${app.roleVariant})`)) {
    fail(`${app.role} does not pass its fixed role to the backend`)
  }
  if (!appLib.includes(`api::${app.role}_invoke_handler()`)) fail(`${app.role} does not register its IPC handler`)
  if (appLib.includes(app.forbiddenHandler)) fail(`${app.role} registers the other role's IPC handler`)

  const handler = backendApi.match(new RegExp(`pub fn ${app.role}_invoke_handler[\\s\\S]*?\\n\\}`, 'u'))?.[0]
  if (!handler) {
    fail(`backend ${app.role} IPC handler is missing`)
  } else {
    if (!handler.includes(`${app.role}::`)) fail(`backend ${app.role} handler has no role-specific commands`)
    const otherRole = app.role === 'master' ? 'slave' : 'master'
    if (handler.includes(`${otherRole}::`)) fail(`backend ${app.role} handler registers ${otherRole} commands`)
  }

  sharedCommandSets.push(
    [...(handler ?? '').matchAll(/(?:common_commands|message_parser|point_table_import)::[A-Za-z0-9_]+/gu)]
      .map(([command]) => command)
      .sort(),
  )
  checkWorkspaceDependencies(`${base}/Cargo.toml`, manifest)
}

if (sharedCommandSets[0].join('\n') !== sharedCommandSets[1].join('\n')) {
  fail('master and slave register different shared IPC command sets')
}

const parserManifest = read('src-tauri/libs/iec60870-parser/Cargo.toml')
checkWorkspacePackageMetadata('src-tauri/libs/iec60870-parser/Cargo.toml', parserManifest)
expectMatch(parserManifest, new RegExp(`^version\\s*=\\s*"${parserVersion}"`, 'm'), `parser version must remain ${parserVersion}`)
expectMatch(parserManifest, /^publish\s*=\s*false/m, 'parser package must not be published separately')
expectMatch(backendManifest, /^version\.workspace\s*=\s*true/m, 'backend package must use the workspace version')
expectMatch(backendManifest, /^publish\s*=\s*false/m, 'backend package must not be published separately')
for (const [relativePath, manifest] of [
  [backendManifestPath, backendManifest],
  ['src-tauri/libs/iec60870-parser/Cargo.toml', parserManifest],
]) {
  checkWorkspaceDependencies(relativePath, manifest)
}

for (const manifest of [workspaceManifest, backendManifest, parserManifest, ...appManifests]) {
  if (/\bgit\s*=/u.test(manifest)) fail('a Cargo manifest contains a Git dependency')
}

for (const stalePath of [
  'Cargo.toml',
  'scripts/tauri.mjs',
  'src-tauri/build.rs',
  'src-tauri/tauri.conf.json',
  'src-tauri/tauri.master.conf.json',
  'src-tauri/tauri.slave.conf.json',
  'src-tauri/capabilities/default.json',
  'src-tauri/src',
  'vite.config.ts',
]) {
  if (existsSync(path.join(root, stalePath))) fail(`stale file remains: ${stalePath}`)
}

if (!existsSync(path.join(root, 'src-tauri/Cargo.lock'))) {
  fail('src-tauri/Cargo.lock is missing')
}
if (existsSync(path.join(root, 'Cargo.lock')) || existsSync(path.join(root, 'src-tauri/libs/iec60870-parser/Cargo.lock'))) {
  fail('Cargo.lock exists outside the src-tauri workspace root')
}

const license = normalizeNewlines(read('LICENSE'))
const licenseDigest = createHash('sha256').update(license, 'utf8').digest('hex')
if (licenseDigest !== apacheLicenseSha256) fail('LICENSE does not match the approved Apache License 2.0 text')
for (let section = 1; section <= 9; section += 1) {
  expectMatch(license, new RegExp(`^\\s*${section}\\.\\s`, 'm'), `LICENSE section ${section} is missing`)
}
expectMatch(license, /^\s*END OF TERMS AND CONDITIONS\s*$/m, 'LICENSE terms ending is missing')
expectMatch(license, /^\s*APPENDIX: How to apply the Apache License to your work\.\s*$/m, 'LICENSE appendix is missing')
expectMatch(license, /Copyright \[yyyy\] \[name of copyright owner\]/u, 'LICENSE official appendix placeholders changed')

const notice = normalizeNewlines(read('NOTICE'))
if (notice !== 'IEC104 Simulator\nCopyright 2025-2026 Gululu8023\n') fail('NOTICE project attribution is invalid')
expectMatch(read('README.md'), /Copyright 2025-2026 Gululu8023/u, 'README copyright does not match NOTICE')
const copyrightSource = read('frontend/shared/i18n/copyright.ts')
expectMatch(copyrightSource, /COPYRIGHT_START_YEAR\s*=\s*2025/u, 'About copyright start year does not match NOTICE')
expectMatch(copyrightSource, /COPYRIGHT_AUTHOR\s*=\s*'Gululu8023'/u, 'About copyright author does not match NOTICE')

const appInfoSource = read('src-tauri/libs/backend/src/api/common_commands.rs')
for (const variable of ['CARGO_PKG_VERSION', 'CARGO_PKG_LICENSE', 'CARGO_PKG_REPOSITORY']) {
  expectMatch(appInfoSource, new RegExp(`env!\\("${variable}"\\)`), `AppInfo must use ${variable}`)
}
for (const field of ['license', 'repository_url', 'issues_url', 'docs_url']) {
  expectMatch(appInfoSource, new RegExp(`\\b${field}:`), `AppInfo field is missing: ${field}`)
}
const aboutSource = read('frontend/shared/components/AboutDialog.vue')
for (const field of ['version', 'license', 'repository_url', 'issues_url', 'docs_url']) {
  expectMatch(aboutSource, new RegExp(`appInfo(?:\\.value)?\\.${field}`), `About does not consume AppInfo.${field}`)
}

const requiredDocs = [
  'user-guide.md',
  'point-table-format.md',
  'protocol-support.md',
  'development.md',
]
for (const locale of ['zh-CN', 'en-US']) {
  for (const document of requiredDocs) {
    if (!existsSync(path.join(root, 'docs', locale, document))) fail(`missing ${locale} document: ${document}`)
  }
}
for (const locale of ['zh-CN', 'en-US']) {
  for (const staleDocument of [
    'installation.md',
    'quick-start.md',
    'master-guide.md',
    'slave-guide.md',
    'known-limitations.md',
    'troubleshooting.md',
  ]) {
    if (existsSync(path.join(root, 'docs', locale, staleDocument))) {
      fail(`stale split document remains: docs/${locale}/${staleDocument}`)
    }
  }
}
for (const requiredFile of [
  'README.md',
  'CHANGELOG.md',
  'CHANGELOG.en.md',
  'CONTRIBUTING.md',
  'SECURITY.md',
  'THIRD_PARTY_NOTICES.md',
  'docs/releases/v1.0.0.md',
]) {
  if (!existsSync(path.join(root, requiredFile))) fail(`release file is missing: ${requiredFile}`)
}

for (const requiredScreenshot of [
  'assets/screenshots/english-ui.png',
  'assets/screenshots/file-transfer.png',
  'assets/screenshots/master-control-sbo.png',
  'assets/screenshots/master-counter-interrogation.png',
  'assets/screenshots/master-link-settings.png',
  'assets/screenshots/master-overview.png',
  'assets/screenshots/message-monitor.png',
  'assets/screenshots/message-tree.png',
  'assets/screenshots/point-table-import-preview.png',
  'assets/screenshots/slave-command-mapping.png',
  'assets/screenshots/slave-simulation-running.png',
  'assets/screenshots/slave-simulation.png',
  'assets/screenshots/soe-panel.png',
]) {
  if (!existsSync(path.join(root, requiredScreenshot))) {
    fail(`release screenshot is missing: ${requiredScreenshot}`)
  }
}

const publicMarkdownFiles = [
  'README.md',
  'CHANGELOG.md',
  'CHANGELOG.en.md',
  'CONTRIBUTING.md',
  'SECURITY.md',
  'THIRD_PARTY_NOTICES.md',
  ...listMarkdownFiles('docs'),
]
for (const relativePath of publicMarkdownFiles) checkMarkdownLinks(relativePath)

const changelog = read('CHANGELOG.md')
const englishChangelog = read('CHANGELOG.en.md')
expectMatch(changelog, new RegExp(`^## \\[Unreleased\\] - v${packageVersion.replaceAll('.', '\\.')}\\s*$`, 'm'), 'Chinese changelog product version is invalid')
expectMatch(englishChangelog, new RegExp(`^## \\[Unreleased\\] - v${packageVersion.replaceAll('.', '\\.')}\\s*$`, 'm'), 'English changelog product version is invalid')

const publicClaims = [read('README.md'), read('docs/en-US/README.md'), ...listMarkdownFiles('docs').map(read)].join('\n')
for (const forbidden of ['YourCompany', 'iec104-simulator-src', '完整实现 IEC104', 'supports all IEC104']) {
  if (publicClaims.includes(forbidden)) fail(`public documentation contains forbidden stale claim: ${forbidden}`)
}

if (issues.length) {
  for (const issue of issues) console.error(`[release] ${issue}`)
  process.exitCode = 1
} else {
  console.log(`release verification passed (${packageVersion}, master + slave)`)
}
