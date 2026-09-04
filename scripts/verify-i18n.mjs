import { existsSync, readFileSync, readdirSync, statSync } from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import ts from 'typescript'

const rootDir = path.resolve(fileURLToPath(new URL('..', import.meta.url)))
const issues = []

const localeFiles = {
  'zh-CN': 'frontend/shared/i18n/locales/zh-CN',
  'en-US': 'frontend/shared/i18n/locales/en-US',
}
const localeDomains = [
  'common',
  'errors',
  'settings',
  'message',
  'protocol',
  'master',
  'slave',
  'file-transfer',
]

const generatedOrLowValueDirs = new Set([
  '.git',
  '.next',
  '.nuxt',
  '.turbo',
  '.cache',
  'build',
  'coverage',
  'dist',
  'node_modules',
  'target',
  'temp',
  'tmp',
  'vendor',
])

const hardcodedChineseScanRoots = [
  'frontend/master/src',
  'frontend/slave/src',
  'frontend/shared',
]

const hardcodedChineseAllowPathFragments = [
  'frontend/shared/i18n/locales/',
  'frontend/shared/content/helpDocs.ts',
  'frontend/shared/api/iec104/catalog.ts',
  'frontend/shared/api/iec104/display.ts',
  'frontend/shared/api/iec104/quality.ts',
  'frontend/shared/api/masterControl.ts',
  'frontend/shared/api/protocolQuality.ts',
  'frontend/shared/ui/connectionStatus.ts',
  'frontend/shared/ui/dataTypeIcon.ts',
  'frontend/shared/components/message-panel/types.ts',
]

const hardcodedChineseAllowLineRules = [
  {
    file: 'frontend/master/src/components/MasterFileTransferDialog.vue',
    pattern: /\/.*(顺序事件|事件顺序|遥信变位|变位|录波|扰动|故障|模拟量|曲线|历史)/u,
  },
  {
    file: 'frontend/shared/components/message-panel/useMessagePanelExport.ts',
    pattern: /const headers = \['时间', '方向', '从站', '描述', '原始内容'\]/u,
  },
]

const enHanAllowKeys = new Set([
  'language.zhCN',
])

const dynamicBackendErrorCodes = new Set([
  'FILE_TRANSFER_BUSY',
  'FILE_TOO_LARGE',
  'PROTOCOL_STATE',
  'UPLOAD_FILE_MISSING',
  'VALIDATION_ERROR',
])

function fail(scope, message) {
  issues.push(`${scope}: ${message}`)
}

function readText(relativePath) {
  return readFileSync(path.join(rootDir, relativePath), 'utf8')
}

function toPosix(relativePath) {
  return relativePath.split(path.sep).join('/')
}

function collectFiles(relativeDir, extensions) {
  const absoluteDir = path.join(rootDir, relativeDir)
  if (!existsSync(absoluteDir)) return []

  const files = []
  const visit = (absolutePath) => {
    const name = path.basename(absolutePath)
    if (generatedOrLowValueDirs.has(name)) return

    const stat = statSync(absolutePath)
    if (stat.isDirectory()) {
      for (const child of readdirSync(absolutePath)) {
        visit(path.join(absolutePath, child))
      }
      return
    }

    if (!extensions.some((extension) => absolutePath.endsWith(extension))) return
    files.push(toPosix(path.relative(rootDir, absolutePath)))
  }

  visit(absoluteDir)
  return files
}

function parseLocaleFile(relativePath) {
  const source = readText(relativePath)
  const sourceFile = ts.createSourceFile(
    relativePath,
    source,
    ts.ScriptTarget.Latest,
    true,
    ts.ScriptKind.TS,
  )
  const declaration = sourceFile.statements
    .filter(ts.isVariableStatement)
    .flatMap((statement) => [...statement.declarationList.declarations])
    .find((item) => item.initializer && ts.isObjectLiteralExpression(item.initializer))
  if (!declaration?.initializer || !ts.isObjectLiteralExpression(declaration.initializer)) {
    fail(relativePath, 'cannot find locale object export')
    return {}
  }

  const readPropertyName = (name) => {
    if (ts.isIdentifier(name) || ts.isStringLiteral(name) || ts.isNumericLiteral(name)) {
      return name.text
    }
    fail(relativePath, `unsupported computed locale key at ${sourceFile.getLineAndCharacterOfPosition(name.pos).line + 1}`)
    return null
  }

  const readValue = (node, keyPath) => {
    if (ts.isObjectLiteralExpression(node)) return readObject(node, keyPath)
    if (ts.isStringLiteral(node) || ts.isNoSubstitutionTemplateLiteral(node)) return node.text
    if (ts.isNumericLiteral(node)) return Number(node.text)
    if (node.kind === ts.SyntaxKind.TrueKeyword) return true
    if (node.kind === ts.SyntaxKind.FalseKeyword) return false
    if (node.kind === ts.SyntaxKind.NullKeyword) return null
    fail(relativePath, `unsupported value for ${keyPath}`)
    return undefined
  }

  const readObject = (node, prefix = '') => {
    const result = Object.create(null)
    for (const property of node.properties) {
      if (!ts.isPropertyAssignment(property)) {
        fail(relativePath, `unsupported locale property at ${sourceFile.getLineAndCharacterOfPosition(property.pos).line + 1}`)
        continue
      }
      const key = readPropertyName(property.name)
      if (key == null) continue
      const keyPath = prefix ? `${prefix}.${key}` : key
      if (Object.prototype.hasOwnProperty.call(result, key)) {
        fail(relativePath, `duplicate locale key ${keyPath}`)
        continue
      }
      result[key] = readValue(property.initializer, keyPath)
    }
    return result
  }

  return readObject(declaration.initializer)
}

function mergeLocaleObject(target, source, scope, prefix = '') {
  for (const [key, value] of Object.entries(source)) {
    const keyPath = prefix ? `${prefix}.${key}` : key
    if (
      value &&
      typeof value === 'object' &&
      !Array.isArray(value) &&
      target[key] &&
      typeof target[key] === 'object' &&
      !Array.isArray(target[key])
    ) {
      mergeLocaleObject(target[key], value, scope, keyPath)
    } else if (Object.prototype.hasOwnProperty.call(target, key)) {
      fail(scope, `duplicate locale key ${keyPath}`)
    } else {
      target[key] = value
    }
  }
  return target
}

function parseLocaleObject(relativeDir) {
  return localeDomains.reduce(
    (locale, domain) =>
      mergeLocaleObject(
        locale,
        parseLocaleFile(`${relativeDir}/${domain}.ts`),
        relativeDir,
      ),
    {},
  )
}

function flattenLocale(value, prefix = '', out = new Map()) {
  if (value && typeof value === 'object' && !Array.isArray(value)) {
    for (const [key, child] of Object.entries(value)) {
      flattenLocale(child, prefix ? `${prefix}.${key}` : key, out)
    }
    return out
  }

  out.set(prefix, value)
  return out
}

function hasHan(value) {
  return /\p{Script=Han}/u.test(value)
}

function compareLocaleKeys(zhFlat, enFlat) {
  const zhKeys = [...zhFlat.keys()].sort()
  const enKeys = [...enFlat.keys()].sort()

  for (const key of zhKeys) {
    if (!enFlat.has(key)) fail('locale parity', `en-US is missing key ${key}`)
  }
  for (const key of enKeys) {
    if (!zhFlat.has(key)) fail('locale parity', `zh-CN is missing key ${key}`)
  }
}

function checkLocaleValues(zhFlat, enFlat) {
  const placeholderPattern = /\b(TODO|TBD|PLACEHOLDER)\b|待翻译|占位|翻译/u

  for (const [key, zhValue] of zhFlat) {
    if (typeof zhValue !== 'string') continue
    if (zhValue.trim() === '') {
      fail('zh-CN locale', `${key} is empty`)
    }
    if (zhValue.includes('@')) {
      fail('zh-CN locale', `${key} contains raw @; pass it as an interpolation value`)
    }
  }

  for (const [key, enValue] of enFlat) {
    if (typeof enValue !== 'string') continue
    if (enValue.trim() === '') {
      fail('en-US locale', `${key} is empty`)
    }
    if (placeholderPattern.test(enValue)) {
      fail('en-US locale', `${key} looks like placeholder text`)
    }
    if (!enHanAllowKeys.has(key) && hasHan(enValue)) {
      fail('en-US locale', `${key} contains Chinese text: ${enValue}`)
    }
    if (enValue.includes('@')) {
      fail('en-US locale', `${key} contains raw @; pass it as an interpolation value`)
    }
  }
}

function getNestedValue(root, dottedKey) {
  return dottedKey.split('.').reduce((current, part) => {
    if (!current || typeof current !== 'object') return undefined
    return current[part]
  }, root)
}

function compareObjectKeySet(scope, zhObject, enObject) {
  const zhKeys = zhObject && typeof zhObject === 'object' ? Object.keys(zhObject).sort() : []
  const enKeys = enObject && typeof enObject === 'object' ? Object.keys(enObject).sort() : []

  for (const key of zhKeys) {
    if (!enKeys.includes(key)) fail(scope, `en-US is missing protocol key ${key}`)
  }
  for (const key of enKeys) {
    if (!zhKeys.includes(key)) fail(scope, `zh-CN is missing protocol key ${key}`)
  }
}

function checkProtocolIdentifierKeys(zhLocale, enLocale) {
  const groups = [
    'iec104.types.name',
    'iec104.types.short',
  ]
  const typeNamePattern = /^[A-Z]_[A-Z]{2}_[A-Z]{2}_[0-9]$/

  for (const group of groups) {
    const zhGroup = getNestedValue(zhLocale, group)
    const enGroup = getNestedValue(enLocale, group)
    compareObjectKeySet(group, zhGroup, enGroup)

    for (const key of Object.keys(zhGroup ?? {})) {
      if (!typeNamePattern.test(key)) {
        fail(group, `unexpected IEC104 type key ${key}`)
      }
    }
  }
}

function parseApiErrorTranslationKeys() {
  const relativePath = 'frontend/shared/i18n/errors.ts'
  const source = readText(relativePath)
  const match = source.match(/API_ERROR_TRANSLATION_KEYS[^=]*=\s*({[\s\S]*?})\r?\n\r?\n/)
  if (!match) {
    fail(relativePath, 'cannot find API_ERROR_TRANSLATION_KEYS object')
    return new Map()
  }

  const pairs = new Map()
  const pairPattern = /^\s*([A-Z0-9_]+):\s*['"]([^'"]+)['"],?\s*$/gm
  for (const pair of match[1].matchAll(pairPattern)) {
    pairs.set(pair[1], pair[2])
  }
  return pairs
}

function collectBackendApiErrorCodes() {
  const codes = new Set(dynamicBackendErrorCodes)
  const files = collectFiles('src-tauri/libs/backend/src/api', ['.rs'])
  const patterns = [
    /error_with_code(?:_and_params)?\(\s*"([A-Z0-9_]+)"/g,
    /Err\(\(\s*"([A-Z0-9_]+)"/g,
  ]

  for (const file of files) {
    const source = readText(file)
    for (const pattern of patterns) {
      for (const match of source.matchAll(pattern)) {
        codes.add(match[1])
      }
    }
  }

  return codes
}

function checkApiErrorCoverage(zhFlat, enFlat) {
  const translationKeys = parseApiErrorTranslationKeys()
  const backendCodes = collectBackendApiErrorCodes()

  for (const code of backendCodes) {
    if (!translationKeys.has(code)) {
      fail('API error coverage', `${code} is returned by backend but missing API_ERROR_TRANSLATION_KEYS`)
    }
  }

  for (const [code, translationKey] of translationKeys) {
    const expectedKey = `errors.${code}`
    if (translationKey !== expectedKey) {
      fail('API error coverage', `${code} should map to ${expectedKey}, got ${translationKey}`)
    }
    if (!zhFlat.has(translationKey)) {
      fail('API error coverage', `zh-CN is missing ${translationKey}`)
    }
    if (!enFlat.has(translationKey)) {
      fail('API error coverage', `en-US is missing ${translationKey}`)
    }
  }
}

function isCommentOnlyLine(line) {
  const trimmed = line.trim()
  return (
    trimmed === '' ||
    trimmed.startsWith('//') ||
    trimmed.startsWith('*') ||
    trimmed.startsWith('/*') ||
    trimmed.startsWith('*/') ||
    trimmed.startsWith('<!--') ||
    trimmed.startsWith('-->') ||
    trimmed.startsWith('/* ')
  )
}

function removeTrailingComment(line) {
  const markers = ['//', '/*', '<!--']
  let end = line.length
  for (const marker of markers) {
    const index = line.indexOf(marker)
    if (index >= 0 && index < end) end = index
  }
  return line.slice(0, end)
}

function isAllowedChinesePath(file) {
  return hardcodedChineseAllowPathFragments.some((fragment) => file.includes(fragment))
}

function isAllowedChineseLine(file, codePart) {
  return hardcodedChineseAllowLineRules.some(
    (rule) => file === rule.file && rule.pattern.test(codePart),
  )
}

function checkHardcodedChinese() {
  const sourceFiles = hardcodedChineseScanRoots.flatMap((scanRoot) =>
    collectFiles(scanRoot, ['.ts', '.vue']),
  )

  for (const file of sourceFiles) {
    if (isAllowedChinesePath(file)) continue
    const lines = readText(file).split(/\r?\n/)
    lines.forEach((line, index) => {
      if (!hasHan(line)) return
      if (isCommentOnlyLine(line)) return
      const codePart = removeTrailingComment(line)
      if (!hasHan(codePart)) return
      if (isAllowedChineseLine(file, codePart)) return
      fail('hardcoded Chinese', `${file}:${index + 1}: ${codePart.trim()}`)
    })
  }
}

function main() {
  const zhLocale = parseLocaleObject(localeFiles['zh-CN'])
  const enLocale = parseLocaleObject(localeFiles['en-US'])
  const zhFlat = flattenLocale(zhLocale)
  const enFlat = flattenLocale(enLocale)

  compareLocaleKeys(zhFlat, enFlat)
  checkLocaleValues(zhFlat, enFlat)
  checkProtocolIdentifierKeys(zhLocale, enLocale)
  checkApiErrorCoverage(zhFlat, enFlat)
  checkHardcodedChinese()

  if (issues.length > 0) {
    console.error(`i18n verification failed with ${issues.length} issue(s):`)
    for (const issue of issues) {
      console.error(`- ${issue}`)
    }
    process.exitCode = 1
    return
  }

  console.log('i18n verification passed')
}

main()
