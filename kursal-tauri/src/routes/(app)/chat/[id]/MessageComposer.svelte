<script lang="ts">
  import { tick } from "svelte";
  import {
    Send,
    X,
    Paperclip,
    Smile,
    Bold,
    Italic,
    Strikethrough,
    Code,
    EyeOff,
    Link,
  } from "lucide-svelte";
  import { t } from "$lib/i18n";
  import Spinner from "$lib/components/Spinner.svelte";
  import EmojiPicker from "$lib/components/EmojiPicker.svelte";
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
  let formatBarVisible = $state(false);
  let stickyFormatBar = $state(false);
  let popoverPos = $state<{ top: number; left: number } | null>(null);
  // Suppress selection-driven position updates while we are mid-edit so
  // the popover doesn't jump between execCommand's collapsed caret and
  // the re-selected inner range.
  let isApplyingFormat = false;

  // Replace the textarea's [start..end] range with `text`.
  // Uses execCommand('insertText') so the change lands in the native
  // undo stack (Ctrl/Cmd+Z works), then sets the new selection.
  function replaceRange(
    start: number,
    end: number,
    text: string,
    selStart: number,
    selEnd: number,
  ) {
    if (!composerEl) return;
    const el = composerEl;
    isApplyingFormat = true;
    el.focus();
    el.setSelectionRange(start, end);
    const ok = document.execCommand("insertText", false, text);
    if (!ok) {
      const v = el.value;
      el.value = v.slice(0, start) + text + v.slice(end);
      el.dispatchEvent(new Event("input", { bubbles: true }));
    }
    requestAnimationFrame(() => {
      el.setSelectionRange(selStart, selEnd);
      isApplyingFormat = false;
      // Single position update once selection is final.
      if (formatBarVisible) positionPopoverAtSelection();
    });
  }

  function applyWrap(prefix: string, suffix: string = prefix) {
    if (!composerEl) return;
    const el = composerEl;
    const start = el.selectionStart;
    const end = el.selectionEnd;
    const value = el.value;
    const before = value.slice(0, start);
    const sel = value.slice(start, end);
    const after = value.slice(end);
    const wrapped = before.endsWith(prefix) && after.startsWith(suffix);
    if (wrapped) {
      // Strip surrounding markers
      replaceRange(
        start - prefix.length,
        end + suffix.length,
        sel,
        start - prefix.length,
        end - prefix.length,
      );
    } else {
      replaceRange(
        start,
        end,
        prefix + sel + suffix,
        start + prefix.length,
        end + prefix.length,
      );
    }
  }

  function applyLink() {
    if (!composerEl) return;
    const el = composerEl;
    const start = el.selectionStart;
    const end = el.selectionEnd;
    const sel = el.value.slice(start, end) || "text";
    const inserted = `[${sel}](url)`;
    const urlStart = start + sel.length + 3;
    replaceRange(start, end, inserted, urlStart, urlStart + 3);
  }

  // Mirror-div trick: copy textarea styles into a hidden div,
  // splice a marker span at the caret, read its rect, position popover.
  function getCaretCoords(
    el: HTMLTextAreaElement,
    pos: number,
  ): { left: number; top: number; height: number } {
    const styles = window.getComputedStyle(el);
    const div = document.createElement("div");
    const props = [
      "boxSizing",
      "width",
      "height",
      "borderTopWidth",
      "borderRightWidth",
      "borderBottomWidth",
      "borderLeftWidth",
      "borderStyle",
      "paddingTop",
      "paddingRight",
      "paddingBottom",
      "paddingLeft",
      "fontStyle",
      "fontVariant",
      "fontWeight",
      "fontStretch",
      "fontSize",
      "fontSizeAdjust",
      "lineHeight",
      "fontFamily",
      "textAlign",
      "textTransform",
      "textIndent",
      "textDecoration",
      "letterSpacing",
      "wordSpacing",
      "tabSize",
      "MozTabSize",
    ];
    for (const p of props) (div.style as any)[p] = (styles as any)[p];
    div.style.position = "absolute";
    div.style.visibility = "hidden";
    div.style.whiteSpace = "pre-wrap";
    div.style.wordWrap = "break-word";
    div.style.top = "0";
    div.style.left = "-9999px";
    div.style.overflow = "hidden";
    div.textContent = el.value.slice(0, pos);
    const span = document.createElement("span");
    span.textContent = el.value.slice(pos) || ".";
    div.appendChild(span);
    document.body.appendChild(div);
    const spanRect = span.getBoundingClientRect();
    const divRect = div.getBoundingClientRect();
    const lineHeight =
      parseFloat(styles.lineHeight) || parseFloat(styles.fontSize) * 1.4;
    document.body.removeChild(div);
    const taRect = el.getBoundingClientRect();
    return {
      left: taRect.left + (spanRect.left - divRect.left) - el.scrollLeft,
      top: taRect.top + (spanRect.top - divRect.top) - el.scrollTop,
      height: lineHeight,
    };
  }

  function positionPopoverAtSelection() {
    if (!composerEl) {
      popoverPos = null;
      return;
    }
    const start = composerEl.selectionStart;
    const coords = getCaretCoords(composerEl, start);
    const POPOVER_W = 224;
    const POPOVER_H = 36;
    const margin = 8;
    let left = coords.left - POPOVER_W / 2;
    let top = coords.top - POPOVER_H - 6;
    // clamp viewport
    left = Math.max(
      margin,
      Math.min(left, window.innerWidth - POPOVER_W - margin),
    );
    if (top < margin) top = coords.top + coords.height + 6;
    popoverPos = { top, left };
  }

  function positionPopoverAt(x: number, y: number) {
    const POPOVER_W = 224;
    const POPOVER_H = 36;
    const margin = 8;
    let left = x - POPOVER_W / 2;
    let top = y - POPOVER_H - 6;
    left = Math.max(
      margin,
      Math.min(left, window.innerWidth - POPOVER_W - margin),
    );
    if (top < margin) top = y + 6;
    popoverPos = { top, left };
  }

  function handleFormatClick(prefix: string, suffix: string = prefix) {
    stickyFormatBar = true;
    applyWrap(prefix, suffix);
  }

  function handleLinkClick() {
    stickyFormatBar = true;
    applyLink();
  }

  function updateFormatBar() {
    if (isApplyingFormat) return;
    if (!composerEl) return;
    if (document.activeElement !== composerEl) {
      if (!stickyFormatBar) formatBarVisible = false;
      return;
    }
    const has = composerEl.selectionEnd > composerEl.selectionStart;
    formatBarVisible = has || stickyFormatBar;
    if (formatBarVisible) positionPopoverAtSelection();
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      if (showEmoji) {
        showEmoji = false;
        return;
      }
      if (formatBarVisible) {
        formatBarVisible = false;
        stickyFormatBar = false;
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

    const meta = e.metaKey || e.ctrlKey;
    if (meta && !e.altKey) {
      const key = e.key.toLowerCase();
      if (e.shiftKey) {
        if (key === "x") {
          e.preventDefault();
          applyWrap("~~");
          return;
        }
        if (key === "p") {
          e.preventDefault();
          applyWrap("||");
          return;
        }
      } else {
        if (key === "b") {
          e.preventDefault();
          applyWrap("**");
          return;
        }
        if (key === "i") {
          e.preventDefault();
          applyWrap("_");
          return;
        }
        if (key === "e") {
          e.preventDefault();
          applyWrap("`");
          return;
        }
        if (key === "k") {
          e.preventDefault();
          applyLink();
          return;
        }
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

  function handleContextMenu(e: MouseEvent) {
    e.preventDefault();
    e.stopPropagation();
    stickyFormatBar = true;
    formatBarVisible = true;
    composerEl?.focus();
    positionPopoverAt(e.clientX, e.clientY);
  }

  function handleMouseDown(e: MouseEvent) {
    // Left click in textarea = user is moving caret / starting fresh selection.
    // Drop the sticky flag so an empty selection actually hides the bar.
    if (e.button === 0) stickyFormatBar = false;
  }

  function handleSelect() {
    if (composerEl) {
      const has = composerEl.selectionEnd > composerEl.selectionStart;
      if (!has) stickyFormatBar = false;
    }
    updateFormatBar();
  }

  function handleInput() {
    onInput();
    if (isApplyingFormat) return;
    if (formatBarVisible) {
      requestAnimationFrame(positionPopoverAtSelection);
    }
  }

  function handleBlur() {
    setTimeout(() => {
      if (!composerEl) return;
      if (document.activeElement !== composerEl) {
        formatBarVisible = false;
        stickyFormatBar = false;
      }
    }, 120);
  }

  $effect(() => {
    function onSel() {
      if (!composerEl) return;
      if (document.activeElement !== composerEl) return;
      updateFormatBar();
    }
    function onResize() {
      if (formatBarVisible) positionPopoverAtSelection();
    }
    document.addEventListener("selectionchange", onSel);
    window.addEventListener("resize", onResize);
    return () => {
      document.removeEventListener("selectionchange", onSel);
      window.removeEventListener("resize", onResize);
    };
  });
</script>

<div class="composer">
  {#if contact.blocked}
    <div class="blocked-bar">
      <span>{t("chat.composer.blocked")}</span>
      <button class="blocked-link" onclick={onOpenProfile}
        >{t("chat.composer.manage")}</button
      >
    </div>
  {:else}
    {#if replyActive}
      <div class="composer-context">
        <span class="ctx-bar"></span>
        <div class="ctx-body">
          <span class="ctx-label">{t("chat.composer.contextReplying")}</span>
          <span class="ctx-preview">{replyingPreview}</span>
        </div>
        <button
          class="ctx-cancel"
          onclick={onCancelReply}
          aria-label={t("chat.composer.cancelReplyAriaLabel")}
          disabled={sending}
        >
          <X size={14} />
        </button>
      </div>
    {:else if editActive}
      <div class="composer-context editing">
        <span class="ctx-bar"></span>
        <div class="ctx-body">
          <span class="ctx-label">{t("chat.composer.contextEditing")}</span>
          <span class="ctx-preview">{editingPreview}</span>
        </div>
        <button
          class="ctx-cancel"
          onclick={onCancelEdit}
          aria-label={t("chat.composer.cancelEditAriaLabel")}
          disabled={sending}
        >
          <X size={14} />
        </button>
      </div>
    {/if}

    <div class="composer-row">
      <button
        class="composer-btn"
        title={t("chat.composer.attachFile")}
        aria-label={t("chat.composer.attachFile")}
        onclick={onAttach}
        disabled={sending}
      >
        <Paperclip size={18} />
      </button>

      <div class="emoji-compose-anchor">
        <button
          class="composer-btn"
          title={t("chat.composer.emoji")}
          aria-label={t("chat.composer.emoji")}
          onclick={() => (showEmoji = !showEmoji)}
          disabled={sending}
        >
          <Smile size={18} />
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
        oninput={handleInput}
        onkeydown={handleKeydown}
        oncontextmenu={handleContextMenu}
        onmousedown={handleMouseDown}
        onselect={handleSelect}
        onblur={handleBlur}
        placeholder={t("chat.composer.placeholder", {
          name: contact.displayName,
        })}
        rows="1"
        maxlength={MAX_MESSAGE_LENGTH}
        disabled={sending}
      ></textarea>

      <button
        class="send-btn"
        class:ready={!!inputText.trim() && !sending}
        onclick={onSend}
        disabled={!inputText.trim() || sending}
        aria-label={t("chat.composer.sendAriaLabel")}
      >
        {#if sending}
          <Spinner size={16} color="#fff" />
        {:else}
          <Send size={16} />
        {/if}
      </button>
    </div>
  {/if}
</div>

{#if formatBarVisible && popoverPos}
  <div
    class="format-bar-floating"
    role="toolbar"
    tabindex="0"
    aria-label="Markdown formatting"
    style="top: {popoverPos.top}px; left: {popoverPos.left}px;"
    onmousedown={(e) => e.preventDefault()}
  >
    <button
      class="fmt-btn"
      title="{t('chat.composer.formatBold')}  (⌘B)"
      aria-label={t("chat.composer.formatBold")}
      onmousedown={(e) => e.preventDefault()}
      onclick={() => handleFormatClick("**")}
    >
      <Bold size={14} />
    </button>
    <button
      class="fmt-btn"
      title="{t('chat.composer.formatItalic')}  (⌘I)"
      aria-label={t("chat.composer.formatItalic")}
      onmousedown={(e) => e.preventDefault()}
      onclick={() => handleFormatClick("_")}
    >
      <Italic size={14} />
    </button>
    <button
      class="fmt-btn"
      title="{t('chat.composer.formatStrike')}  (⌘⇧X)"
      aria-label={t("chat.composer.formatStrike")}
      onmousedown={(e) => e.preventDefault()}
      onclick={() => handleFormatClick("~~")}
    >
      <Strikethrough size={14} />
    </button>
    <button
      class="fmt-btn"
      title="{t('chat.composer.formatCode')}  (⌘E)"
      aria-label={t("chat.composer.formatCode")}
      onmousedown={(e) => e.preventDefault()}
      onclick={() => handleFormatClick("`")}
    >
      <Code size={14} />
    </button>
    <button
      class="fmt-btn"
      title="{t('chat.composer.formatSpoiler')}  (⌘⇧P)"
      aria-label={t("chat.composer.formatSpoiler")}
      onmousedown={(e) => e.preventDefault()}
      onclick={() => handleFormatClick("||")}
    >
      <EyeOff size={14} />
    </button>
    <button
      class="fmt-btn"
      title="{t('chat.composer.formatLink')}  (⌘K)"
      aria-label={t("chat.composer.formatLink")}
      onmousedown={(e) => e.preventDefault()}
      onclick={handleLinkClick}
    >
      <Link size={14} />
    </button>
  </div>
{/if}

<style>
  .composer {
    background: color-mix(in srgb, var(--bg-secondary) 80%, transparent);
    backdrop-filter: blur(20px) saturate(140%);
    -webkit-backdrop-filter: blur(20px) saturate(140%);
    padding: 4px 8px;
    position: relative;
    flex-shrink: 0;
    border-radius: 20px;
    border: 1px solid var(--border-light);
    box-shadow:
      0 1px 2px rgba(0, 0, 0, 0.04),
      0 8px 24px rgba(0, 0, 0, 0.08);
    pointer-events: auto;
  }

  .format-bar-floating {
    position: fixed;
    z-index: 1000;
    display: flex;
    align-items: center;
    gap: 2px;
    padding: 3px;
    background: var(--surface);
    backdrop-filter: blur(14px) saturate(140%);
    -webkit-backdrop-filter: blur(14px) saturate(140%);
    border: 1px solid var(--border-light);
    border-radius: 10px;
    box-shadow: 0 6px 20px rgba(0, 0, 0, 0.18);
    width: fit-content;
    animation: fmt-in 160ms cubic-bezier(0.34, 1.56, 0.64, 1);
  }
  @keyframes fmt-in {
    from {
      opacity: 0;
      transform: translateY(4px) scale(0.94);
    }
    to {
      opacity: 1;
      transform: translateY(0) scale(1);
    }
  }
  .fmt-btn {
    width: 28px;
    height: 28px;
    border-radius: 7px;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-secondary);
    transition: all var(--transition);
  }
  .fmt-btn:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }
  .fmt-btn:active {
    transform: scale(0.92);
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
    gap: 2px;
    padding: 0;
    transition: border-color var(--transition);
  }

  .composer-btn {
    width: 32px;
    height: 32px;
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
    padding: 6px 6px;
    resize: none;
    min-height: 32px;
    max-height: 160px;
    font-size: 14.5px;
    line-height: 1.35;
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
    width: 32px;
    height: 32px;
    background: transparent;
    color: var(--text-muted);
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    transition:
      background 200ms cubic-bezier(0.34, 1.56, 0.64, 1),
      color 200ms ease,
      transform 200ms cubic-bezier(0.34, 1.56, 0.64, 1);
    flex-shrink: 0;
    transform: scale(0.9);
    opacity: 0.7;
  }
  .send-btn.ready {
    background: var(--accent);
    color: #fff;
    transform: scale(1);
    opacity: 1;
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
  .send-btn :global(svg) {
    transition: transform 200ms cubic-bezier(0.34, 1.56, 0.64, 1);
  }
  .send-btn.ready :global(svg) {
    transform: translateX(1px);
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
