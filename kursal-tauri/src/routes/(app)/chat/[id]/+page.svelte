<script lang="ts">
  import { page } from "$app/stores";
  import { onMount, tick } from "svelte";
  import { browser } from "$app/environment";
  import { goto } from "$app/navigation";
  import { contactsState } from "$lib/state/contacts.svelte";
  import { messagesState } from "$lib/state/messages.svelte";
  import { profileState } from "$lib/state/profile.svelte";
  import { sendText } from "$lib/api/messages";
  import { shareProfile } from "$lib/api/identity";
  import type { MessageResponse } from "$lib/types";
  import { notifications } from "$lib/state/notifications.svelte";
  import { marked } from "marked";
  import DOMPurify from "dompurify";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { confirm } from "@tauri-apps/plugin-dialog";
  import { ShieldAlert, Send, Reply, Copy, ArrowLeft, X } from "lucide-svelte";
  import Avatar from "$lib/components/Avatar.svelte";
  import StatusDot from "$lib/components/StatusDot.svelte";
  import Spinner from "$lib/components/Spinner.svelte";
  import SecurityCodeModal from "$lib/components/SecurityCodeModal.svelte";
  import ProfileModal from "$lib/components/ProfileModal.svelte";

  const contactId = $derived($page.params.id ?? "");
  const contact = $derived(contactId ? contactsState.getById(contactId) : null);
  const messages = $derived(
    contactId ? messagesState.forContact(contactId) : [],
  );

  let inputText = $state("");
  let sending = $state(false);
  let showSecurityCode = $state(false);
  let showProfile = $state(false);
  let hoveredMessageId = $state<string | null>(null);
  let replyingToMessageId = $state<string | null>(null);
  let isCoarsePointer = $state(false);
  let listEl = $state<HTMLElement | null>(null);
  let composerEl = $state<HTMLTextAreaElement | null>(null);

  // Auto-scroll logic
  let isScrolledToBottom = $state(true);
  let unreadCount = $state(0);
  let prevMessagesLength = $state(0);
  let pendingScrollFrame = 0;
  let lastFocusedBeforeModal = $state<HTMLElement | null>(null);
  const markdownCache = new Map<string, string>();
  const trustedHosts = new Set(["github.com", "docs.rs", "crates.io"]);

  let dismissedBanner = $state(false);
  let showShareBanner = $derived(
    browser && contact && !contact.profileShared && !dismissedBanner,
  );

  async function handleShareQuickProfile() {
    if (!browser || !contactId) return;
    try {
      const name = profileState.displayName;
      let avatarBytes = profileState.avatarBytes;
      // Share profile to this specific peer
      await shareProfile(name, avatarBytes, contactId);

      // Update local state proactively
      if (contact) {
        contactsState.upsert({ ...contact, profileShared: true });
      }
      notifications.push("Profile shared", "success");
    } catch (e) {
      notifications.push("Failed to share profile", "error");
      console.error(e);
    }
  }

  function closeShareBanner() {
    if (browser && contactId) {
      dismissedBanner = true;
    }
  }

  function openProfileModal() {
    lastFocusedBeforeModal = document.activeElement as HTMLElement | null;
    showProfile = true;
  }

  function closeProfileModal() {
    showProfile = false;
    tick().then(() => lastFocusedBeforeModal?.focus());
  }

  function openSecurityCodeModal() {
    lastFocusedBeforeModal = document.activeElement as HTMLElement | null;
    showSecurityCode = true;
  }

  function closeSecurityCodeModal() {
    showSecurityCode = false;
    tick().then(() => lastFocusedBeforeModal?.focus());
  }

  function handleScroll() {
    if (!listEl) return;
    if (pendingScrollFrame) return;
    pendingScrollFrame = window.requestAnimationFrame(() => {
      pendingScrollFrame = 0;
      if (!listEl) return;
      const { scrollTop, scrollHeight, clientHeight } = listEl;
      // 50px threshold for being "at the bottom"
      isScrolledToBottom =
        Math.abs(scrollHeight - scrollTop - clientHeight) < 50;
      if (isScrolledToBottom && unreadCount > 0) {
        unreadCount = 0;
      }
      if (isScrolledToBottom && contactId) {
        messagesState.markRead(contactId);
      }
    });
  }

  function scrollToBottom(behavior: ScrollBehavior = "auto") {
    if (!listEl) return;
    listEl.scrollTo({ top: listEl.scrollHeight, behavior });
    unreadCount = 0;
    isScrolledToBottom = true;
    if (contactId) messagesState.markRead(contactId);
  }

  function renderMarkdown(content: string): string {
    const cached = markdownCache.get(content);
    if (cached) return cached;

    const html = marked.parse(content, {
      async: false,
      gfm: true,
      breaks: true,
    });
    const sanitized = DOMPurify.sanitize(html as string);
    markdownCache.set(content, sanitized);
    if (markdownCache.size > 600) {
      const firstKey = markdownCache.keys().next().value;
      if (firstKey) markdownCache.delete(firstKey);
    }
    return sanitized;
  }

  async function handleMarkdownClick(e: MouseEvent) {
    const target = e.target as HTMLElement | null;
    const anchor = target?.closest("a") as HTMLAnchorElement | null;
    if (!anchor) return;

    e.preventDefault();
    e.stopPropagation();

    const href = anchor.getAttribute("href");
    if (!href) return;

    try {
      const url = new URL(href, window.location.origin);
      if (!["http:", "https:", "mailto:", "tel:"].includes(url.protocol)) {
        notifications.push("Unsupported link type", "error");
        return;
      }

      if (
        ["http:", "https:"].includes(url.protocol) &&
        !trustedHosts.has(url.hostname)
      ) {
        const approved = await confirm(
          `Open external link in browser?\n\nHost: ${url.hostname}\nPath: ${url.pathname || "/"}`,
          { title: "Open External Link", kind: "warning" },
        );
        if (!approved) return;
      }

      await openUrl(url.toString());
    } catch {
      notifications.push("Invalid link", "error");
    }
  }

  const visibleMessages = $derived.by(() => {
    return messages.length > 250
      ? messages.slice(messages.length - 250)
      : messages;
  });

  function getMessagePreview(content: string): string {
    const clean = content.replace(/\s+/g, " ").trim();
    if (!clean) return "(empty message)";
    return clean.length > 92 ? clean.slice(0, 89) + "..." : clean;
  }

  function resizeComposer() {
    if (!composerEl) return;
    composerEl.style.height = "auto";
    composerEl.style.height = `${Math.min(composerEl.scrollHeight, 170)}px`;
  }

  const messageIndex = $derived.by(() => {
    const byId = new Map<string, MessageResponse>();
    for (const msg of messages) {
      byId.set(msg.id, msg);
    }
    return byId;
  });

  const replyingToPreview = $derived(
    replyingToMessageId
      ? getMessagePreview(messageIndex.get(replyingToMessageId)?.content ?? "")
      : "",
  );

  function startReply(message: MessageResponse) {
    replyingToMessageId = message.id;
    tick().then(() => composerEl?.focus());
  }

  function cancelReply() {
    replyingToMessageId = null;
  }

  async function copyMessageText(message: MessageResponse) {
    try {
      await navigator.clipboard.writeText(message.content);
      notifications.push("Message copied", "success");
    } catch {
      notifications.push("Could not copy message", "error");
    }
  }

  onMount(() => {
    isCoarsePointer = window.matchMedia("(pointer: coarse)").matches;

    // Load messages and check if contact exists
    messagesState.loadFor(contactId);

    // After loading, check if contact exists
    if (!contact && !contactsState.loading) {
      // Short delay to allow for state update
      const timer = setTimeout(() => {
        if (contactId && !contactsState.getById(contactId)) {
          notifications.push("Contact not found", "error");
          goto("/chat", { replaceState: true });
        }
      }, 100);
      return () => clearTimeout(timer);
    }

    return () => {
      if (pendingScrollFrame) {
        window.cancelAnimationFrame(pendingScrollFrame);
      }
    };
  });

  $effect(() => {
    if (!listEl) return;
    const handleClick = (e: Event) => {
      void handleMarkdownClick(e as MouseEvent);
    };
    listEl.addEventListener("click", handleClick);
    return () => {
      listEl?.removeEventListener("click", handleClick);
    };
  });

  // Scroll to bottom whenever messages change
  $effect(() => {
    const len = messages.length;
    if (len > prevMessagesLength) {
      const diff = len - prevMessagesLength;
      tick().then(() => {
        const lastMsg = messages[len - 1];
        if (lastMsg?.direction === "sent" || isScrolledToBottom) {
          scrollToBottom(isScrolledToBottom ? "auto" : "smooth");
        } else {
          unreadCount += diff;
        }
      });
    } else if (len < prevMessagesLength) {
      // Something was deleted, maybe a reaction length changed, just ignore
    }
    prevMessagesLength = len;
  });

  $effect(() => {
    if (contactId && isScrolledToBottom) {
      messagesState.markRead(contactId);
    }
  });

  $effect(() => {
    inputText;
    tick().then(resizeComposer);
  });

  async function handleSend() {
    const text = inputText.trim();
    if (!text) return;

    const replyTo = replyingToMessageId;
    inputText = "";
    sending = true;

    try {
      if (!contactId) return;
      const messageId = await sendText(contactId, text, replyTo);
      // Optimistically add to UI
      messagesState.appendOptimistic({
        id: messageId,
        contactId,
        direction: "sent",
        content: text,
        status: "sending",
        timestamp: Date.now(),
        replyTo,
      });
      cancelReply();
    } catch (e) {
      inputText = text; // restore text on error
      notifications.push("Failed to send message", "error");
      console.error("Send failed:", e);
    } finally {
      sending = false;
    }
  }

  async function handleResend(msg: import("$lib/types").MessageResponse) {
    if (!contactId) return;

    // Attempt sending again
    messagesState.removeLocally(msg.id, msg.contactId);

    const text = msg.content;
    const replyTo = msg.replyTo;

    try {
      const messageId = await sendText(contactId, text, replyTo);
      messagesState.appendOptimistic({
        id: messageId,
        contactId,
        direction: "sent",
        content: text,
        status: "sending",
        timestamp: Date.now(),
        replyTo,
      });
    } catch (e) {
      notifications.push("Failed to resend message", "error");
      console.error("Resend failed:", e);
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape" && replyingToMessageId) {
      e.preventDefault();
      cancelReply();
      return;
    }

    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  }
</script>

{#if contact}
  <div class="chat-container">
    <!-- Header -->
    <div class="header">
      <div class="header-left">
        <button
          class="mobile-back-btn"
          onclick={() => goto("/chat")}
          title="Back to contacts"
        >
          <ArrowLeft size={20} />
        </button>
        <Avatar
          name={contact.displayName}
          src={contact.avatarBase64}
          size={34}
        />
        <button
          class="header-info"
          onclick={openProfileModal}
          aria-label="Open contact profile"
        >
          <div class="header-name-row">
            <h2>{contact.displayName}</h2>
            <StatusDot
              status={contactsState.connectionStatus[contact.userId] ??
                "disconnected"}
            />
          </div>
          <p class="peer-id">ID: {contact.userId.slice(0, 12)}...</p>
        </button>
      </div>
      <div class="header-right">
        {#if !contact.verified}
          <button
            class="verify-btn"
            title="Verify contact"
            aria-label="Verify contact"
            onclick={openSecurityCodeModal}
          >
            <ShieldAlert size={16} />
          </button>
        {/if}
      </div>
    </div>

    {#if showShareBanner}
      <div class="share-banner">
        <span
          >Share your display name and photo with {contact?.displayName}?</span
        >
        <div class="banner-actions">
          <button class="btn-primary btn-sm" onclick={handleShareQuickProfile}
            >Share</button
          >
          <button
            class="icon-btn"
            onclick={closeShareBanner}
            aria-label="Close"
          >
            <X size={16} />
          </button>
        </div>
      </div>
    {/if}

    <!-- Messages -->
    <div class="messages" bind:this={listEl} onscroll={handleScroll}>
      {#if visibleMessages.length === 0}
        <div class="no-messages">No messages yet. Say hello!</div>
      {:else}
        {#each visibleMessages as msg (msg.id)}
          {@const repliedMessage = msg.replyTo
            ? messageIndex.get(msg.replyTo)
            : null}
          <div
            class="message-row"
            class:sent={msg.direction === "sent"}
            role="group"
            data-msg-id={msg.id}
            onmouseenter={() => (hoveredMessageId = msg.id)}
            onmouseleave={() => (hoveredMessageId = null)}
          >
            <div class="message-bubble">
              {#if msg.replyTo}
                <div class="reply-reference">
                  <span class="reply-pill">Reply</span>
                  <span class="reply-content">
                    {#if repliedMessage}
                      {getMessagePreview(repliedMessage.content)}
                    {:else}
                      Original message
                    {/if}
                  </span>
                </div>
              {/if}

              <div class="selectable markdown-content">
                {@html renderMarkdown(msg.content)}
              </div>

              {#if messagesState.reactionsFor(msg.id, msg.contactId).length > 0}
                <div class="message-reactions">
                  {#each messagesState.reactionsFor(msg.id, msg.contactId) as emoji}
                    <span class="reaction-chip">{emoji}</span>
                  {/each}
                </div>
              {/if}

              <div class="message-meta">
                <div class="message-time">
                  {new Date(msg.timestamp).toLocaleTimeString()}
                </div>
                {#if msg.direction === "sent" && msg.status !== "delivered"}
                  <div
                    class="status-indicator"
                    class:failed={msg.status === "failed"}
                  >
                    <span class="status">
                      {msg.status === "sending"
                        ? "Sending..."
                        : "Failed to send"}
                    </span>
                    {#if msg.status === "failed"}
                      <div class="failed-actions">
                        <button
                          type="button"
                          class="failed-btn"
                          onclick={() => handleResend(msg)}>Try again</button
                        >
                        <span class="failed-divider">•</span>
                        <button
                          type="button"
                          class="failed-btn"
                          onclick={() =>
                            messagesState.removeLocally(msg.id, msg.contactId)}
                          >Delete</button
                        >
                      </div>
                    {/if}
                  </div>
                {/if}
              </div>
            </div>

            {#if hoveredMessageId === msg.id || isCoarsePointer}
              <div class="message-actions">
                <button
                  class="action-btn"
                  type="button"
                  onclick={() => startReply(msg)}><Reply size={14} /></button
                >
                <button
                  class="action-btn"
                  type="button"
                  onclick={() => copyMessageText(msg)}
                  ><Copy size={14} /></button
                >
              </div>
            {/if}
          </div>
        {/each}
      {/if}
    </div>

    <!-- Input -->
    <div class="input-area">
      {#if unreadCount > 0 && !isScrolledToBottom}
        <button class="unread-banner" onclick={() => scrollToBottom("smooth")}>
          {unreadCount} new message{unreadCount === 1 ? "" : "s"}
        </button>
      {/if}

      {#if contact.blocked}
        <div class="blocked-message">
          <p>You cannot send messages to a blocked contact.</p>
          <button class="unblock-btn" onclick={openProfileModal}
            >Profile Settings</button
          >
        </div>
      {:else}
        {#if replyingToMessageId}
          <div class="replying-banner">
            <div class="replying-label">Replying to message</div>
            <div class="replying-preview">{replyingToPreview}</div>
            <button class="reply-cancel" type="button" onclick={cancelReply}
              >Cancel</button
            >
          </div>
        {/if}

        <textarea
          bind:this={composerEl}
          bind:value={inputText}
          oninput={resizeComposer}
          onkeydown={handleKeydown}
          placeholder="Type a message..."
          rows="1"
          disabled={sending}
        ></textarea>
        <div class="send-button">
          {#if sending}
            <Spinner size={18} color="var(--accent)" />
          {:else}
            <button onclick={handleSend} disabled={!inputText.trim()}>
              <Send size={18} />
            </button>
          {/if}
        </div>
      {/if}
    </div>
  </div>

  {#if showSecurityCode && contact}
    <SecurityCodeModal
      contactId={contact.userId}
      onClose={closeSecurityCodeModal}
    />
  {/if}

  {#if showProfile && contact}
    <ProfileModal {contact} onClose={closeProfileModal} />
  {/if}
{:else}
  <div class="loading">
    <Spinner />
  </div>
{/if}

<style>
  .chat-container {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }

  .share-banner {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px 18px;
    background: var(--surface);
    border-bottom: 1px solid var(--border);
    font-size: 0.85rem;
    color: var(--text-light);
  }

  .banner-actions {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .banner-actions .btn-sm {
    padding: 4px 10px;
    font-size: 0.8rem;
    border-radius: 4px;
  }

  .mobile-back-btn {
    display: none;
    align-items: center;
    justify-content: center;
    width: 32px;
    height: 32px;
    margin-right: -4px;
    border-radius: 8px;
    transition: background var(--transition);
  }

  .mobile-back-btn:hover {
    background: rgba(255, 255, 255, 0.1);
  }

  @media (max-width: 768px) {
    .mobile-back-btn {
      display: flex;
    }
  }

  .header {
    min-height: var(--header-height);
    border-bottom: 1px solid var(--border);
    padding: 0 18px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    background: rgba(15, 23, 42, 0.5);
    backdrop-filter: blur(20px);
  }

  .header-right {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .header-name-row {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .header-left {
    display: flex;
    align-items: center;
    gap: 14px;
  }

  .header-info {
    text-align: left;
    background: transparent;
    border: none;
    cursor: pointer;
    border-radius: 8px;
    padding: 4px 6px;
    margin-left: -6px;
    transition: background var(--transition);
  }

  .header-info:hover {
    background: rgba(255, 255, 255, 0.05);
  }

  .header-info h2 {
    margin: 0;
    font-size: 17px;
    font-weight: 700;
    letter-spacing: -0.01em;
  }

  .peer-id {
    margin: 0;
    font-size: 12px;
    color: var(--text-muted);
  }

  .verify-btn {
    width: 36px;
    height: 36px;
    border-radius: 10px;
    border: 1px solid rgba(251, 191, 36, 0.45);
    cursor: pointer;
    background: rgba(251, 191, 36, 0.1);
    color: var(--warning);
    transition: all var(--transition);
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }

  .verify-btn:hover {
    background: rgba(251, 191, 36, 0.2);
  }

  .messages {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 18px 18px 14px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .no-messages {
    margin: auto;
    text-align: center;
    color: var(--text-muted);
    font-size: 14px;
    border: 1px dashed rgba(148, 163, 184, 0.28);
    border-radius: 18px;
    padding: 14px 20px;
    background: rgba(30, 41, 59, 0.46);
  }

  .message-row {
    display: flex;
    gap: 8px;
    align-items: flex-end;
    align-self: flex-start;
    max-width: 100%;
    content-visibility: auto;
    contain-intrinsic-size: 72px;
  }

  .message-row.sent {
    align-self: flex-end;
    flex-direction: row-reverse;
  }

  .message-bubble {
    background: rgba(30, 41, 59, 0.58);
    border: 1px solid rgba(148, 163, 184, 0.24);
    border-radius: 14px;
    padding: 10px 14px;
    font-size: 14.5px;
    line-height: 1.45;
    max-width: min(80vw, 680px);
    width: fit-content;
    word-break: break-word;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .markdown-content :global(p) {
    margin: 0;
  }

  .markdown-content :global(p + p) {
    margin-top: 0.45rem;
  }

  .markdown-content :global(code) {
    font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas,
      "Liberation Mono", "Courier New", monospace;
    font-size: 0.9em;
    background: rgba(2, 6, 23, 0.38);
    border: 1px solid rgba(148, 163, 184, 0.24);
    border-radius: 6px;
    padding: 0.08rem 0.3rem;
  }

  .markdown-content :global(pre) {
    margin: 0.45rem 0 0;
    padding: 0.6rem 0.7rem;
    border-radius: 10px;
    border: 1px solid rgba(148, 163, 184, 0.24);
    background: rgba(2, 6, 23, 0.45);
    overflow-x: auto;
  }

  .markdown-content :global(pre code) {
    border: none;
    background: transparent;
    padding: 0;
  }

  .markdown-content :global(ul),
  .markdown-content :global(ol) {
    margin: 0.45rem 0 0;
    padding-left: 1.2rem;
  }

  .markdown-content :global(a) {
    color: inherit;
    text-decoration: underline;
  }

  .message-row.sent .message-bubble {
    background: linear-gradient(
      135deg,
      rgba(67, 56, 202, 0.9),
      rgba(79, 70, 229, 0.86)
    );
    color: #eef2ff;
    border-color: rgba(129, 140, 248, 0.55);
  }

  .message-row.sent:has(.status-indicator:not(.failed)) .message-bubble {
    background: rgba(30, 41, 59, 0.8);
    border-color: rgba(148, 163, 184, 0.3);
    color: var(--text-secondary);
  }

  .message-row.sent:has(.status-indicator.failed) .message-bubble {
    background: rgba(153, 27, 27, 0.4);
    border-color: rgba(220, 38, 38, 0.5);
  }

  .reply-reference {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
    border-left: 2px solid rgba(129, 140, 248, 0.55);
    padding: 4px 8px;
    border-radius: 8px;
    background: rgba(15, 23, 42, 0.35);
  }

  .reply-pill {
    font-weight: 700;
    color: rgba(199, 210, 254, 0.95);
  }

  .reply-content {
    color: rgba(224, 231, 255, 0.82);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .message-meta {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .message-time {
    font-size: 11px;
    color: var(--text-muted);
  }

  .message-row.sent .message-time {
    color: rgba(255, 255, 255, 0.5);
  }

  .message-reactions {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .reaction-chip {
    border: 1px solid rgba(148, 163, 184, 0.3);
    background: rgba(15, 23, 42, 0.5);
    border-radius: 999px;
    padding: 2px 8px;
    font-size: 13px;
    line-height: 1.2;
  }

  .status {
    font-size: 12px;
    color: rgba(255, 255, 255, 0.65);
  }

  .status-indicator {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 2px;
  }

  .failed-actions {
    display: flex;
    gap: 6px;
    align-items: center;
    font-size: 11px;
  }

  .failed-btn {
    background: none;
    border: none;
    padding: 0;
    color: var(--danger);
    cursor: pointer;
    font-weight: 600;
  }

  .failed-btn:hover {
    text-decoration: underline;
  }

  .failed-divider {
    color: rgba(255, 255, 255, 0.3);
  }

  .message-actions {
    display: flex;
    gap: 6px;
    margin-bottom: 4px;
  }

  .action-btn {
    border: 1px solid rgba(148, 163, 184, 0.28);
    background: rgba(15, 23, 42, 0.82);
    color: rgba(226, 232, 240, 0.92);
    border-radius: 999px;
    font-size: 12px;
    font-weight: 600;
    padding: 4px 10px;
    cursor: pointer;
    transition: all var(--transition);
  }

  .action-btn:hover {
    background: rgba(67, 56, 202, 0.38);
    border-color: rgba(129, 140, 248, 0.5);
  }

  .input-area {
    padding: 14px 16px;
    background: linear-gradient(
      180deg,
      rgba(30, 41, 59, 0.82),
      rgba(15, 23, 42, 0.95)
    );
    border-top: 1px solid var(--border);
    display: grid;
    grid-template-columns: 1fr auto;
    gap: 10px;
    align-items: end;
    position: relative;
  }

  .unread-banner {
    position: absolute;
    top: -46px;
    right: 24px;
    background: var(--accent);
    color: #fff;
    padding: 6px 14px;
    border-radius: 999px;
    font-size: 13px;
    font-weight: 600;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.2);
    animation: slideUp 0.2s ease-out;
    z-index: 10;
  }

  @keyframes slideUp {
    from {
      opacity: 0;
      transform: translateY(10px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }

  .replying-banner {
    grid-column: 1 / -1;
    display: grid;
    grid-template-columns: auto 1fr auto;
    gap: 10px;
    align-items: center;
    background: rgba(15, 23, 42, 0.6);
    border: 1px solid rgba(129, 140, 248, 0.35);
    border-radius: 12px;
    padding: 8px 10px;
  }

  .replying-label {
    font-size: 12px;
    font-weight: 700;
    color: rgba(199, 210, 254, 0.9);
  }

  .replying-preview {
    font-size: 12px;
    color: rgba(226, 232, 240, 0.82);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .reply-cancel {
    background: transparent;
    border: 1px solid rgba(248, 113, 113, 0.42);
    color: rgba(252, 165, 165, 0.95);
    border-radius: 999px;
    font-size: 11px;
    font-weight: 700;
    padding: 4px 10px;
    cursor: pointer;
  }

  textarea {
    width: 100%;
    background: rgba(15, 23, 42, 0.9);
    border: 1px solid var(--border);
    border-radius: 14px;
    color: var(--text-primary);
    padding: 12px 13px;
    resize: none;
    min-height: 44px;
    max-height: 170px;
    font-size: 14px;
  }

  textarea:focus {
    outline: none;
    border-color: rgba(129, 140, 248, 0.68);
    box-shadow: 0 0 0 2px rgba(129, 140, 248, 0.24);
  }

  textarea:disabled {
    opacity: 0.5;
  }

  .send-button {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 42px;
    height: 42px;
  }

  .send-button button {
    width: 100%;
    height: 100%;
    background: linear-gradient(135deg, #6366f1, #7c83f6);
    color: #eef2ff;
    border-radius: 12px;
    cursor: pointer;
    transition:
      transform var(--transition),
      filter var(--transition);
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }

  .send-button button:hover:not(:disabled) {
    filter: brightness(1.05);
    transform: translateY(-1px);
  }

  .blocked-message {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 8px;
    padding: 16px;
    width: 100%;
    text-align: center;
    color: var(--text-muted);
    font-size: 14px;
    background: rgba(15, 23, 42, 0.5);
    border-radius: var(--radius-md);
  }

  .unblock-btn {
    background: rgba(129, 140, 248, 0.1);
    color: var(--accent);
    padding: 6px 12px;
    border-radius: var(--radius-sm);
    font-size: 13px;
    font-weight: 600;
    transition: background 0.2s;
  }

  .unblock-btn:hover {
    background: rgba(129, 140, 248, 0.2);
  }

  .send-button button:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .loading {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  @media (max-width: 900px) {
    .message-bubble {
      max-width: 90vw;
    }

    .message-actions {
      opacity: 1;
    }
  }
</style>
