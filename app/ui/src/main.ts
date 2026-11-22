import { mount } from 'svelte'
import './app.css'
import App from './App.svelte'
import { initSkins } from './lib/skins'
import { applyTextSize } from './lib/i18n.svelte'

// Apply the skin before first paint so there is no light/dark flash.
initSkins()
applyTextSize()
// Dynamic Type changes while the app runs arrive as a resize/visibility change.
document.addEventListener('visibilitychange', applyTextSize)

const app = mount(App, {
  target: document.getElementById('app')!,
})

export default app
