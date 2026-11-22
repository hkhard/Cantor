import { svelte } from '@sveltejs/vite-plugin-svelte'
import { defineConfig } from 'vite'

const host = process.env.TAURI_DEV_HOST

export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: 'ws', host, port: 5183 } : undefined,
  },
  optimizeDeps: { exclude: ['verovio'] },
  build: { target: 'safari16', chunkSizeWarningLimit: 9000 },
})
