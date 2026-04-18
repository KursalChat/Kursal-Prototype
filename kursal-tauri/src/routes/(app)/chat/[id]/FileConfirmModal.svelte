<script lang="ts">
  import { FileText } from "lucide-svelte";
  import Spinner from "$lib/components/Spinner.svelte";
  import { formatFileSize } from "./chat-utils";

  interface Props {
    filename: string;
    sizeBytes: number;
    sending: boolean;
    onConfirm: () => void;
    onCancel: () => void;
  }

  let { filename, sizeBytes, sending, onConfirm, onCancel }: Props = $props();
</script>

<div
  class="file-confirm-backdrop"
  role="presentation"
  onclick={onCancel}
  onkeydown={(e) => {
    if (e.key === "Escape") onCancel();
  }}
>
  <div
    class="file-confirm"
    role="dialog"
    aria-label="Send file"
    onclick={(e) => e.stopPropagation()}
    onkeydown={(e) => e.stopPropagation()}
    tabindex="-1"
  >
    <div class="file-confirm-icon"><FileText size={28} /></div>
    <h3>Send file?</h3>
    <div class="file-confirm-info">
      <span class="f-name" title={filename}>{filename}</span>
      {#if sizeBytes > 0}
        <span class="f-size">{formatFileSize(sizeBytes)}</span>
      {/if}
    </div>
    <div class="file-confirm-actions">
      <button class="fc-btn ghost" onclick={onCancel} disabled={sending}>
        Cancel
      </button>
      <button class="fc-btn primary" onclick={onConfirm} disabled={sending}>
        {#if sending}<Spinner size={14} color="#fff" />{:else}Send{/if}
      </button>
    </div>
  </div>
</div>

<style>
  .file-confirm-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(2, 6, 23, 0.65);
    backdrop-filter: blur(6px);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 400;
    animation: fadeIn 0.14s ease;
  }
  @keyframes fadeIn {
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }
  .file-confirm {
    width: min(360px, calc(100vw - 32px));
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    padding: 22px 22px 18px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
    box-shadow: var(--glow);
  }
  .file-confirm h3 {
    font-size: 16px;
    font-weight: 600;
    margin: 0;
  }
  .file-confirm-icon {
    width: 52px;
    height: 52px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 50%;
    background: var(--accent-dim);
    color: var(--accent);
  }
  .file-confirm-info {
    width: 100%;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    padding: 10px 12px;
    background: var(--bg-tertiary);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    overflow: hidden;
  }
  .file-confirm-info .f-name {
    font-weight: 600;
    font-size: 13.5px;
    max-width: 100%;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--text-primary);
  }
  .file-confirm-info .f-size {
    font-size: 12px;
    color: var(--text-muted);
  }
  .file-confirm-actions {
    display: flex;
    gap: 8px;
    width: 100%;
    margin-top: 2px;
  }
  .fc-btn {
    flex: 1;
    padding: 10px 14px;
    border-radius: var(--radius-md);
    font-weight: 600;
    font-size: 13.5px;
    transition: all var(--transition);
    display: flex;
    align-items: center;
    justify-content: center;
    min-height: 38px;
  }
  .fc-btn.ghost {
    background: var(--bg-tertiary);
    color: var(--text-secondary);
    border: 1px solid var(--border);
  }
  .fc-btn.ghost:hover:not(:disabled) {
    background: var(--bg-hover);
    color: var(--text-primary);
  }
  .fc-btn.primary {
    background: var(--accent);
    color: #fff;
  }
  .fc-btn.primary:hover:not(:disabled) {
    background: var(--accent-hover);
  }
  .fc-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
</style>
