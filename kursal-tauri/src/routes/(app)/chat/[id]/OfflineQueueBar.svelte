<script lang="ts">
  import { CloudUpload } from "lucide-svelte";
  import Spinner from "$lib/components/Spinner.svelte";

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
    aria-label="Send all queued messages"
    title="Send all queued messages now"
  >
    <span class="icon">
      {#if flushing}
        <Spinner size={14} color="currentColor" />
      {:else}
        <CloudUpload size={16} />
      {/if}
    </span>
    <span class="count">{count}</span>
    <span class="label">{flushing ? "Sending…" : "Send queued"}</span>
  </button>
{/if}

<style>
  .queue-bar {
    display: flex;
    align-items: center;
    gap: 8px;
    align-self: center;
    margin: 0 auto 8px;
    padding: 6px 14px;
    background: var(--accent-dim);
    color: var(--accent-hover);
    border: 1px solid rgba(129, 140, 248, 0.35);
    border-radius: 999px;
    font-size: 12.5px;
    font-weight: 600;
    cursor: pointer;
    transition: all var(--transition);
    animation: slideUp 0.18s ease-out;
    box-shadow: 0 4px 12px rgba(99, 102, 241, 0.18);
  }
  .queue-bar:hover:not(:disabled) {
    background: rgba(129, 140, 248, 0.22);
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
