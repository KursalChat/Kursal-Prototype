<script lang="ts">
  import { CloudUpload } from "lucide-svelte";
  import Spinner from "$lib/components/Spinner.svelte";
  import { t } from "$lib/i18n";

  interface Props {
    count: number;
    flushing: boolean;
    onFlush: () => void;
  }

  let { count, flushing, onFlush }: Props = $props();
</script>

{#if count > 0}
  <button
    class="queue-bar"
    class:busy={flushing}
    onclick={onFlush}
    disabled={flushing}
    aria-label={t('chat.offlineQueue.sendAllAriaLabel')}
    title={t('chat.offlineQueue.sendAllTitle')}
  >
    <span class="icon">
      {#if flushing}
        <Spinner size={14} color="currentColor" />
      {:else}
        <CloudUpload size={16} />
      {/if}
    </span>
    <span class="count">{count}</span>
    <span class="label">{flushing ? t('chat.offlineQueue.sending') : t('chat.offlineQueue.sendQueued')}</span>
  </button>
{/if}

<style>
  .queue-bar {
    display: flex;
    align-items: center;
    gap: 8px;
    align-self: center;
    margin: 0 auto;
    padding: 6px 14px;
    background: var(--surface);
    backdrop-filter: blur(12px);
    -webkit-backdrop-filter: blur(12px);
    color: var(--accent-hover);
    border: 1px solid var(--border-light);
    border-radius: 999px;
    font-size: 12.5px;
    font-weight: 600;
    cursor: pointer;
    transition: transform var(--transition), background var(--transition);
    animation: slideUp 0.22s cubic-bezier(0.34, 1.56, 0.64, 1);
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.10);
  }
  .queue-bar:hover:not(:disabled) {
    background: var(--bg-hover);
    transform: translateY(-1px);
  }
  .queue-bar:active:not(:disabled) {
    transform: translateY(0);
  }
  .queue-bar:disabled {
    cursor: progress;
    opacity: 0.85;
  }
  .icon {
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }
  .count {
    background: var(--accent);
    color: #fff;
    border-radius: 999px;
    min-width: 20px;
    padding: 0 6px;
    height: 18px;
    font-size: 11px;
    font-weight: 700;
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }
  .label {
    letter-spacing: 0.01em;
  }

  @keyframes slideUp {
    from {
      opacity: 0;
      transform: translateY(6px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }
</style>
