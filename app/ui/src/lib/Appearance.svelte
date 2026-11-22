<script lang="ts">
  // Skin picker: System + every registered skin, as a glass segmented control.
  import { onDestroy } from 'svelte'
  import { SKINS, getPreference, onSkinChange, setPreference, type Preference } from './skins'
  import { t, i18n } from './i18n.svelte'

  let pref: Preference = $state(getPreference())
  const off = onSkinChange((p) => (pref = p))
  onDestroy(off)

  // Skin names are Swedish in skins.ts; English names for the English UI.
  const EN: Record<string, string> = { light: 'Light', dark: 'Dark', sepia: 'Sepia' }
  let options = $derived([
    { id: 'system', name: 'System' },
    ...SKINS.map((s) => ({ id: s.id, name: i18n.lang === 'en' ? (EN[s.id] ?? s.name) : s.name })),
  ])
</script>

<div class="segmented" role="radiogroup" aria-label={t('read.appearance')}>
  {#each options as o (o.id)}
    <button
      role="radio"
      aria-checked={pref === o.id}
      class:on={pref === o.id}
      onclick={() => setPreference(o.id)}>{o.name}</button
    >
  {/each}
</div>
