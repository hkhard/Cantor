<script lang="ts">
  // Cold-start splash: the flapping Songbird (design/ChoirBird icon set,
  // songbird-flap.gif, a 0.8 s loop) on the icon's navy, matching the
  // static iOS launch screen so the hand-over is seamless. Shown until the
  // app is ready, and for at least one wing beat; fades out (no fade with
  // reduced motion, and then the still picture instead of the animation).
  import { onMount } from 'svelte'
  import { t } from './i18n.svelte'

  let { ready }: { ready: boolean } = $props()

  const reduce = typeof matchMedia !== 'undefined' && matchMedia('(prefers-reduced-motion: reduce)').matches
  // Cold start only: a reload of the web view in the same app run (or a
  // test harness reload) goes straight to the app.
  const warm = typeof sessionStorage !== 'undefined' && sessionStorage.getItem('choirbird.splashed') === '1'
  try {
    sessionStorage.setItem('choirbird.splashed', '1')
  } catch {
    /* storage unavailable */
  }
  let minDone = $state(warm)
  let gone = $state(warm)
  let leaving = $derived(ready && minDone)

  onMount(() => {
    const t1 = setTimeout(() => (minDone = true), reduce ? 0 : 800)
    return () => clearTimeout(t1)
  })
  $effect(() => {
    if (!leaving) return
    const t2 = setTimeout(() => (gone = true), reduce ? 0 : 320)
    return () => clearTimeout(t2)
  })
</script>

{#if !gone}
  <div class="splash" class:leaving role="status" aria-live="polite" aria-label={t('splash.loading')}>
    <img src={reduce ? '/choirbird-mark.svg' : '/songbird-flap.gif'} alt="" width="320" height="320" />
  </div>
{/if}

<style>
  .splash {
    position: fixed;
    inset: 0;
    z-index: 1000;
    display: flex;
    align-items: center;
    justify-content: center;
    background: #0d1b3e;
    opacity: 1;
    transition: opacity 0.3s ease;
  }
  .splash.leaving {
    opacity: 0;
    pointer-events: none;
  }
  img {
    width: 20rem;
    height: 20rem;
    border-radius: 4.5rem;
  }
  @media (prefers-reduced-motion: reduce) {
    .splash {
      transition: none;
    }
  }
</style>
