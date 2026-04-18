<script lang="ts">
  import { tick } from "svelte";
  import { Send, X, Paperclip, Smile } from "lucide-svelte";
  import Spinner from "$lib/components/Spinner.svelte";
  import EmojiPicker from "$lib/components/EmojiPicker.svelte";
  import { typingState } from "$lib/state/typing.svelte";
  import { getMessagePreview } from "./chat-utils";
  import type { ContactResponse } from "$lib/types";

  interface Props {
    contact: ContactResponse;
    inputText: string;
    sending: boolean;
    isCoarsePointer: boolean;
    replyingPreview: string;
    editingPreview: string;
    replyActive: boolean;
    editActive: boolean;
    onSend: () => void;
    onAttach: () => void;
    onInput: () => void;
    onCancelReply: () => void;
    onCancelEdit: () => void;
    onOpenProfile: () => void;
    composerEl?: HTMLTextAreaElement | null;
  }

  let {
    contact,
    inputText = $bindable(),
    sending,
    isCoarsePointer,
    replyingPreview,
    editingPreview,
    replyActive,
    editActive,
    onSend,
    onAttach,
    onInput,
    onCancelReply,
    onCancelEdit,
    onOpenProfile,
    composerEl = $bindable(null),
  }: Props = $props();

  const MAX_MESSAGE_LENGTH = 10000;
  let showEmoji = $state(false);

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      if (showEmoji) {
        showEmoji = false;
        return;
      }
      if (replyActive) {
        e.preventDefault();
        onCancelReply();
        return;
      }
      if (editActive) {
        e.preventDefault();
        onCancelEdit();
        return;
      }
    }
    if (e.key === "Enter" && !e.shiftKey && !isCoarsePointer) {
      e.preventDefault();
      onSend();
    }
  }

  function handleEmojiSelect(emoji: string) {
    inputText += emoji;
    showEmoji = false;
    tick().then(() => composerEl?.focus());
  }
</script>

<div class="composer">
  {#if typingState.isTyping(contact.userId)}
    <div class="composer-typing">
      <span class="typing-dots">
        <span></span><span></span><span></span>
      </span>
      <span class="composer-typing-label">
        {contact.displayName} is typing…
      </span>
    </div>
  {/if}
  {#if contact.blocked}
    <div class="blocked-bar">
      <span>This contact is blocked.</span>
      <button class="blocked-link" onclick={onOpenProfile}>Manage</button>
    </div>
  {:else}
    {#if replyActive}
      <div class="composer-context">
        <span class="ctx-bar"></span>
        <div class="ctx-body">
          <span class="ctx-label">Replying</span>
          <span class="ctx-preview">{replyingPreview}</span>
        </div>
        <button
          class="ctx-cancel"
          onclick={onCancelReply}
          aria-label="Cancel reply"
          disabled={sending}
        >
          <X size={14} />
        </button>
      </div>
    {:else if editActive}
      <div class="composer-context editing">
        <span class="ctx-bar"></span>
        <div class="ctx-body">
          <span class="ctx-label">Editing</span>
          <span class="ctx-preview">{editingPreview}</span>
        </div>
        <button
          class="ctx-cancel"
          onclick={onCancelEdit}
          aria-label="Cancel edit"
          disabled={sending}
        >
          <X size={14} />
        </button>
      </div>
    {/if}

    <div class="composer-row">
      <button
        class="composer-btn"
        title="Attach file"
        aria-label="Attach file"
        onclick={onAttach}
        disabled={sending}
      >
        <Paperclip size={20} />
      </button>

      <div class="emoji-compose-anchor">
        <button
          class="composer-btn"
          title="Emoji"
          aria-label="Emoji"
          onclick={() => (showEmoji = !showEmoji)}
          disabled={sending}
        >
          <Smile size={20} />
        </button>
        {#if showEmoji}
          <div class="emoji-compose-popover">
            <EmojiPicker
              onSelect={handleEmojiSelect}
              onClose={() => (showEmoji = false)}
            />
          </div>
        {/if}
      </div>

      <textarea
        bind:this={composerEl}
        bind:value={inputText}
        oninput={onInput}
        onkeydown={handleKeydown}
        placeholder={`Message ${contact.displayName}`}
        rows="1"
        maxlength={MAX_MESSAGE_LENGTH}
        disabled={sending}
      ></textarea>

      <button
        class="send-btn"
        class:ready={!!inputText.trim() && !sending}
        onclick={onSend}
        disabled={!inputText.trim() || sending}
        aria-label="Send message"
      >
        {#if sending}
          <Spinner size={18} color="#fff" />
        {:else}
          <Send size={18} />
        {/if}
      </button>
    </div>
  {/if}
</div>

<style>
  .composer {
    background: rgba(17, 24, 39, 0.92);
    backdrop-filter: blur(12px);
    -webkit-backdrop-filter: blur(12px);
    padding: 8px 12px max(8px, env(safe-area-inset-bottom));
    position: relative;
    flex-shrink: 0;
  }
  .composer::before {
    content: "";
    position: absolute;
    left: 0;
    right: 0;
    bottom: 100%;
    height: 8px;
    background: linear-gradient(
      to top,
      rgba(17, 24, 39, 0.92) 0%,
      rgba(17, 24, 39, 0.55) 50%,
      rgba(17, 24, 39, 0) 100%
    );
    pointer-events: none;
    z-index: 1;
  }

  .composer-typing {
    position: absolute;
    left: 12px;
    bottom: calc(100% + 4px);
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    color: var(--accent);
    font-style: italic;
    pointer-events: none;
    max-width: calc(100% - 24px);
    z-index: 2;
  }
  .composer-typing-label {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .typing-dots {
    display: inline-flex;
    align-items: flex-end;
    gap: 2px;
    margin-right: 2px;
  }
  .typing-dots span {
    width: 4px;
    height: 4px;
    border-radius: 50%;
    background: currentColor;
    animation: typing-bounce 1.1s ease-in-out infinite;
  }
  .typing-dots span:nth-child(2) {
    animation-delay: 0.15s;
  }
  .typing-dots span:nth-child(3) {
    animation-delay: 0.3s;
  }
  @keyframes typing-bounce {
    0%,
    60%,
    100% {
      transform: translateY(0);
      opacity: 0.5;
    }
    30% {
      transform: translateY(-3px);
      opacity: 1;
    }
  }

  .composer-context {
    display: flex;
    align-items: stretch;
    gap: 8px;
    padding: 6px 10px;
    margin-bottom: 6px;
    background: var(--accent-dim);
    border-radius: var(--radius-md);
    font-size: 12.5px;
    animation: slideUp 0.15s ease-out;
  }
  .composer-context.editing {
    background: rgba(251, 191, 36, 0.12);
  }
  .ctx-bar {
    width: 3px;
    background: var(--accent);
    border-radius: 2px;
  }
  .composer-context.editing .ctx-bar {
    background: var(--warning);
  }
  .ctx-body {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
    justify-content: center;
  }
  .ctx-label {
    font-weight: 700;
    color: var(--accent-hover);
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.02em;
  }
  .composer-context.editing .ctx-label {
    color: var(--warning);
  }
  .ctx-preview {
    color: var(--text-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .ctx-cancel {
    width: 26px;
    height: 26px;
    align-self: center;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-muted);
    transition: all var(--transition);
    flex-shrink: 0;
  }
  .ctx-cancel:hover {
    background: rgba(248, 113, 113, 0.15);
    color: var(--danger);
  }

  @keyframes slideUp {
    from {
      opacity: 0;
      transform: translateY(4px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }

  .composer-row {
    display: flex;
    align-items: flex-end;
    gap: 6px;
    background: var(--bg-input);
    border: 1px solid var(--border);
    border-radius: 22px;
    padding: 3px;
    transition: border-color var(--transition);
  }
  .composer-row:focus-within {
    border-color: rgba(129, 140, 248, 0.5);
  }

  .composer-btn {
    width: 38px;
    height: 38px;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-muted);
    border-radius: 50%;
    transition: all var(--transition);
    flex-shrink: 0;
  }
  .composer-btn:hover:not(:disabled) {
    color: var(--text-primary);
    background: var(--bg-hover);
  }
  .composer-btn:active:not(:disabled) {
    transform: scale(0.92);
  }
  .composer-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .emoji-compose-anchor {
    position: relative;
  }
  .emoji-compose-popover {
    position: absolute;
    bottom: calc(100% + 8px);
    left: 0;
    z-index: 200;
  }

  textarea {
    flex: 1;
    background: transparent;
    border: none;
    color: var(--text-primary);
    padding: 9px 6px;
    resize: none;
    min-height: 38px;
    max-height: 180px;
    font-size: 15px;
    line-height: 1.4;
    outline: none;
    font-family: inherit;
  }
  textarea::placeholder {
    color: var(--text-muted);
  }
  textarea:disabled {
    opacity: 0.5;
  }

  .send-btn {
    width: 38px;
    height: 38px;
    background: var(--bg-hover);
    color: var(--text-muted);
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: all 0.18s ease;
    flex-shrink: 0;
  }
  .send-btn.ready {
    background: var(--accent);
    color: #fff;
    transform: scale(1);
  }
  .send-btn.ready:hover {
    background: var(--accent-hover);
    transform: scale(1.06);
  }
  .send-btn:active:not(:disabled) {
    transform: scale(0.92);
  }
  .send-btn:disabled {
    cursor: not-allowed;
  }

  .blocked-bar {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    padding: 14px;
    color: var(--text-muted);
    font-size: 13px;
  }
  .blocked-link {
    color: var(--accent);
    font-size: 13px;
    font-weight: 600;
    padding: 0;
  }
  .blocked-link:hover {
    text-decoration: underline;
  }

  @media (max-width: 768px) {
    .emoji-compose-popover {
      left: -56px;
    }
    .composer {
      padding-left: 8px;
      padding-right: 8px;
    }
    textarea {
      font-size: 16px;
    }
  }
</style>
