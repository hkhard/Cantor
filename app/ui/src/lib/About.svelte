<script lang="ts">
  // NFR-09: About, with the licences of what Cantor is built on.
  import { t, i18n, setLang, textSize, setTextScale } from './i18n.svelte'

  let { onclose }: { onclose: () => void } = $props()

  const licences = [
    { name: 'Verovio', what: 'Notsättning / music engraving', licence: 'LGPL-3.0', url: 'https://www.verovio.org' },
    { name: 'pdf.js', what: 'PDF', licence: 'Apache-2.0', url: 'https://mozilla.github.io/pdf.js/' },
    { name: 'GeneralUser GS', what: 'SoundFont (S. Christian Collins)', licence: 'GeneralUser GS License v2.0 (fri användning)', url: 'https://schristiancollins.com/generaluser.php' },
    { name: 'RustySynth', what: 'SoundFont-syntes', licence: 'MIT', url: 'https://github.com/sinshu/rustysynth' },
    { name: 'cpal', what: 'Ljud ut / audio output', licence: 'Apache-2.0', url: 'https://github.com/RustAudio/cpal' },
    { name: 'Tauri', what: 'App', licence: 'MIT / Apache-2.0', url: 'https://tauri.app' },
    { name: 'Svelte', what: 'UI', licence: 'MIT', url: 'https://svelte.dev' },
    { name: 'quick-xml, zip, serde, objc2', what: 'MusicXML, MXL, data, iOS', licence: 'MIT / Apache-2.0', url: 'https://crates.io' },
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
  button {
    align-self: flex-end;
  }
</style>
