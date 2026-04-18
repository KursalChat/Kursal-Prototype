<script lang="ts">
  import { Reply, Copy, Pencil, Trash2, MoreHorizontal } from "lucide-svelte";
  import type { MessageResponse } from "$lib/types";

  interface Props {
    msg: MessageResponse;
    onClose: () => void;
    onReact: (emoji: string) => void;
    onMoreEmoji: () => void;
    onReply: () => void;
    onCopy: () => void;
    onEdit: () => void;
    onDelete: () => void;
  }

  let {
    msg,
    onClose,
    onReact,
    onMoreEmoji,
    onReply,
    onCopy,
    onEdit,
    onDelete,
  }: Props = $props();

  const quickEmojis = ["👍", "❤️", "😂", "😮", "😢", "🙏"];
</script>

<div
  class="sheet-backdrop"
  onclick={onClose}
  onkeydown={(e) => {
    if (e.key === "Escape") onClose();
  }}
  role="button"
  tabindex="-1"
  aria-label="Close"
></div>
<div class="action-sheet" role="dialog" aria-label="Message actions">
  <div class="sheet-handle"></div>
  <div class="sheet-reactions">
    {#each quickEmojis as emoji (emoji)}
      <button class="sheet-emoji" onclick={() => onReact(emoji)}>{emoji}</button>
    {/each}
    <button
      class="sheet-emoji more"
      onclick={onMoreEmoji}
      aria-label="More emoji"
    >
      <MoreHorizontal size={18} />
    </button>
  </div>
  <div class="sheet-actions">
    <button class="sheet-row" onclick={onReply}>
      <Reply size={18} /><span>Reply</span>
    </button>
    <button class="sheet-row" onclick={onCopy}>
      <Copy size={18} /><span>Copy</span>
    </button>
    {#if msg.direction === "sent" && !msg.fileDetails}
      <button class="sheet-row" onclick={onEdit}>
        <Pencil size={18} /><span>Edit</span>
      </button>
    {/if}
    {#if msg.direction === "sent"}
      <button class="sheet-row danger" onclick={onDelete}>
        <Trash2 size={18} /><span>Delete</span>
      </button>
    {/if}
  </div>
</div>

<style>
  .sheet-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.55);
    z-index: 300;
    animation: fadeIn 0.15s ease;
  }
  @keyframes fadeIn {
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }
  .action-sheet {
    position: fixed;
    bottom: 0;
    left: 0;
    right: 0;
    background: var(--bg-secondary);
    border-top: 1px solid var(--border);
    border-radius: 20px 20px 0 0;
    padding: 8px 10px max(16px, env(safe-area-inset-bottom));
    z-index: 310;
    animation: sheetUp 0.22s cubic-bezier(0.3, 0, 0.2, 1);
    box-shadow: 0 -10px 40px rgba(0, 0, 0, 0.4);
  }
  @keyframes sheetUp {
    from {
      transform: translateY(100%);
    }
    to {
      transform: translateY(0);
    }
  }
  .sheet-handle {
    width: 36px;
    height: 4px;
    background: rgba(148, 163, 184, 0.3);
    border-radius: 4px;
    margin: 6px auto 12px;
  }
  .sheet-reactions {
    display: flex;
    align-items: center;
    justify-content: space-around;
    gap: 4px;
    padding: 8px 4px;
    background: var(--bg-tertiary);
    border-radius: 16px;
    margin-bottom: 10px;
  }
  .sheet-emoji {
    font-size: 24px;
    width: 44px;
    height: 44px;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: all 0.12s;
  }
  .sheet-emoji:hover,
  .sheet-emoji:active {
    background: var(--bg-hover);
    transform: scale(1.15);
  }
  .sheet-emoji.more {
    color: var(--text-secondary);
    background: var(--bg-hover);
  }
  .sheet-actions {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .sheet-row {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 14px 16px;
    color: var(--text-primary);
    border-radius: 10px;
    font-size: 15px;
    text-align: left;
    transition: background var(--transition);
  }
  .sheet-row:hover,
  .sheet-row:active {
    background: var(--bg-hover);
  }
  .sheet-row.danger {
    color: var(--danger);
  }
</style>
