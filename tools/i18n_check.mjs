#!/usr/bin/env node
// nfr_10_sv_en_localisation_complete: every UI string has Swedish and
// English, and no Svelte file shows literal text that bypasses t().
// Run: node tools/i18n_check.mjs  (exit 1 on any finding)
import { readFileSync, readdirSync } from 'node:fs'
import { join, dirname } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = join(dirname(fileURLToPath(import.meta.url)), '..', 'app', 'ui', 'src')
const findings = []

// 1. Both dictionaries have exactly the same keys, and no value is empty.
const i18n = readFileSync(join(root, 'lib', 'i18n.svelte.ts'), 'utf8')
function dict(name) {
  const m = i18n.match(new RegExp(`const ${name}[^{]*\\{([\\s\\S]*?)\\n\\}`))
  if (!m) { findings.push(`i18n: dictionary ${name} not found`); return new Map() }
  const out = new Map()
  for (const k of m[1].matchAll(/^\s*'([^']+)':\s*(['"`])((?:\\.|(?!\2).)*)\2/gm)) out.set(k[1], k[3])
  return out
}
const sv = dict('sv'), en = dict('en')
for (const k of sv.keys()) if (!en.has(k)) findings.push(`i18n: '${k}' has no English`)
for (const k of en.keys()) if (!sv.has(k)) findings.push(`i18n: '${k}' has no Swedish`)
for (const [n, d] of [['sv', sv], ['en', en]]) for (const [k, v] of d) if (!v.trim()) findings.push(`i18n: '${k}' is empty in ${n}`)

// 2. Every t('key') used in the UI exists.
function files(dir) {
  return readdirSync(dir, { withFileTypes: true }).flatMap((e) =>
    e.isDirectory() ? files(join(dir, e.name)) : /\.(svelte|ts)$/.test(e.name) ? [join(dir, e.name)] : [])
}
const all = files(root)
// Words allowed as literal text: names, symbols and units, not language.
const ALLOW = new Set(['Cantor', 'Lacrymosa', 'MusicXML', 'PDF', 'MIDI', 'MXL', 'forScore', 'AGPL-3.0', 'GitHub', 'BPM', 'bpm', 'A4', 'Hz'])
const hasWord = (s) => s.split(/[^\p{L}][^\p{L}]*/u).some((w) => w.length > 1 && !ALLOW.has(w) && !/^[A-Z]$/.test(w))

for (const f of all) {
  const src = readFileSync(f, 'utf8')
  const rel = f.slice(root.length + 1)
  for (const m of src.matchAll(/\bt\(\s*'([^']+)'/g)) if (!sv.has(m[1])) findings.push(`${rel}: t('${m[1]}') is not in the dictionary`)
  if (f.endsWith('i18n.svelte.ts')) continue
  const line = (i) => src.slice(0, i).split('\n').length
  // Dialog helpers and alerts with literal text, in any file.
  for (const m of src.matchAll(/\b(alert|ask|confirmBox|toast|notify)\(\s*(['"`])((?:\\.|(?!\2).)*)\2/g))
    if (hasWord(m[3])) findings.push(`${rel}:${line(m.index)}: ${m[1]}() with literal "${m[3]}"`)
  if (!f.endsWith('.svelte')) continue
  // Markup only: drop <script> and <style>, blank them to keep line numbers.
  const blank = (s) => s.replace(/[^\n]/g, ' ')
  let mk = src.replace(/<(script|style)\b[\s\S]*?<\/\1>/g, blank)
  mk = mk.replace(/<!--[\s\S]*?-->/g, blank)
  // Literal user-visible attributes.
  for (const m of mk.matchAll(/\s(aria-label|title|placeholder|alt|aria-description|aria-roledescription|label)="([^"{]*)"/g))
    if (hasWord(m[2])) findings.push(`${rel}:${line(m.index)}: ${m[1]}="${m[2]}"`)
  // Text nodes: strip {expressions} (balanced, one level of nesting is enough
  // in practice) and look between tags.
  let text = mk
  for (let i = 0; i < 4; i++) text = text.replace(/\{[^{}]*\}/g, blank)
  for (const m of text.matchAll(/>([^<>]+)</g))
    if (hasWord(m[1])) findings.push(`${rel}:${line(m.index)}: text "${m[1].trim().slice(0, 60)}"`)
}

if (findings.length) {
  console.error(`nfr_10_sv_en_localisation_complete: ${findings.length} finding(s)`)
  for (const x of findings) console.error('  ' + x)
  process.exit(1)
}
console.log(`nfr_10_sv_en_localisation_complete: ok (${sv.size} keys, ${all.length} files)`)
