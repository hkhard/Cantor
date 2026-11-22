<script lang="ts">
  // Skin picker: System + every registered skin, as a glass segmented control.
  import { onDestroy } from 'svelte'
  import { SKINS, getPreference, onSkinChange, setPreference, type Preference } from './skins'

  let pref: Preference = $state(getPreference())
  const off = onSkinChange((p) => (pref = p))
  onDestroy(off)

  const options = [{ id: 'system', name: 'System' }, ...SKINS.map((s) => ({ id: s.id, name: s.name }))]
</script>

<div class="segmented" role="radiogroup" aria-label="Utseende">
  {#each options as o (o.id)}
    <button
      role="radio"
      aria-checked={pref === o.id}
      class:on={pref === o.id}
      onclick={() => setPreference(o.id)}>{o.name}</button
    >
  {/each}
</div>
