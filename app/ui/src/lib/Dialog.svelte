<script lang="ts">
  import { dialog, settle } from './prompts.svelte'
  import { t } from './i18n.svelte'

  let input: HTMLInputElement | undefined = $state()
  let value = $state('')

  $effect(() => {
    const d = dialog.current
    if (d) {
      value = d.value
      queueMicrotask(() => {
        input?.focus()
        input?.select()
      })
    }
  })

  function key(e: KeyboardEvent) {
    if (e.key === 'Enter') settle(dialog.current?.kind === 'prompt' ? value : '')
    if (e.key === 'Escape') settle(null)
  }
</script>

{#if dialog.current}
  <div class="bg" role="presentation" onclick={() => settle(null)}></div>
  <div class="dlg glass" role="dialog" aria-modal="true" aria-label={dialog.current.message} tabindex="-1" onkeydown={key}>
    <p>{dialog.current.message}</p>
    {#if dialog.current.kind === 'prompt'}
      <input bind:this={input} bind:value aria-label={dialog.current.message} />
    {/if}
    <div class="actions">
      <button onclick={() => settle(null)}>{t('common.cancel')}</button>
      <button class="primary" onclick={() => settle(dialog.current?.kind === 'prompt' ? value : '')}
        >{dialog.current.kind === 'prompt' ? t('common.save') : 'OK'}</button
      >
    </div>
  </div>
{/if}

<style>
  .bg {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.25);
    z-index: 60;
  }
  .dlg {
    position: fixed;
    z-index: 61;
    left: 50%;
    top: 30%;
    transform: translate(-50%, -50%);
    width: min(420px, 90vw);
    padding: 18px 20px;
    border-radius: 18px;
    display: flex;
    flex-direction: column;
    gap: 12px;
    background: var(--glass-bg-strong, #fff);
  }
  p {
    margin: 0;
    font-size: 1rem;
  }
  input {
    font-size: 1.0625rem;
    padding: 8px 10px;
    border-radius: 10px;
    border: 1px solid rgba(0, 0, 0, 0.15);
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
</style>
