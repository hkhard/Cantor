// In-app replacements for window.prompt/confirm. WKWebView on iOS only shows
// those when the host implements its UI delegate, so they can silently
// return null; these render inside the page on every platform.

export interface DialogReq {
  kind: 'prompt' | 'confirm'
  message: string
  value: string
  resolve: (v: string | null) => void
}

export const dialog = $state<{ current: DialogReq | null }>({ current: null })

export function ask(message: string, value = ''): Promise<string | null> {
  return new Promise((resolve) => {
    dialog.current = { kind: 'prompt', message, value, resolve }
  })
}

export function confirmBox(message: string): Promise<boolean> {
  return new Promise((resolve) => {
    dialog.current = { kind: 'confirm', message, value: '', resolve: (v) => resolve(v !== null) }
  })
}

export function settle(v: string | null) {
  const d = dialog.current
  dialog.current = null
  d?.resolve(v)
}
