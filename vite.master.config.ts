import { defineConfig } from 'vite'
import { createAppViteConfig } from './vite.shared.config'

// @ts-expect-error process is a Node.js global provided by Vite.
const host = process.env.TAURI_DEV_HOST

export default defineConfig(createAppViteConfig({ app: 'master', port: 1420, hmrPort: 1421, host }))
