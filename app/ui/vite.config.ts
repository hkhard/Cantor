import { svelte } from '@sveltejs/vite-plugin-svelte'
import { defineConfig, type Plugin } from 'vite'
import { cpSync, existsSync, readFileSync } from 'node:fs'
import { resolve } from 'node:path'

// pdf.js needs its wasm image decoders (JBIG2 / JPEG 2000: most scanned
// sheet music), standard fonts and CMaps at runtime. Serve them from
// /pdfjs/ in dev and copy them into the build.
const PDFJS = resolve(__dirname, 'node_modules/pdfjs-dist')
const PDFJS_DIRS = ['wasm', 'standard_fonts', 'cmaps', 'iccs']
function pdfjsAssets(): Plugin {
  return {
    name: 'pdfjs-assets',
    configureServer(server) {
      for (const d of PDFJS_DIRS) {
        server.middlewares.use(`/pdfjs/${d}/`, (req, res, next) => {
          const f = resolve(PDFJS, d, decodeURIComponent((req.url ?? '').split('?')[0].replace(/^\//, '')))
          if (!f.startsWith(resolve(PDFJS, d)) || !existsSync(f)) return next()
          if (f.endsWith('.wasm')) res.setHeader('Content-Type', 'application/wasm')
          res.end(readFileSync(f))
        })
      }
    },
    writeBundle(opts) {
      const out = opts.dir ?? resolve(__dirname, 'dist')
      for (const d of PDFJS_DIRS) {
        if (existsSync(resolve(PDFJS, d))) cpSync(resolve(PDFJS, d), resolve(out, 'pdfjs', d), { recursive: true })
      }
    },
  }
}

const host = process.env.TAURI_DEV_HOST

export default defineConfig({
  plugins: [svelte(), pdfjsAssets()],
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
