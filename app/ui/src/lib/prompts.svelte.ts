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

/**
 * iOS only shows the keyboard for an input focused inside a user gesture
 * (a tap's touchend). Dialogs open later (after an await or an animation),
 * so their input would get focus with no keyboard. Focusing this hidden
 * input during the gesture brings the keyboard up; moving focus from it to
 * the dialog's input keeps it up. Call from pointerup/click handlers.
 */
export function primeKeyboard() {
  let el = document.getElementById('kbd-primer') as HTMLInputElement | null
  if (!el) {
    el = document.createElement('input')
    el.id = 'kbd-primer'
    el.setAttribute('aria-hidden', 'true')
    el.tabIndex = -1
    // 16 px keeps iOS from zooming; off-screen but focusable.
    el.style.cssText = 'position:fixed;top:0;left:0;width:1px;height:1px;opacity:0;font-size:16px;border:0;padding:0;pointer-events:none'
    document.body.appendChild(el)
  }
  el.focus({ preventScroll: true })
}

export function ask(message: string, value = ''): Promise<string | null> {
  primeKeyboard()
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
