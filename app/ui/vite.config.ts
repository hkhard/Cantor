import { svelte } from '@sveltejs/vite-plugin-svelte'
import { defineConfig, type Plugin } from 'vite'
import { cpSync, existsSync, readFileSync } from 'node:fs'
import { execSync } from 'node:child_process'
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

// Release identity (ROADMAP phase P / AGPL 6(d)): the exact source of this
// build. `git describe` gives a tag such as v0.2.0, or tag-N-gSHA, or a bare
// SHA; "-dirty" marks uncommitted changes. CANTOR_VERSION overrides it.
function gitVersion(): { version: string; ref: string } {
  const env = process.env.CANTOR_VERSION
  let d = env ?? ''
  if (!d) {
    try {
      d = execSync('git describe --tags --always --dirty', { cwd: __dirname }).toString().trim()
    } catch {
      d = 'dev'
    }
  }
  // Link to the tag when the build is exactly a tag, else to the commit.
  const m = /-g([0-9a-f]{7,})(-dirty)?$/.exec(d)
  const ref = m ? m[1] : d.replace(/-dirty$/, '')
  return { version: d, ref }
}
const V = gitVersion()

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
  define: {
    __CANTOR_VERSION__: JSON.stringify(V.version),
    __CANTOR_REF__: JSON.stringify(V.ref),
  },
  build: { target: 'safari16', chunkSizeWarningLimit: 9000 },
})
