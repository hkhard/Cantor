// SET-03: "markdown-light" for instructions. Paragraphs are separated by a
// blank line, single newlines are line breaks. Inline: **bold**,
// [text](https://…), bare https:// links, and bar references such as
// "t. 12", "m. 12", "takt 12", "bar 12" that jump to that bar. Output is a
// token tree rendered by Instructions.svelte with plain elements (never
// {@html}), so a received set cannot inject markup.

export type Span =
  | { kind: 'text'; text: string }
  | { kind: 'bold'; children: Span[] }
  | { kind: 'link'; text: string; href: string }
  | { kind: 'bar'; text: string; bar: number }
  | { kind: 'br' }

export type Paragraph = Span[]

const SAFE_URL = /^(https?:\/\/|mailto:)[^\s<>"']+$/i
// [text](url) | bare url | bar reference
const INLINE =
  /\[([^\]\n]+)\]\(([^)\s]+)\)|((?:https?:\/\/|mailto:)[^\s<>"')\]]+[^\s<>"')\].,;:!?])|\b((?:t|m|mm)\.\s?|(?:takt|bar|measure)\s)(\d{1,4})\b/gi

function inline(src: string): Span[] {
  const out: Span[] = []
  let last = 0
  for (const m of src.matchAll(INLINE)) {
    const at = m.index ?? 0
    if (at > last) out.push({ kind: 'text', text: src.slice(last, at) })
    if (m[1] !== undefined) {
      if (SAFE_URL.test(m[2])) out.push({ kind: 'link', text: m[1], href: m[2] })
      else out.push({ kind: 'text', text: m[0] })
    } else if (m[3] !== undefined) {
      out.push({ kind: 'link', text: m[3], href: m[3] })
    } else {
      const bar = Number(m[5])
      out.push(bar > 0 ? { kind: 'bar', text: m[0], bar } : { kind: 'text', text: m[0] })
    }
    last = at + m[0].length
  }
  if (last < src.length) out.push({ kind: 'text', text: src.slice(last) })
  return out
}

function line(src: string): Span[] {
  const out: Span[] = []
  const parts = src.split(/\*\*/)
  // An odd count of ** pairs up; an unmatched last ** stays literal.
  const closed = parts.length % 2 === 1
  parts.forEach((p, i) => {
    if (!p) return
    const bold = i % 2 === 1 && (closed || i < parts.length - 1)
    if (bold) out.push({ kind: 'bold', children: inline(p) })
    else out.push(...inline(i % 2 === 1 ? '**' + p : p))
  })
  return out
}

export function parseInstructions(text: string): Paragraph[] {
  return text
    .replace(/\r\n?/g, '\n')
    .split(/\n\s*\n/)
    .map((p) => p.trim())
    .filter(Boolean)
    .map((p) => {
      const spans: Span[] = []
      p.split('\n').forEach((l, i) => {
        if (i) spans.push({ kind: 'br' })
        spans.push(...line(l))
      })
      return spans
    })
}
