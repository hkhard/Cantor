<script lang="ts">
  // NFR-09: About, with the licences of what Cantor is built on.
  import { t, i18n, setLang, textSize, setTextScale } from './i18n.svelte'
  import { api } from './api'
  import { confirmBox } from './prompts.svelte'

  let syncName: string | null = $state(null)
  let syncMsg = $state('')
  api.syncFolderName().then((n) => (syncName = n)).catch(() => {})

  async function chooseFolder() {
    try {
      const n = await api.chooseSyncFolder()
      if (n) {
        syncName = n
        await syncNow()
      }
    } catch (e) {
      syncMsg = t('backup.failed', { e: String(e) })
    }
  }

  async function syncNow() {
    busy = true
    try {
      const r = await api.syncNow()
      syncMsg = t('sync.done', { sent: r.sent, received: r.received })
      if (r.changed) onrestored?.()
    } catch (e) {
      syncMsg = t('backup.failed', { e: String(e) })
    }
    busy = false
  }

  async function stopSync() {
    await api.forgetSyncFolder()
    syncName = null
    syncMsg = ''
  }

  let backupMsg = $state('')
  let busy = $state(false)
  let restoreInput: HTMLInputElement | undefined = $state()

  async function makeBackup() {
    busy = true
    try {
      backupMsg = t('backup.made', { name: await api.backupLibrary() })
    } catch (e) {
      backupMsg = t('backup.failed', { e: String(e) })
    }
    busy = false
  }

  async function restorePicked(e: Event) {
    const input = e.target as HTMLInputElement
    const f = input.files?.[0]
    input.value = ''
    if (!f || !(await confirmBox(t('backup.restoreConfirm')))) return
    busy = true
    try {
      backupMsg = t('backup.restored', { n: await api.restoreLibrary(f) })
      onrestored?.()
    } catch (err) {
      backupMsg = t('backup.failed', { e: String(err) })
    }
    busy = false
  }

  let { onclose, onrestored }: { onclose: () => void; onrestored?: () => void } = $props()

  // AGPL-3.0 section 6(d)/13: where the Corresponding Source is. Must point at
  // the public repository (or tag) of the exact version before any release
  // outside the maintainer's own devices (docs/ROADMAP.md, phase 0).
  const SOURCE_URL = 'https://github.com/hkhard/Cantor'
  // The exact source of this build: a tag (v0.2.0) or a commit.
  const VERSION = __CANTOR_VERSION__
  const SOURCE_REF_URL = `${SOURCE_URL}/tree/${__CANTOR_REF__}`

  const licences = [
    { name: 'Verovio', what: 'Notsättning / music engraving', licence: 'LGPL-3.0', url: 'https://www.verovio.org' },
    { name: 'pdf.js', what: 'PDF', licence: 'Apache-2.0', url: 'https://mozilla.github.io/pdf.js/' },
    { name: 'GeneralUser GS', what: 'SoundFont (S. Christian Collins)', licence: 'GeneralUser GS License v2.0 (fri användning)', url: 'https://schristiancollins.com/generaluser.php' },
    { name: 'RustySynth', what: 'SoundFont-syntes', licence: 'MIT', url: 'https://github.com/sinshu/rustysynth' },
    { name: 'cpal', what: 'Ljud ut / audio output', licence: 'Apache-2.0', url: 'https://github.com/RustAudio/cpal' },
    { name: 'Tauri', what: 'App', licence: 'MIT / Apache-2.0', url: 'https://tauri.app' },
    { name: 'Svelte', what: 'UI', licence: 'MIT', url: 'https://svelte.dev' },
    { name: 'quick-xml, zip, serde, objc2', what: 'MusicXML, MXL, data, iOS', licence: 'MIT / Apache-2.0', url: 'https://crates.io' },
    { name: t('about.allOthers'), what: 'THIRD_PARTY_NOTICES.md', licence: 'MIT, Apache-2.0, BSD, ISC, Zlib, MPL-2.0, Unicode', url: SOURCE_URL },
  ]
</script>

<div class="sheetbg" role="presentation" onclick={onclose}></div>
<div class="about glass" role="dialog" aria-label={t('about.title')}>
  <img src="/cantor-mark.svg" alt="" width="56" height="56" />
  <h3>Cantor</h3>
  <p>{t('about.body')}</p>
  <label class="lang"
    >{t('about.language')}
    <select value={i18n.lang} onchange={(e) => setLang((e.target as HTMLSelectElement).value as 'sv' | 'en')}>
      <option value="sv">Svenska</option>
      <option value="en">English</option>
    </select>
  </label>
  <label class="lang"
    >{t('about.textSize')}
    <select value={String(textSize.scale)} onchange={(e) => setTextScale(Number((e.target as HTMLSelectElement).value))}>
      <option value="0.9">90 %</option>
      <option value="1">100 %</option>
      <option value="1.25">125 %</option>
      <option value="1.5">150 %</option>
      <option value="2">200 %</option>
    </select>
  </label>
  <h4>{t('sync.title')}</h4>
  <p class="hint">{t('sync.hint')}</p>
  {#if syncName}
    <p>{t('sync.folder', { name: syncName })}</p>
    <div class="row">
      <button onclick={syncNow} disabled={busy}>{t('sync.now')}</button>
      <button onclick={chooseFolder} disabled={busy}>{t('sync.change')}</button>
      <button onclick={stopSync} disabled={busy}>{t('sync.stop')}</button>
    </div>
  {:else}
    <div class="row"><button onclick={chooseFolder} disabled={busy}>{t('sync.choose')}</button></div>
  {/if}
  {#if syncMsg}<p role="status">{syncMsg}</p>{/if}
  <h4>{t('backup.title')}</h4>
  <p class="hint">{t('backup.hint')}</p>
  <div class="row">
    <button onclick={makeBackup} disabled={busy}>{t('backup.make')}</button>
    <button onclick={() => restoreInput?.click()} disabled={busy}>{t('backup.restore')}</button>
    <input bind:this={restoreInput} type="file" accept=".cantorbackup" style="display:none" onchange={restorePicked} aria-label={t('backup.restore')} />
  </div>
  {#if backupMsg}<p role="status">{backupMsg}</p>{/if}
  <h4>{t('about.licence')}</h4>
  <p class="licence">{t('about.licenceBody')}</p>
  <p class="licence">{t('about.version')} <span class="url">{VERSION}</span></p>
  <p class="licence">
    {t('about.source')} <span class="url">{SOURCE_REF_URL}</span>
  </p>
  <h4>{t('about.licences')}</h4>
  <ul>
    {#each licences as l}
      <li><b>{l.name}</b> · {l.what} · <span class="lic">{l.licence}</span> <span class="url">{l.url}</span></li>
    {/each}
  </ul>
  <button class="primary" onclick={onclose}>{t('common.close')}</button>
</div>

<style>
  .sheetbg {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.25);
    z-index: 40;
  }
  .about {
    position: fixed;
    z-index: 41;
    left: 50%;
    top: 50%;
    transform: translate(-50%, -50%);
    width: min(560px, 92vw);
    max-height: 88vh;
    overflow: auto;
    padding: 20px 22px;
    border-radius: 18px;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 8px;
  }
  h3,
  h4 {
    margin: 4px 0 0;
  }
  p {
    margin: 0;
  }
  ul {
    margin: 0;
    padding-left: 18px;
    font-size: 0.8125rem;
    line-height: 1.5;
  }
  .lic {
    color: var(--accent, #b8562b);
  }
  .url {
    color: #77716a;
    font-size: 0.75rem;
  }
  .lang {
    display: flex;
    gap: 8px;
    align-items: center;
  }
  button.primary {
    align-self: flex-end;
  }
  .row {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }
  .hint {
    font-size: 0.8125rem;
    color: var(--ink-muted);
  }
</style>
