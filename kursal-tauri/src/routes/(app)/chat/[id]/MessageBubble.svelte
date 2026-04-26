<script lang="ts">
  import {
    Reply,
    Copy,
    Pencil,
    Trash2,
    Smile,
    FileText,
    Download,
    Check,
    CheckCheck,
    CloudUpload,
    FolderOpen,
  } from "lucide-svelte";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { openPath, revealItemInDir } from "@tauri-apps/plugin-opener";
  import { exists } from "@tauri-apps/plugin-fs";
  import Spinner from "$lib/components/Spinner.svelte";
  import { notifications } from "$lib/state/notifications.svelte";
  import type { MessageResponse } from "$lib/types";
  import {
    formatFileSize,
    formatTime,
    getMessagePreview,
    mediaKindFromFilename,
    renderMarkdown,
  } from "./chat-utils";

  async function openLocalFile(path: string) {
    try {
      await openPath(path);
    } catch (e) {
      notifications.push(
        `Couldn't open file: ${e instanceof Error ? e.message : e}`,
        "error",
      );
    }
  }
  async function revealLocalFile(path: string) {
    try {
      await revealItemInDir(path);
    } catch (e) {
      notifications.push(
        `Couldn't reveal file: ${e instanceof Error ? e.message : e}`,
        "error",
      );
    }
  }

  interface Reaction {
    emoji: string;
    userIds: string[];
  }

  interface Props {
    msg: MessageResponse;
    repliedMessage: MessageResponse | null;
    reactions: Reaction[];
    userId: string;
    isFirst: boolean;
    isLast: boolean;
    isCoarsePointer: boolean;
    hovered: boolean;
    emojiOpen: boolean;
    swipeDx: number;
    fileOfferState: "idle" | "accepting" | "accepted" | undefined;
    transferPercent: number;
    transferInProgress: boolean;
    transferDone: boolean;
    onHoverEnter: () => void;
    onHoverLeave: () => void;
    onTouchStart: (e: TouchEvent) => void;
    onTouchMove: (e: TouchEvent) => void;
    onTouchEnd: () => void;
    onTouchCancel: () => void;
    onReplyRefClick: (replyToId: string) => void;
    onAcceptFile: () => void;
    onToggleReact: (emoji: string) => void;
    onStartReply: () => void;
    onCopy: () => void;
    onStartEdit: () => void;
    onDelete: () => void;
    onResend: () => void;
    onDeleteLocal: () => void;
    onToggleEmojiPicker: (rect: DOMRect | null) => void;
  }

  let {
    msg,
    repliedMessage,
    reactions,
    userId,
    isFirst,
    isLast,
    isCoarsePointer,
    hovered,
    emojiOpen,
    swipeDx,
    fileOfferState,
    transferPercent,
    transferInProgress,
    transferDone,
    onHoverEnter,
    onHoverLeave,
    onTouchStart,
    onTouchMove,
    onTouchEnd,
    onTouchCancel,
    onReplyRefClick,
    onAcceptFile,
    onToggleReact,
    onStartReply,
    onCopy,
    onStartEdit,
    onDelete,
    onResend,
    onDeleteLocal,
    onToggleEmojiPicker,
  }: Props = $props();

  let pathMissing = $state(false);
  $effect(() => {
    const path = msg.fileDetails?.autodownloadPath ?? null;
    if (!path || transferInProgress) {
      pathMissing = false;
      return;
    }
    let cancelled = false;
    exists(path)
      .then((ok) => {
        if (!cancelled) pathMissing = !ok;
      })
      .catch(() => {
        if (!cancelled) pathMissing = true;
      });
    return () => {
      cancelled = true;
    };
  });
</script>

<div
  class="msg-row"
  class:first={isFirst}
  class:last={isLast}
  class:sent={msg.direction === "sent"}
  class:received={msg.direction === "received"}
  role="group"
  data-msg-id={msg.id}
  onmouseenter={onHoverEnter}
  onmouseleave={onHoverLeave}
  ontouchstart={onTouchStart}
  ontouchmove={onTouchMove}
  ontouchend={onTouchEnd}
  ontouchcancel={onTouchCancel}
>
  {#if swipeDx > 0}
    <div
      class="swipe-reply-indicator"
      style="opacity: {Math.min(swipeDx / 60, 1)}"
    >
      <Reply size={16} />
    </div>
  {/if}

  <div
    class="bubble"
    class:sent={msg.direction === "sent"}
    class:has-file={!!msg.fileDetails}
    class:failed={msg.status === "failed"}
    class:queued={msg.status === "queued"}
    class:sending={msg.status === "sending"}
    style="transform: translateX({swipeDx}px);"
  >
    {#if msg.replyTo}
      <button
        class="reply-ref"
        onclick={() => msg.replyTo && onReplyRefClick(msg.replyTo)}
      >
        <span class="reply-bar"></span>
        <span class="reply-body">
          <span class="reply-label">Reply</span>
          <span class="reply-text">
            {repliedMessage
              ? getMessagePreview(repliedMessage.content)
              : "Original message"}
          </span>
        </span>
      </button>
    {/if}

    <div class="selectable msg-content">
      {#if msg.fileDetails}
        {@const autoPath =
          msg.fileDetails.autodownloadPath &&
          !transferInProgress &&
          !pathMissing
            ? msg.fileDetails.autodownloadPath
            : null}
        {@const mediaKind = autoPath
          ? mediaKindFromFilename(msg.fileDetails.filename)
          : "other"}
        {@const mediaSrc = autoPath ? convertFileSrc(autoPath) : null}

        {#if autoPath && mediaSrc && mediaKind === "image"}
          <button
            class="media-img-btn"
            title="Open"
            onclick={() => openLocalFile(autoPath)}
          >
            <img
              class="media-img"
              src={mediaSrc}
              alt={msg.fileDetails.filename}
              loading="lazy"
            />
          </button>
        {:else if autoPath && mediaSrc && mediaKind === "video"}
          <!-- svelte-ignore a11y_media_has_caption -->
          <video
            class="media-video"
            src={mediaSrc}
            controls
            preload="metadata"
          ></video>
        {:else if autoPath && mediaSrc && mediaKind === "audio"}
          <audio
            class="media-audio"
            src={mediaSrc}
            controls
            preload="metadata"
          ></audio>
        {/if}

        <div class="file-bubble" class:embedded={!!autoPath && mediaKind !== "other"}>
          <div class="file-icon"><FileText size={22} /></div>
          <div class="file-info">
            <span class="file-name">{msg.fileDetails.filename}</span>
            <span class="file-size">
              {formatFileSize(msg.fileDetails.sizeBytes) ||
                (msg.direction === "sent" ? "Sent" : "Received")}
            </span>
            {#if transferInProgress}
              <div class="file-progress">
                <div class="progress-bar">
                  <div
                    class="progress-fill"
                    style="width:{transferPercent}%"
                  ></div>
                </div>
                <span class="progress-label">{transferPercent}%</span>
              </div>
            {/if}
          </div>
          {#if msg.direction === "received"}
            {#if autoPath}
              <button
                class="file-dl-btn"
                title="Show in folder"
                aria-label="Show in folder"
                onclick={() => revealLocalFile(autoPath)}
              >
                <FolderOpen size={16} />
              </button>
            {:else if fileOfferState === "accepted" && transferDone}
              <span class="file-done" title="Complete">
                <Check size={18} />
              </span>
            {:else if fileOfferState === "accepted"}
              <span class="file-pending">
                <Spinner size={16} color="var(--accent)" />
              </span>
            {:else}
              <button
                class="file-dl-btn"
                title="Download"
                aria-label="Download file"
                disabled={fileOfferState === "accepting"}
                onclick={onAcceptFile}
              >
                {#if fileOfferState === "accepting"}
                  <Spinner size={14} color="#fff" />
                {:else}
                  <Download size={16} />
                {/if}
              </button>
            {/if}
          {/if}
        </div>
      {:else}
        {@html renderMarkdown(msg.content, msg.edited)}
      {/if}
    </div>

    <div class="msg-meta">
      <span class="msg-time">{formatTime(msg.timestamp)}</span>
      {#if msg.direction === "sent"}
        {#if msg.status === "sending"}
          <span class="msg-status-dot">
            <Spinner size={10} color="currentColor" />
          </span>
        {:else if msg.status === "queued"}
          <span class="msg-status-dot queued" title="Queued — will send when online">
            <CloudUpload size={12} />
          </span>
          <button class="retry-btn" onclick={onResend} title="Send now">Send</button>
          <button class="retry-btn danger" onclick={onDeleteLocal} title="Discard">×</button>
        {:else if msg.status === "failed"}
          <span class="msg-status failed">failed</span>
          <button class="retry-btn" onclick={onResend}>Retry</button>
          <button class="retry-btn danger" onclick={onDeleteLocal}>Delete</button>
        {:else}
          <span class="msg-status-dot delivered">
            <CheckCheck size={12} />
          </span>
        {/if}
      {/if}
    </div>
  </div>

  {#if reactions.length > 0}
    <div class="reactions" class:sent={msg.direction === "sent"}>
      {#each reactions as reaction (reaction.emoji)}
        <button
          class="reaction-chip"
          class:mine={reaction.userIds.includes(userId)}
          onclick={() => onToggleReact(reaction.emoji)}
        >
          <span class="rem">{reaction.emoji}</span>
          {#if reaction.userIds.length > 1}
            <span class="rcount">{reaction.userIds.length}</span>
          {/if}
        </button>
      {/each}
    </div>
  {/if}

  {#if !isCoarsePointer && (hovered || emojiOpen) && msg.status !== "sending" && msg.status !== "failed" && msg.status !== "queued"}
    <div class="msg-actions" class:sent={msg.direction === "sent"}>
      <button
        class="act-btn"
        class:active={emojiOpen}
        title="React"
        aria-label="React"
        onmousedown={(e) => e.stopPropagation()}
        onclick={(e) =>
          onToggleEmojiPicker(
            (e.currentTarget as HTMLElement).getBoundingClientRect(),
          )}
      >
        <Smile size={15} />
      </button>
      <button class="act-btn" title="Reply" aria-label="Reply" onclick={onStartReply}>
        <Reply size={15} />
      </button>
      <button class="act-btn" title="Copy" aria-label="Copy" onclick={onCopy}>
        <Copy size={15} />
      </button>
      {#if msg.direction === "sent" && !msg.fileDetails}
        <button class="act-btn" title="Edit" aria-label="Edit" onclick={onStartEdit}>
          <Pencil size={15} />
        </button>
      {/if}
      {#if msg.direction === "sent"}
        <button
          class="act-btn danger"
          title="Delete"
          aria-label="Delete"
          onclick={onDelete}
        >
          <Trash2 size={15} />
        </button>
      {/if}
    </div>
  {/if}
</div>

<style>
  .msg-row {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    transition: background 0.3s ease;
    border-radius: 16px;
    padding: 1px 0;
  }
  .msg-row.sent {
    align-items: flex-end;
  }
  :global(.msg-row.flash) {
    background: var(--accent-dim);
  }

  .swipe-reply-indicator {
    position: absolute;
    left: -36px;
    top: 50%;
    transform: translateY(-50%);
    width: 28px;
    height: 28px;
    border-radius: 50%;
    background: var(--accent);
    color: #fff;
    display: flex;
    align-items: center;
    justify-content: center;
    pointer-events: none;
  }

  .bubble {
    --r-big: 18px;
    --r-sm: 6px;
    background: var(--bg-secondary);
    color: var(--text-primary);
    border: 1px solid var(--border);
    padding: 8px 13px;
    font-size: 14.5px;
    line-height: 1.5;
    width: fit-content;
    max-width: 100%;
    word-break: break-word;
    display: flex;
    flex-direction: column;
    gap: 3px;
    position: relative;
    transition:
      transform 0.18s ease,
      background 0.15s ease;
    border-radius: var(--r-big);
  }
  .msg-row.received .bubble {
    border-bottom-left-radius: var(--r-sm);
    border-top-left-radius: var(--r-sm);
  }
  .msg-row.received.first .bubble {
    border-top-left-radius: var(--r-big);
  }
  .msg-row.received.last .bubble {
    border-bottom-left-radius: var(--r-big);
  }
  .msg-row.received.first.last .bubble {
    border-radius: var(--r-big);
    border-bottom-left-radius: var(--r-sm);
  }

  .bubble.sent {
    background: linear-gradient(135deg, #6366f1, #818cf8);
    color: #fff;
    border-color: rgba(165, 180, 252, 0.3);
  }
  .msg-row.sent .bubble {
    border-bottom-right-radius: var(--r-sm);
    border-top-right-radius: var(--r-sm);
  }
  .msg-row.sent.first .bubble {
    border-top-right-radius: var(--r-big);
  }
  .msg-row.sent.last .bubble {
    border-bottom-right-radius: var(--r-big);
  }
  .msg-row.sent.first.last .bubble {
    border-radius: var(--r-big);
    border-bottom-right-radius: var(--r-sm);
  }

  .bubble.sending {
    opacity: 0.85;
  }
  .bubble.failed {
    background: linear-gradient(
      135deg,
      rgba(153, 27, 27, 0.5),
      rgba(185, 28, 28, 0.5)
    );
    border-color: rgba(248, 113, 113, 0.4);
    color: #fff;
  }
  .bubble.queued {
    background: linear-gradient(
      135deg,
      rgba(99, 102, 241, 0.55),
      rgba(129, 140, 248, 0.55)
    );
    border-style: dashed;
    border-color: rgba(165, 180, 252, 0.55);
    color: #fff;
  }
  .bubble.has-file {
    padding: 8px;
  }

  .msg-content :global(p) {
    margin: 0;
  }
  .msg-content :global(p + p) {
    margin-top: 0.45em;
  }
  .msg-content :global(code) {
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 0.88em;
    background: rgba(0, 0, 0, 0.28);
    border-radius: 4px;
    padding: 0.1em 0.35em;
  }
  .msg-content :global(pre) {
    margin: 0.5em 0 0.1em;
    padding: 0.6em 0.7em;
    border-radius: 10px;
    background: rgba(0, 0, 0, 0.35);
    overflow-x: auto;
    font-size: 12.5px;
  }
  .msg-content :global(pre code) {
    background: transparent;
    padding: 0;
  }
  .msg-content :global(ul),
  .msg-content :global(ol) {
    margin: 0.4em 0 0;
    padding-left: 1.3em;
  }
  .msg-content :global(li) {
    margin: 0.15em 0;
  }
  .msg-content :global(a) {
    color: inherit;
    text-decoration: underline;
    text-underline-offset: 2px;
  }
  .bubble:not(.sent) .msg-content :global(a) {
    color: var(--accent-hover);
  }
  .msg-content :global(blockquote) {
    border-left: 3px solid rgba(255, 255, 255, 0.3);
    padding-left: 8px;
    margin: 0.3em 0;
    color: rgba(255, 255, 255, 0.75);
  }
  .msg-content :global(.edited-tag) {
    font-size: 0.78em;
    opacity: 0.6;
    margin-left: 4px;
    font-style: italic;
  }

  .reply-ref {
    display: flex;
    align-items: stretch;
    gap: 8px;
    font-size: 12.5px;
    padding: 4px 0;
    margin-bottom: 2px;
    background: none;
    width: 100%;
    text-align: left;
    cursor: pointer;
    border-radius: 6px;
    overflow: hidden;
  }
  .reply-ref:hover {
    background: rgba(0, 0, 0, 0.12);
  }
  .reply-bar {
    width: 3px;
    background: currentColor;
    border-radius: 2px;
    opacity: 0.7;
    flex-shrink: 0;
  }
  .bubble.sent .reply-bar {
    background: rgba(255, 255, 255, 0.7);
  }
  .reply-body {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
    padding: 2px 4px;
  }
  .reply-label {
    font-weight: 700;
    font-size: 11px;
    opacity: 0.9;
    text-transform: uppercase;
    letter-spacing: 0.02em;
  }
  .bubble:not(.sent) .reply-label {
    color: var(--accent-hover);
  }
  .reply-text {
    opacity: 0.72;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 100%;
  }

  .reactions {
    display: flex;
    flex-wrap: wrap;
    gap: 3px;
    margin-top: 2px;
    max-width: 100%;
  }
  .reactions.sent {
    justify-content: flex-end;
  }
  .reaction-chip {
    border: 1px solid var(--border);
    background: var(--bg-secondary);
    border-radius: 999px;
    padding: 2px 8px;
    font-size: 13px;
    cursor: pointer;
    transition: all 0.12s;
    user-select: none;
    display: inline-flex;
    align-items: center;
    gap: 3px;
    line-height: 1.3;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.2);
  }
  .reaction-chip:hover {
    background: var(--bg-hover);
    transform: translateY(-1px);
  }
  .reaction-chip.mine {
    background: var(--accent-dim);
    border-color: rgba(129, 140, 248, 0.45);
    color: var(--accent-hover);
  }
  .reaction-chip.mine:hover {
    background: rgba(129, 140, 248, 0.25);
  }
  .rem {
    font-size: 14px;
    line-height: 1;
  }
  .rcount {
    font-size: 11px;
    opacity: 0.85;
    font-weight: 600;
  }

  .msg-meta {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 1px;
    justify-content: flex-end;
  }
  .msg-time {
    font-size: 10.5px;
    color: var(--text-muted);
  }
  .bubble.sent .msg-time {
    color: rgba(255, 255, 255, 0.65);
  }
  .msg-status {
    font-size: 10.5px;
    color: rgba(255, 255, 255, 0.75);
    text-transform: lowercase;
  }
  .msg-status.failed {
    color: #fecaca;
    font-weight: 600;
  }
  .msg-status-dot {
    display: inline-flex;
    color: rgba(255, 255, 255, 0.7);
  }
  .msg-status-dot.delivered {
    color: rgba(255, 255, 255, 0.9);
  }
  .msg-status-dot.queued {
    color: rgba(255, 255, 255, 0.95);
  }
  .retry-btn {
    font-size: 10.5px;
    font-weight: 700;
    color: #fff;
    padding: 1px 5px;
    cursor: pointer;
    background: rgba(255, 255, 255, 0.15);
    border-radius: 4px;
  }
  .retry-btn:hover {
    background: rgba(255, 255, 255, 0.28);
  }
  .retry-btn.danger {
    background: rgba(248, 113, 113, 0.25);
  }
  .retry-btn.danger:hover {
    background: rgba(248, 113, 113, 0.4);
  }

  .msg-actions {
    position: absolute;
    top: -14px;
    left: 4px;
    z-index: 100;
    display: flex;
    gap: 2px;
    background: var(--bg-secondary);
    padding: 3px;
    border-radius: 10px;
    border: 1px solid var(--border);
    box-shadow: 0 6px 16px rgba(0, 0, 0, 0.4);
    opacity: 0;
    animation: fadeInActions 0.12s ease forwards;
  }
  .msg-actions.sent {
    left: auto;
    right: 4px;
  }
  @keyframes fadeInActions {
    to {
      opacity: 1;
    }
  }

  .act-btn {
    width: 30px;
    height: 30px;
    border-radius: 7px;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-secondary);
    transition: all var(--transition);
  }
  .act-btn:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }
  .act-btn.danger:hover {
    background: rgba(248, 113, 113, 0.15);
    color: var(--danger);
  }

  .act-btn.active {
    background: var(--accent-dim);
    color: var(--accent-hover);
  }

  .media-img-btn {
    display: block;
    padding: 0;
    background: none;
    border: 0;
    cursor: zoom-in;
    border-radius: 12px;
    overflow: hidden;
    line-height: 0;
    max-width: 100%;
  }
  .media-img {
    display: block;
    max-width: 320px;
    max-height: 320px;
    width: auto;
    height: auto;
    object-fit: contain;
    border-radius: 12px;
  }
  .media-video {
    display: block;
    max-width: 320px;
    max-height: 360px;
    width: 100%;
    border-radius: 12px;
    background: #000;
  }
  .media-audio {
    display: block;
    width: 280px;
    max-width: 100%;
  }

  .file-bubble {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 4px;
    min-width: 220px;
    max-width: 320px;
  }
  .file-bubble.embedded {
    margin-top: 6px;
    min-width: 0;
    padding: 4px 2px 0;
    border-top: 1px solid rgba(255, 255, 255, 0.12);
  }
  .bubble:not(.sent) .file-bubble.embedded {
    border-top-color: var(--border-light);
  }
  .file-icon {
    width: 40px;
    height: 40px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(255, 255, 255, 0.12);
    border-radius: var(--radius-md);
    color: currentColor;
    flex-shrink: 0;
  }
  .bubble:not(.sent) .file-icon {
    background: var(--accent-dim);
    color: var(--accent);
  }
  .file-info {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .file-name {
    font-weight: 600;
    font-size: 13.5px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .file-size {
    font-size: 11px;
    opacity: 0.7;
  }
  .file-progress {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 5px;
  }
  .progress-bar {
    flex: 1;
    height: 4px;
    background: rgba(255, 255, 255, 0.12);
    border-radius: 999px;
    overflow: hidden;
  }
  .progress-fill {
    height: 100%;
    background: var(--accent);
    transition: width 120ms linear;
  }
  .bubble.sent .progress-fill {
    background: #fff;
  }
  .progress-label {
    font-size: 10px;
    opacity: 0.7;
    min-width: 30px;
    text-align: right;
  }
  .file-dl-btn {
    width: 36px;
    height: 36px;
    border-radius: 50%;
    background: var(--accent);
    color: #fff;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    transition: all var(--transition);
    flex-shrink: 0;
  }
  .file-dl-btn:hover:not(:disabled) {
    background: var(--accent-hover);
    transform: scale(1.05);
  }
  .file-dl-btn:active:not(:disabled) {
    transform: scale(0.95);
  }
  .file-dl-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .file-done {
    width: 36px;
    height: 36px;
    border-radius: 50%;
    background: rgba(52, 211, 153, 0.2);
    color: var(--success);
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }
  .file-pending {
    width: 36px;
    height: 36px;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }

  @media (max-width: 768px) {
    .msg-actions {
      display: none;
    }
    .bubble {
      font-size: 15px;
    }
  }
</style>
