import { mount } from 'svelte'
import './app.css'
import App from './App.svelte'
import { initSkins } from './lib/skins'

// Apply the skin before first paint so there is no light/dark flash.
initSkins()

const app = mount(App, {
  target: document.getElementById('app')!,
})

export default app
