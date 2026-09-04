import vue from '@vitejs/plugin-vue'
import AutoImport from 'unplugin-auto-import/vite'
import Components from 'unplugin-vue-components/vite'
import { ElementPlusResolver } from 'unplugin-vue-components/resolvers'
import { resolve } from 'path'
import type { UserConfig } from 'vite'

interface AppViteConfigOptions {
  app: 'master' | 'slave'
  port: number
  hmrPort: number
  host?: string
}

export function createAppViteConfig(options: AppViteConfigOptions): UserConfig {
  const appRoot = resolve(__dirname, `frontend/${options.app}`)
  const sharedRoot = resolve(__dirname, 'frontend/shared')
  return {
    root: appRoot,
    cacheDir: resolve(__dirname, `node_modules/.vite-${options.app}`),
    plugins: [
      vue(),
      AutoImport({
        resolvers: [ElementPlusResolver()],
        imports: ['vue'],
        dts: resolve(appRoot, 'auto-imports.d.ts'),
      }),
      Components({ resolvers: [ElementPlusResolver()], dts: resolve(appRoot, 'components.d.ts') }),
    ],
    resolve: { alias: { '@': resolve(appRoot, 'src'), '@shared': sharedRoot } },
    clearScreen: false,
    server: {
      port: options.port,
      strictPort: true,
      host: options.host || false,
      hmr: options.host ? { protocol: 'ws', host: options.host, port: options.hmrPort } : undefined,
      watch: { ignored: ['**/src-tauri/**'] },
      fs: { allow: [appRoot, sharedRoot, resolve(__dirname, 'node_modules')] },
    },
    build: { outDir: resolve(__dirname, `dist/${options.app}`), emptyOutDir: true },
  }
}
