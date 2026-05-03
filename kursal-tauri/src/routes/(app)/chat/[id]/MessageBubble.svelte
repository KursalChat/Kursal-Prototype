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
    FolderOpen,
    Send,
  } from "lucide-svelte";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { revealItemInDir } from "@tauri-apps/plugin-opener";
  import { exists } from "@tauri-apps/plugin-fs";
  import { t } from "$lib/i18n";
  import Spinner from "$lib/components/Spinner.svelte";
  import { notifications } from "$lib/state/notifications.svelte";
  import type { MessageResponse } from "$lib/types";
  import {
    formatFileSize,
    formatTime,
    getMessagePreview,
    mediaKindFromFilename,
    renderMarkdown,
    midTruncate,
    fileTypeColor,
  } from "./chat-utils";

  async function revealLocalFile(path: string) {
    try {
      await revealItemInDir(path);
    } catch (e) {
      notifications.push(
        t("chat.bubble.errorRevealFile", {
          error: e instanceof Error ? e.message : String(e),
        }),
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
    onOpenMedia: (
      path: string,
      kind: "image" | "video",
      filename: string,
    ) => void;
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
    onOpenMedia,
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

  {#if msg.replyTo}
    <button
      class="reply-ref"
      class:sent={msg.direction === "sent"}
      onclick={() => msg.replyTo && onReplyRefClick(msg.replyTo)}
      style="transform: translateX({swipeDx}px); transition: {swipeDx > 0
        ? 'none'
        : 'transform 0.18s ease'};"
    >
      <span class="reply-label">{t("chat.bubble.replyLabel")}</span>
      <span class="reply-text">
        {repliedMessage
          ? getMessagePreview(repliedMessage.content)
          : t("chat.bubble.replyFallback")}
      </span>
    </button>
  {/if}

  <div class="bubble-anchor" class:sent={msg.direction === "sent"}>
    <div
      class="bubble"
      class:sent={msg.direction === "sent"}
      class:has-file={!!msg.fileDetails}
      class:failed={msg.status === "failed"}
      class:queued={msg.status === "queued"}
      class:sending={msg.status === "sending"}
      class:has-reply={!!msg.replyTo}
      style="transform: translateX({swipeDx}px); transition: {swipeDx > 0
        ? 'none'
        : 'transform 0.18s ease, background 0.15s ease'};"
    >
      <div class="msg-content" class:selectable={!isCoarsePointer}>
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
              onclick={() =>
                msg.fileDetails &&
                onOpenMedia(autoPath, "image", msg.fileDetails.filename)}
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
              ondblclick={() =>
                msg.fileDetails &&
                onOpenMedia(autoPath, "video", msg.fileDetails.filename)}
            ></video>
          {:else if autoPath && mediaSrc && mediaKind === "audio"}
            <audio
              class="media-audio"
              src={mediaSrc}
              controls
              preload="metadata"
            ></audio>
          {/if}

          <div
            class="file-bubble"
            class:embedded={!!autoPath && mediaKind !== "other"}
            style="--file-color: {fileTypeColor(msg.fileDetails.filename)};"
          >
            <div class="file-icon"><FileText size={22} /></div>
            <div class="file-info">
              <span class="file-name" title={msg.fileDetails.filename}>
                {midTruncate(msg.fileDetails.filename, 30)}
              </span>
              <span class="file-size">
                {formatFileSize(msg.fileDetails.sizeBytes) ||
                  (msg.direction === "sent"
                    ? t("chat.bubble.fileSizeSent")
                    : t("chat.bubble.fileSizeReceived"))}
              </span>
            </div>
            {#if msg.direction === "received"}
              {#if autoPath}
                <button
                  class="file-dl-btn ghost"
                  title={t("chat.bubble.showInFolder")}
                  aria-label={t("chat.bubble.showInFolder")}
                  onclick={() => revealLocalFile(autoPath)}
                >
                  <FolderOpen size={16} />
                </button>
              {:else if fileOfferState === "accepted" && transferDone}
                <span class="file-done" title={t("chat.bubble.complete")}>
                  <Check size={18} />
                </span>
              {:else if transferInProgress || fileOfferState === "accepted"}
                <span
                  class="file-progress-ring"
                  aria-label="{transferPercent}%"
                >
                  <svg viewBox="0 0 36 36" width="36" height="36">
                    <circle cx="18" cy="18" r="15" class="ring-track" />
                    <circle
                      cx="18"
                      cy="18"
                      r="15"
                      class="ring-fill"
                      stroke-dasharray="{(transferPercent / 100) * 94.25} 94.25"
                    />
                  </svg>
                  <span class="ring-pct">{transferPercent}%</span>
                </span>
              {:else}
                <button
                  class="file-dl-btn"
                  title={t("chat.bubble.download")}
                  aria-label={t("chat.bubble.downloadAriaLabel")}
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
            {:else if transferInProgress}
              <span class="file-progress-ring" aria-label="{transferPercent}%">
                <svg viewBox="0 0 36 36" width="36" height="36">
                  <circle cx="18" cy="18" r="15" class="ring-track" />
                  <circle
                    cx="18"
                    cy="18"
                    r="15"
                    class="ring-fill on-sent"
                    stroke-dasharray="{(transferPercent / 100) * 94.25} 94.25"
                  />
                </svg>
                <span class="ring-pct">{transferPercent}%</span>
              </span>
            {/if}
          </div>
        {:else}
          {@html renderMarkdown(msg.content, msg.edited)}
        {/if}
      </div>

      {#if isLast}
        <div class="msg-meta">
          <span class="msg-time">{formatTime(msg.timestamp)}</span>
        </div>
      {/if}
    </div>

    {#if !isCoarsePointer && (hovered || emojiOpen) && msg.status !== "sending" && msg.status !== "failed" && msg.status !== "queued"}
      <div class="msg-actions" class:sent={msg.direction === "sent"}>
        <button
          class="act-btn"
          class:active={emojiOpen}
          title={t("chat.bubble.actionReact")}
          aria-label={t("chat.bubble.actionReact")}
          onmousedown={(e) => e.stopPropagation()}
          onclick={(e) =>
            onToggleEmojiPicker(
              (e.currentTarget as HTMLElement).getBoundingClientRect(),
            )}
        >
          <Smile size={15} />
        </button>
        <button
          class="act-btn"
          title={t("chat.bubble.actionReply")}
          aria-label={t("chat.bubble.actionReply")}
          onclick={onStartReply}
        >
          <Reply size={15} />
        </button>
        <button
          class="act-btn"
          title={t("chat.bubble.actionCopy")}
          aria-label={t("chat.bubble.actionCopy")}
          onclick={onCopy}
        >
          <Copy size={15} />
        </button>
        {#if msg.direction === "sent" && !msg.fileDetails}
          <button
            class="act-btn"
            title={t("chat.bubble.actionEdit")}
            aria-label={t("chat.bubble.actionEdit")}
            onclick={onStartEdit}
          >
            <Pencil size={15} />
          </button>
        {/if}
        {#if msg.direction === "sent"}
          <button
            class="act-btn danger"
            title={t("chat.bubble.actionDelete")}
            aria-label={t("chat.bubble.actionDelete")}
            onclick={onDelete}
          >
            <Trash2 size={15} />
          </button>
        {/if}
      </div>
    {/if}
  </div>

  {#if msg.direction === "sent" && (msg.status === "queued" || msg.status === "failed")}
    <div
      class="msg-recovery"
      class:sent={msg.direction === "sent"}
      class:queued={msg.status === "queued"}
    >
      <span class="recovery-text">
        {msg.status === "failed"
          ? t("chat.bubble.statusFailed")
          : t("chat.bubble.offlineQueued")}
      </span>
      <div class="recovery-actions">
        <button
          class="recovery-icon primary"
          onclick={onResend}
          title={msg.status === "failed"
            ? t("chat.bubble.actionRetry")
            : t("chat.bubble.actionSend")}
          aria-label={msg.status === "failed"
            ? t("chat.bubble.actionRetry")
            : t("chat.bubble.actionSend")}
        >
          <Send size={13} />
        </button>
        <button
          class="recovery-icon"
          onclick={onDeleteLocal}
          title={t("chat.bubble.actionDelete")}
          aria-label={t("chat.bubble.actionDelete")}
        >
          <Trash2 size={13} />
        </button>
      </div>
    </div>
  {/if}

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
    touch-action: pan-y;
  }
  .msg-row.sent {
    align-items: flex-end;
  }
  :global(.msg-row.flash) {
    background: var(--accent-dim);
  }

  .swipe-reply-indicator {
    position: absolute;
    left: 4px;
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
    z-index: 2;
  }
  .msg-row.sent .swipe-reply-indicator {
    left: auto;
    right: 4px;
  }

  .bubble {
    --r-big: 20px;
    --r-sm: 6px;
    background: var(--bg-secondary);
    color: var(--text-primary);
    padding: 9px 14px;
    font-size: 14.5px;
    line-height: 1.55;
    width: fit-content;
    max-width: 100%;
    word-break: break-word;
    display: flex;
    flex-direction: column;
    gap: 3px;
    position: relative;
    transition:
      transform 0.18s ease,
      background 0.15s ease,
      box-shadow 0.18s ease;
    border-radius: var(--r-big);
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.04);
    animation: bubble-in 220ms cubic-bezier(0.34, 1.56, 0.64, 1);
  }
  :global([data-theme="light"]) .bubble {
    box-shadow: 0 1px 2px rgba(15, 23, 42, 0.05);
  }
  @keyframes bubble-in {
    from {
      transform: scale(0.94) translateY(4px);
      opacity: 0;
    }
    to {
      transform: scale(1) translateY(0);
      opacity: 1;
    }
  }

  /* Grouping pinch: only between adjacent same-author bubbles */
  .msg-row.received:not(.first) .bubble {
    border-top-left-radius: var(--r-sm);
  }
  .msg-row.received:not(.last) .bubble {
    border-bottom-left-radius: var(--r-sm);
  }
  .msg-row.sent:not(.first) .bubble {
    border-top-right-radius: var(--r-sm);
  }
  .msg-row.sent:not(.last) .bubble {
    border-bottom-right-radius: var(--r-sm);
  }

  .bubble.sent {
    background: var(--accent);
    color: #fff;
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.06);
  }

  .bubble.sending {
    opacity: 0.78;
  }
  .bubble.sending::after {
    content: "";
    position: absolute;
    inset: 0;
    border-radius: inherit;
    background: linear-gradient(
      90deg,
      transparent,
      rgba(255, 255, 255, 0.12),
      transparent
    );
    background-size: 200% 100%;
    animation: shimmer 1.4s linear infinite;
    pointer-events: none;
  }
  @keyframes shimmer {
    from {
      background-position: 200% 0;
    }
    to {
      background-position: -200% 0;
    }
  }
  .bubble.failed {
    background: var(--danger);
    color: #fff;
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.06);
    animation:
      bubble-in 220ms cubic-bezier(0.34, 1.56, 0.64, 1),
      failed-pulse 700ms ease-out 220ms 1;
  }
  @keyframes failed-pulse {
    0% {
      box-shadow:
        0 1px 2px rgba(0, 0, 0, 0.06),
        0 0 0 0 color-mix(in srgb, var(--danger) 50%, transparent);
    }
    100% {
      box-shadow:
        0 1px 2px rgba(0, 0, 0, 0.06),
        0 0 0 10px color-mix(in srgb, var(--danger) 0%, transparent);
    }
  }
  .bubble.queued {
    background: color-mix(in srgb, var(--accent) 60%, transparent);
    color: #fff;
    outline: 1.5px dashed color-mix(in srgb, var(--accent) 70%, transparent);
    outline-offset: -1.5px;
    animation:
      bubble-in 220ms cubic-bezier(0.34, 1.56, 0.64, 1),
      queued-breathe 2.6s ease-in-out 220ms infinite;
  }
  @keyframes queued-breathe {
    0%,
    100% {
      opacity: 1;
    }
    50% {
      opacity: 0.78;
    }
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
  .msg-content :global(.spoiler) {
    background: rgba(128, 128, 128, 0.55);
    color: transparent;
    border-radius: 4px;
    padding: 0 4px;
    cursor: pointer;
    user-select: none;
    transition:
      background 0.18s ease,
      color 0.18s ease;
  }
  .msg-content :global(.spoiler:hover:not(.revealed)) {
    background: rgba(128, 128, 128, 0.7);
  }
  .msg-content :global(.spoiler.revealed) {
    background: color-mix(in srgb, currentColor 14%, transparent);
    color: inherit;
    user-select: text;
  }
  .msg-content :global(.spoiler :is(a, code, strong, em)) {
    color: inherit;
  }
  .msg-content :global(.spoiler.revealed :is(a, code, strong, em)) {
    color: inherit;
  }

  .reply-ref {
    display: flex;
    flex-direction: column;
    gap: 1px;
    font-size: 12px;
    padding: 6px 12px 14px;
    margin-bottom: -10px;
    background: var(--bg-hover);
    border-radius: 14px 14px 14px 4px;
    text-align: left;
    cursor: pointer;
    transition: background 140ms ease;
    max-width: min(70%, 420px);
    align-self: flex-start;
    color: var(--text-secondary);
    position: relative;
    z-index: 1;
  }
  .reply-ref.sent {
    align-self: flex-end;
    border-radius: 14px 14px 4px 14px;
  }
  .reply-ref:hover {
    background: color-mix(in srgb, var(--text-primary) 12%, transparent);
  }
  .reply-label {
    font-weight: 600;
    font-size: 11px;
    color: var(--accent-hover);
    line-height: 1.2;
  }
  .reply-text {
    opacity: 0.85;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 100%;
    line-height: 1.3;
  }
  .bubble.has-reply {
    position: relative;
    z-index: 2;
  }

  .reactions {
    display: flex;
    flex-wrap: wrap;
    gap: 0;
    max-width: 100%;
    margin-top: 2px;
    margin-bottom: 2px;
    padding: 0 4px;
  }
  .reactions.sent {
    justify-content: flex-end;
  }
  .reaction-chip {
    background: transparent;
    border-radius: 999px;
    padding: 0 3px;
    font-size: 13px;
    cursor: pointer;
    transition: transform 180ms cubic-bezier(0.34, 1.56, 0.64, 1);
    user-select: none;
    display: inline-flex;
    align-items: center;
    gap: 2px;
    line-height: 1.2;
    animation: reaction-pop 220ms cubic-bezier(0.34, 1.56, 0.64, 1);
  }
  @keyframes reaction-pop {
    from {
      transform: scale(0);
      opacity: 0;
    }
    to {
      transform: scale(1);
      opacity: 1;
    }
  }
  .reaction-chip:hover {
    transform: scale(1.15);
  }
  .reaction-chip.mine {
    background: var(--accent-dim);
  }
  .reaction-chip.mine .rcount {
    color: var(--accent);
    font-weight: 700;
  }
  .rem {
    font-size: 14px;
    line-height: 1;
  }
  .rcount {
    font-size: 11px;
    color: var(--text-muted);
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }

  .msg-meta {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: -2px;
    line-height: 1;
    justify-content: flex-end;
    opacity: 0;
    max-height: 0;
    overflow: hidden;
    transition:
      opacity 0.18s ease,
      max-height 0.18s ease;
    pointer-events: none;
  }
  .msg-row.sent .msg-meta {
    justify-content: flex-end;
  }
  .msg-row.received .msg-meta {
    justify-content: flex-start;
  }
  .msg-row.last .msg-meta,
  .msg-row:hover .msg-meta {
    opacity: 1;
    max-height: 14px;
    pointer-events: auto;
  }
  .msg-time {
    font-size: 10.5px;
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
  }
  .bubble.sent .msg-time {
    color: rgba(255, 255, 255, 0.75);
  }
  .bubble.failed .msg-time,
  .bubble.queued .msg-time {
    color: rgba(255, 255, 255, 0.85);
  }

  /* Recovery strip — failed/queued messages */
  .msg-recovery {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    margin-top: 6px;
    padding: 8px 12px;
    background: var(--danger-dim);
    border-radius: 12px;
    font-size: 12.5px;
    color: var(--text-primary);
    max-width: 100%;
    animation: recovery-in 220ms cubic-bezier(0.34, 1.56, 0.64, 1);
  }
  .msg-recovery.queued {
    background: var(--accent-dim);
  }
  @keyframes recovery-in {
    from {
      opacity: 0;
      transform: translateY(-4px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }
  .recovery-text {
    color: var(--text-secondary);
    font-weight: 500;
  }
  .msg-recovery:not(.queued) .recovery-text {
    color: var(--danger);
  }
  .recovery-actions {
    display: flex;
    gap: 4px;
    flex-shrink: 0;
  }
  .recovery-icon {
    width: 24px;
    height: 24px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 999px;
    color: var(--text-secondary);
    transition: all var(--transition);
  }
  .recovery-icon:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }
  .recovery-icon.primary {
    background: var(--accent);
    color: #fff;
  }
  .recovery-icon.primary:hover {
    background: var(--accent-hover);
  }

  .bubble-anchor {
    position: relative;
    display: inline-block;
    max-width: 100%;
  }
  .bubble-anchor.sent {
    align-self: flex-end;
  }

  .msg-actions {
    position: absolute;
    bottom: calc(100% - 4px);
    left: 4px;
    z-index: 100;
    display: flex;
    gap: 2px;
    background: var(--surface);
    backdrop-filter: blur(10px);
    -webkit-backdrop-filter: blur(10px);
    padding: 3px;
    border-radius: 12px;
    border: 1px solid var(--border-light);
    box-shadow: 0 6px 18px rgba(0, 0, 0, 0.16);
    opacity: 0;
    animation: fadeInActions 160ms cubic-bezier(0.34, 1.56, 0.64, 1) forwards;
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
    padding: 6px 8px 6px 4px;
    min-width: 240px;
    max-width: 340px;
    border-left: 3px solid var(--file-color, var(--text-muted));
    padding-left: 10px;
    border-radius: 4px;
  }
  .file-bubble.embedded {
    margin-top: 8px;
    min-width: 0;
    padding-top: 8px;
    border-top: 1px solid rgba(255, 255, 255, 0.12);
    border-radius: 0;
    padding-left: 4px;
    border-left: 0;
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
    background: color-mix(
      in srgb,
      var(--file-color, var(--text-muted)) 18%,
      transparent
    );
    border-radius: var(--radius-md);
    color: var(--file-color, currentColor);
    flex-shrink: 0;
  }
  .bubble.sent .file-icon {
    background: rgba(255, 255, 255, 0.18);
    color: #fff;
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
    direction: ltr;
  }
  .file-size {
    font-size: 11px;
    opacity: 0.72;
    font-variant-numeric: tabular-nums;
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
  .file-dl-btn.ghost {
    background: var(--bg-hover);
    color: var(--text-secondary);
  }
  .bubble.sent .file-dl-btn.ghost {
    background: rgba(255, 255, 255, 0.18);
    color: #fff;
  }
  .file-dl-btn:hover:not(:disabled) {
    background: var(--accent-hover);
    transform: scale(1.05);
  }
  .file-dl-btn.ghost:hover:not(:disabled) {
    background: var(--bg-hover);
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
    background: color-mix(in srgb, var(--success) 22%, transparent);
    color: var(--success);
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }
  .file-progress-ring {
    position: relative;
    width: 36px;
    height: 36px;
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .file-progress-ring svg {
    transform: rotate(-90deg);
    overflow: visible;
  }
  .file-progress-ring .ring-track {
    fill: none;
    stroke: var(--bg-hover);
    stroke-width: 3;
  }
  .file-progress-ring .ring-fill {
    fill: none;
    stroke: var(--accent);
    stroke-width: 3;
    stroke-linecap: round;
    transition: stroke-dasharray 200ms linear;
  }
  .bubble.sent .file-progress-ring .ring-track {
    stroke: rgba(255, 255, 255, 0.2);
  }
  .file-progress-ring .ring-fill.on-sent,
  .bubble.sent .file-progress-ring .ring-fill {
    stroke: #fff;
  }
  .ring-pct {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 9px;
    font-weight: 700;
    color: var(--text-secondary);
    font-variant-numeric: tabular-nums;
  }
  .bubble.sent .ring-pct {
    color: #fff;
  }

  @media (max-width: 768px) {
    .msg-actions {
      display: none;
    }
    .bubble {
      font-size: 15px;
    }
  }

  /* Touch users: also kill the long-press callout menu. */
  .msg-content:not(.selectable) {
    -webkit-touch-callout: none;
  }
</style>
