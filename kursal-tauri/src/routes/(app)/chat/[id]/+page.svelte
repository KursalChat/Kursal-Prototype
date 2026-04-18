<script lang="ts">
  import { page } from "$app/state";
  import { onMount, tick } from "svelte";
  import { browser } from "$app/environment";
  import { goto } from "$app/navigation";
  import { stat } from "@tauri-apps/plugin-fs";
  import { contactsState } from "$lib/state/contacts.svelte";
  import { messagesState } from "$lib/state/messages.svelte";
  import { profileState } from "$lib/state/profile.svelte";
  import {
    sendText,
    sendFileOffer,
    acceptFileOffer,
    deleteLocalMessage,
    deleteMessage,
    editMessage,
    addReaction,
    removeReaction,
    sendTypingIndicator,
  } from "$lib/api/messages";
  import { shareProfile } from "$lib/api/identity";
  import {
    pickFileForSend,
    pickFileForReceive,
    prepareOfferSourcePath,
    registerDeferredReceiveTarget,
  } from "$lib/utils/file-transfer-paths";
  import type { MessageResponse } from "$lib/types";
  import { notifications } from "$lib/state/notifications.svelte";
  import { confirm } from "@tauri-apps/plugin-dialog";
  import { ShieldAlert, X, Paperclip, ChevronDown } from "lucide-svelte";
  import Avatar from "$lib/components/Avatar.svelte";
  import Spinner from "$lib/components/Spinner.svelte";
  import SecurityCodeModal from "$lib/components/SecurityCodeModal.svelte";
  import ProfileModal from "$lib/components/ProfileModal.svelte";
  import ChatHeader from "./ChatHeader.svelte";
  import MessageBubble from "./MessageBubble.svelte";
  import MessageComposer from "./MessageComposer.svelte";
  import ActionSheet from "./ActionSheet.svelte";
  import FileConfirmModal from "./FileConfirmModal.svelte";
  import EmojiPicker from "$lib/components/EmojiPicker.svelte";
  import {
    formatGroupTime,
    getMessagePreview,
    handleMarkdownClick,
  } from "./chat-utils";

  const contactId = $derived(page.params.id ?? "");
  const contact = $derived(contactId ? contactsState.getById(contactId) : null);
  const messages = $derived(
    contactId ? messagesState.forContact(contactId) : [],
  );
  const firstUnreadId = $derived(
    contactId ? messagesState.firstUnreadFor(contactId) : null,
  );

  let inputText = $state("");
  let sending = $state(false);
  let showSecurityCode = $state(false);
  let showProfile = $state(false);
  let hoveredMessageId = $state<string | null>(null);
  let replyingToMessageId = $state<string | null>(null);
  let editingMessageId = $state<string | null>(null);
  let showEmojiPicker = $state<string | null>(null);
  let emojiPickerAnchor = $state<DOMRect | null>(null);
  let actionSheetMsgId = $state<string | null>(null);
  let fileOfferActionState = $state<
    Record<string, "idle" | "accepting" | "accepted">
  >({});
  const completedFileTimers = new Map<string, ReturnType<typeof setTimeout>>();
  let isCoarsePointer = $state(false);
  let listEl = $state<HTMLElement | null>(null);
  let composerEl = $state<HTMLTextAreaElement | null>(null);

  let isScrolledToBottom = $state(true);
  let isAtMaxBottom = $state(true);
  let unreadCount = $state(0);
  let isDraggingFile = $state(false);
  let pendingFile = $state<{
    backendPath: string;
    filename: string;
    sizeBytes: number;
  } | null>(null);
  let sendingFile = $state(false);
  let unlistenDrop: (() => void) | null = null;
  let prevMessagesLength = $state(0);
  let pendingScrollFrame = 0;
  let windowFocused = $state(true);
  let lastFocusedBeforeModal = $state<HTMLElement | null>(null);

  let swipeStart = { x: 0, y: 0, id: "" };
  let swipeOffset = $state<{ id: string; dx: number } | null>(null);

  let dismissedBanner = $state(false);
  let showShareBanner = $derived(
    browser && contact && !contact.profileShared && !dismissedBanner,
  );

  async function handleShareQuickProfile() {
    if (!browser || !contactId) return;
    try {
      await shareProfile(
        profileState.displayName,
        profileState.avatarBytes,
        contactId,
      );
      if (contact) contactsState.upsert({ ...contact, profileShared: true });
      notifications.push("Profile shared", "success");
    } catch (e) {
      notifications.push("Failed to share profile", "error");
      console.error(e);
    }
  }

  function closeShareBanner() {
    dismissedBanner = true;
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
    if (showEmojiPicker) {
      showEmojiPicker = null;
      emojiPickerAnchor = null;
    }
    if (!listEl || pendingScrollFrame) return;
    pendingScrollFrame = window.requestAnimationFrame(() => {
      pendingScrollFrame = 0;
      if (!listEl) return;
      const { scrollTop, scrollHeight, clientHeight } = listEl;
      const distFromBottom = scrollHeight - scrollTop - clientHeight;
      isScrolledToBottom = Math.abs(distFromBottom) < 60;
      isAtMaxBottom = distFromBottom <= 2;
      if (isScrolledToBottom && unreadCount > 0) unreadCount = 0;
      if (isAtMaxBottom && contactId && windowFocused)
        messagesState.markRead(contactId);
    });
  }

  const PICKER_W = 288;
  const PICKER_H = 320;
  const emojiPickerPos = $derived.by(() => {
    if (!emojiPickerAnchor) return null;
    const r = emojiPickerAnchor;
    const vw = window.innerWidth;
    const vh = window.innerHeight;
    const margin = 8;
    const spaceAbove = r.top;
    const spaceBelow = vh - r.bottom;
    let top: number;
    if (spaceAbove >= PICKER_H + margin) {
      top = r.top - PICKER_H - margin;
    } else if (spaceBelow >= PICKER_H + margin) {
      top = r.bottom + margin;
    } else {
      top = Math.max(margin, vh - PICKER_H - margin);
    }
    let left = r.left + r.width / 2 - PICKER_W / 2;
    left = Math.max(margin, Math.min(left, vw - PICKER_W - margin));
    return { top, left };
  });

  async function handlePickerSelect(emoji: string) {
    if (!showEmojiPicker) return;
    const msg = messageIndex.get(showEmojiPicker);
    if (msg) await toggleReaction(msg, emoji);
    showEmojiPicker = null;
    emojiPickerAnchor = null;
  }
  function closePicker() {
    showEmojiPicker = null;
    emojiPickerAnchor = null;
  }

  function scrollToBottom(behavior: ScrollBehavior = "auto") {
    if (!listEl) return;
    listEl.scrollTo({ top: listEl.scrollHeight, behavior });
    unreadCount = 0;
    isScrolledToBottom = true;
    isAtMaxBottom = true;
    if (contactId && windowFocused) messagesState.markRead(contactId);
  }

  const visibleMessages = $derived.by(() =>
    messages.length > 300 ? messages.slice(messages.length - 300) : messages,
  );

  interface MessageGroup {
    direction: "sent" | "received";
    messages: MessageResponse[];
    timestamp: number;
  }

  const messageGroups = $derived.by(() => {
    const groups: MessageGroup[] = [];
    for (const msg of visibleMessages) {
      const last = groups[groups.length - 1];
      const forceBreak = msg.id === firstUnreadId;
      if (
        !forceBreak &&
        last &&
        last.direction === msg.direction &&
        msg.timestamp - last.messages[last.messages.length - 1].timestamp <
          300000
      ) {
        last.messages.push(msg);
      } else {
        groups.push({
          direction: msg.direction,
          messages: [msg],
          timestamp: msg.timestamp,
        });
      }
    }
    return groups;
  });

  function transferPercent(transferId: string): number {
    const p = messagesState.transferProgressFor(transferId);
    if (!p || p.totalBytes <= 0) return 0;
    return Math.max(
      0,
      Math.min(100, Math.round((p.bytesTransferred / p.totalBytes) * 100)),
    );
  }
  function isTransferDone(transferId: string): boolean {
    const p = messagesState.transferProgressFor(transferId);
    if (!p || p.totalBytes <= 0) return false;
    return p.bytesTransferred >= p.totalBytes;
  }

  function resizeComposer() {
    if (!composerEl) return;
    composerEl.style.height = "auto";
    composerEl.style.height = `${Math.min(composerEl.scrollHeight, 180)}px`;
  }

  let lastTypingSentAt = 0;
  function maybeSendTyping() {
    if (!contactId) return;
    if (!inputText.trim()) {
      lastTypingSentAt = 0;
      return;
    }
    const now = Date.now();
    if (now - lastTypingSentAt < 7000) return;
    lastTypingSentAt = now;
    void sendTypingIndicator(contactId, replyingToMessageId).catch((e) => {
      console.error("send_typing_indicator failed", e);
    });
  }

  function onComposerInput() {
    resizeComposer();
    maybeSendTyping();
  }

  const messageIndex = $derived.by(() => {
    const byId = new Map<string, MessageResponse>();
    for (const msg of messages) byId.set(msg.id, msg);
    return byId;
  });

  const replyingToPreview = $derived(
    replyingToMessageId
      ? getMessagePreview(messageIndex.get(replyingToMessageId)?.content ?? "")
      : "",
  );
  const editingPreview = $derived(
    editingMessageId
      ? getMessagePreview(messageIndex.get(editingMessageId)?.content ?? "")
      : "",
  );

  function startReply(msg: MessageResponse) {
    replyingToMessageId = msg.id;
    editingMessageId = null;
    actionSheetMsgId = null;
    tick().then(() => composerEl?.focus());
  }
  function startEdit(msg: MessageResponse) {
    editingMessageId = msg.id;
    replyingToMessageId = null;
    inputText = msg.content;
    actionSheetMsgId = null;
    tick().then(() => composerEl?.focus());
  }
  function cancelEdit() {
    editingMessageId = null;
    inputText = "";
  }
  function cancelReply() {
    replyingToMessageId = null;
  }

  async function handleDelete(msg: MessageResponse) {
    actionSheetMsgId = null;
    if (
      await confirm("Delete this message for everyone?", {
        title: "Delete message",
        kind: "warning",
      })
    ) {
      try {
        await deleteMessage(contactId, msg.id);
        messagesState.markDeleted(msg.id, contactId);
      } catch (e) {
        console.error("Delete failed", e);
        notifications.push("Failed to delete message", "error");
      }
    }
  }

  async function toggleReaction(msg: MessageResponse, emoji: string) {
    try {
      const reactions = messagesState.reactionsFor(msg.id, contactId);
      const isReacted = reactions
        .find((r) => r.emoji === emoji)
        ?.userIds.includes(profileState.userId || "");
      if (isReacted) {
        await removeReaction(contactId, msg.id, emoji);
        messagesState.removeReaction(
          msg.id,
          contactId,
          emoji,
          profileState.userId || "",
        );
      } else {
        await addReaction(contactId, msg.id, emoji);
        messagesState.addReaction(
          msg.id,
          contactId,
          emoji,
          profileState.userId || "",
        );
      }
    } catch (e) {
      console.error("React failed", e);
    }
    showEmojiPicker = null;
    actionSheetMsgId = null;
  }

  async function copyMessageText(msg: MessageResponse) {
    actionSheetMsgId = null;
    try {
      await navigator.clipboard.writeText(msg.content);
      notifications.push("Copied", "success");
    } catch {
      notifications.push("Could not copy", "error");
    }
  }

  function handleReplyRefClick(replyToId: string) {
    const el = listEl?.querySelector(`[data-msg-id="${replyToId}"]`);
    el?.scrollIntoView({ behavior: "smooth", block: "center" });
    (el as HTMLElement | null)?.classList.add("flash");
    setTimeout(
      () => (el as HTMLElement | null)?.classList.remove("flash"),
      1200,
    );
  }

  async function handleDeleteLocal(msg: MessageResponse) {
    try {
      await deleteLocalMessage(msg.contactId, msg.id);
      messagesState.removeLocally(msg.id, msg.contactId);
    } catch (e) {
      notifications.push("Failed to delete", "error");
    }
  }

  onMount(() => {
    isCoarsePointer = window.matchMedia("(pointer: coarse)").matches;
    windowFocused =
      document.hasFocus() && document.visibilityState === "visible";

    const onFocus = () => {
      windowFocused = document.visibilityState === "visible";
    };
    const onBlur = () => {
      windowFocused = false;
    };
    const onVisibility = () => {
      windowFocused =
        document.visibilityState === "visible" && document.hasFocus();
    };
    window.addEventListener("focus", onFocus);
    window.addEventListener("blur", onBlur);
    document.addEventListener("visibilitychange", onVisibility);

    const onGlobalKey = (e: KeyboardEvent) => {
      if (!composerEl) return;
      if (e.metaKey || e.ctrlKey || e.altKey) return;
      if (e.key.length !== 1) return;
      const ae = document.activeElement;
      if (ae === composerEl) return;
      if (ae instanceof HTMLInputElement || ae instanceof HTMLTextAreaElement)
        return;
      if (ae instanceof HTMLElement && ae.isContentEditable) return;
      composerEl.focus();
    };
    window.addEventListener("keydown", onGlobalKey);

    void (async () => {
      try {
        const { getCurrentWebview } = await import("@tauri-apps/api/webview");
        unlistenDrop = await getCurrentWebview().onDragDropEvent((event) => {
          const p = event.payload;
          if (p.type === "enter" || p.type === "over") {
            isDraggingFile = true;
          } else if (p.type === "leave") {
            isDraggingFile = false;
          } else if (p.type === "drop") {
            isDraggingFile = false;
            const paths = (p as { paths?: string[] }).paths ?? [];
            void handleDroppedPaths(paths);
          }
        });
      } catch (e) {
        console.warn("Drag/drop not available:", e);
      }
    })();

    let contactMissingTimer: ReturnType<typeof setTimeout> | null = null;
    if (!contact && !contactsState.loading) {
      contactMissingTimer = setTimeout(() => {
        if (contactId && !contactsState.getById(contactId)) {
          notifications.push("Contact not found", "error");
          goto("/chat", { replaceState: true });
        }
      }, 100);
    }

    return () => {
      window.removeEventListener("focus", onFocus);
      window.removeEventListener("blur", onBlur);
      window.removeEventListener("keydown", onGlobalKey);
      document.removeEventListener("visibilitychange", onVisibility);
      if (contactMissingTimer) clearTimeout(contactMissingTimer);
      if (pendingScrollFrame) window.cancelAnimationFrame(pendingScrollFrame);
      for (const t of completedFileTimers.values()) clearTimeout(t);
      completedFileTimers.clear();
      unlistenDrop?.();
      unlistenDrop = null;
    };
  });

  $effect(() => {
    const id = contactId;
    if (!id) return;
    void messagesState.loadFor(id).then(() => {
      if (id !== contactId) return;
      const unread = messagesState.unreadFor(id);
      if (unread > 0 && !messagesState.firstUnreadFor(id)) {
        const list = messagesState.forContact(id);
        const idx = Math.max(0, list.length - unread);
        const first = list.slice(idx).find((m) => m.direction === "received");
        if (first) messagesState.setFirstUnread(id, first.id);
      }
    });
    return () => {
      messagesState.clearFirstUnread(id);
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
    }
    prevMessagesLength = len;
  });

  $effect(() => {
    if (contactId && isAtMaxBottom && windowFocused)
      messagesState.markRead(contactId);
  });

  $effect(() => {
    for (const [msgId, st] of Object.entries(fileOfferActionState)) {
      if (st !== "accepted") continue;
      if (!isTransferDone(msgId)) continue;
      if (completedFileTimers.has(msgId)) continue;
      const timer = setTimeout(() => {
        fileOfferActionState[msgId] = "idle";
        completedFileTimers.delete(msgId);
      }, 8000);
      completedFileTimers.set(msgId, timer);
    }
  });

  $effect(() => {
    const id = contactId;
    const unreadId = firstUnreadId;
    const _len = messages.length;
    if (!id || !unreadId) return;
    if (!windowFocused || !isAtMaxBottom) return;
    const timer = setTimeout(() => {
      messagesState.clearFirstUnread(id);
    }, 2000);
    return () => clearTimeout(timer);
  });

  $effect(() => {
    inputText;
    tick().then(() => {
      resizeComposer();
      if (isScrolledToBottom && listEl) {
        listEl.scrollTop = listEl.scrollHeight;
      }
    });
  });

  async function handleSend() {
    const text = inputText.trim();
    if (!text) return;

    if (editingMessageId) {
      if (!contactId) return;
      sending = true;
      try {
        await editMessage(contactId, editingMessageId, text);
        messagesState.updateContent(editingMessageId, contactId, text);
        inputText = "";
        editingMessageId = null;
      } catch (e) {
        notifications.push("Failed to edit message", "error");
        console.error("Edit failed:", e);
      } finally {
        sending = false;
      }
      return;
    }

    const replyTo = replyingToMessageId;
    inputText = "";
    lastTypingSentAt = 0;
    sending = true;
    try {
      if (!contactId) return;
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
      cancelReply();
    } catch (e) {
      inputText = text;
      notifications.push("Failed to send message", "error");
      console.error("Send failed:", e);
    } finally {
      sending = false;
    }
  }

  async function stageFileForSend(backendPath: string, filename: string) {
    let sizeBytes = 0;
    try {
      const info = await stat(backendPath);
      sizeBytes = Number(info.size ?? 0);
    } catch {
      sizeBytes = 0;
    }
    pendingFile = { backendPath, filename, sizeBytes };
  }

  async function handleSendFile() {
    try {
      const prepared = await pickFileForSend();
      if (!prepared) return;
      await stageFileForSend(prepared.backendPath, prepared.filename);
    } catch (e) {
      notifications.push(
        `Failed to open file: ${e instanceof Error ? e.message : e}`,
        "error",
      );
      console.error("File pick failed:", e);
    }
  }

  async function handleDroppedPaths(paths: string[]) {
    if (!paths?.length || !contactId) return;
    try {
      const prepared = await prepareOfferSourcePath(paths[0]);
      await stageFileForSend(prepared.backendPath, prepared.filename);
    } catch (e) {
      notifications.push(
        `Failed to prepare file: ${e instanceof Error ? e.message : e}`,
        "error",
      );
      console.error("Drop handler failed:", e);
    }
  }

  async function confirmSendFile() {
    if (!pendingFile || !contactId || sendingFile) return;
    const file = pendingFile;
    sendingFile = true;
    try {
      const [messageId, fileSize] = await sendFileOffer(
        contactId,
        file.backendPath,
      );
      messagesState.appendOptimistic({
        id: messageId,
        contactId,
        direction: "sent",
        content: "[File]",
        status: "sending",
        timestamp: Date.now(),
        replyTo: null,
        fileDetails: { filename: file.filename, sizeBytes: fileSize },
      });
      pendingFile = null;
    } catch (e) {
      notifications.push(
        `Failed to send file: ${e instanceof Error ? e.message : e}`,
        "error",
      );
      console.error("File send failed:", e);
    } finally {
      sendingFile = false;
    }
  }

  function cancelSendFile() {
    if (sendingFile) return;
    pendingFile = null;
  }

  async function handleAcceptIncomingFile(msg: MessageResponse) {
    if (!contactId || !msg.fileDetails) return;
    if (
      fileOfferActionState[msg.id] === "accepting" ||
      fileOfferActionState[msg.id] === "accepted"
    )
      return;
    fileOfferActionState[msg.id] = "accepting";
    try {
      const resolved = await pickFileForReceive(msg.fileDetails.filename);
      if (!resolved) {
        fileOfferActionState[msg.id] = "idle";
        return;
      }
      await acceptFileOffer(msg.contactId, msg.id, resolved.backendPath);
      if (resolved.deferredTargetUri) {
        registerDeferredReceiveTarget(
          resolved.backendPath,
          resolved.deferredTargetUri,
          msg.fileDetails.filename,
        );
      }
      fileOfferActionState[msg.id] = "accepted";
      notifications.push("File transfer accepted", "success");
    } catch (e) {
      fileOfferActionState[msg.id] = "idle";
      notifications.push(
        `Failed to accept file: ${e instanceof Error ? e.message : e}`,
        "error",
      );
      console.error("Accept file offer failed", e);
    }
  }

  async function handleResend(msg: MessageResponse) {
    if (!contactId) return;
    try {
      await deleteLocalMessage(msg.contactId, msg.id);
    } catch (e) {
      console.error("Failed to delete local message", e);
    }
    messagesState.removeLocally(msg.id, msg.contactId);

    if (msg.fileDetails) {
      try {
        const prepared = await pickFileForSend(msg.fileDetails.filename);
        if (!prepared) return;
        const [messageId, fileSize] = await sendFileOffer(
          contactId,
          prepared.backendPath,
        );
        messagesState.appendOptimistic({
          id: messageId,
          contactId,
          direction: "sent",
          content: "[File]",
          status: "sending",
          timestamp: Date.now(),
          replyTo: null,
          fileDetails: { filename: prepared.filename, sizeBytes: fileSize },
        });
      } catch (e) {
        notifications.push("Failed to resend file", "error");
      }
      return;
    }

    try {
      const messageId = await sendText(contactId, msg.content, msg.replyTo);
      messagesState.appendOptimistic({
        id: messageId,
        contactId,
        direction: "sent",
        content: msg.content,
        status: "sending",
        timestamp: Date.now(),
        replyTo: msg.replyTo,
      });
    } catch (e) {
      notifications.push("Failed to resend", "error");
    }
  }

  function onBubbleTouchStart(e: TouchEvent, msgId: string) {
    if (!isCoarsePointer) return;
    const t = e.touches[0];
    swipeStart = { x: t.clientX, y: t.clientY, id: msgId };
  }
  function onBubbleTouchMove(e: TouchEvent, msgId: string) {
    if (!isCoarsePointer || swipeStart.id !== msgId) return;
    const t = e.touches[0];
    const dx = t.clientX - swipeStart.x;
    const dy = Math.abs(t.clientY - swipeStart.y);
    if (Math.abs(dx) > 6 || dy > 6) cancelLongPress();
    if (dx > 10 && dy < 30) {
      swipeOffset = { id: msgId, dx: Math.min(dx, 80) };
    }
  }
  function onBubbleTouchEnd(msgId: string, msg: MessageResponse) {
    if (swipeOffset?.id === msgId && swipeOffset.dx > 60) {
      startReply(msg);
    }
    swipeStart = { x: 0, y: 0, id: "" };
    swipeOffset = null;
  }

  let longPressTimer: ReturnType<typeof setTimeout> | null = null;
  function onBubbleTouchStartLongPress(msgId: string) {
    if (!isCoarsePointer) return;
    if (longPressTimer) clearTimeout(longPressTimer);
    longPressTimer = setTimeout(() => {
      actionSheetMsgId = msgId;
      if (navigator.vibrate) navigator.vibrate(15);
    }, 450);
  }
  function cancelLongPress() {
    if (longPressTimer) {
      clearTimeout(longPressTimer);
      longPressTimer = null;
    }
  }

  function swipeOffsetFor(id: string): number {
    return swipeOffset?.id === id ? swipeOffset.dx : 0;
  }

  const actionSheetMsg = $derived(
    actionSheetMsgId ? messageIndex.get(actionSheetMsgId) : null,
  );
</script>

{#if contact}
  <div class="chat">
    <ChatHeader
      {contact}
      onOpenProfile={openProfileModal}
      onOpenSecurity={openSecurityCodeModal}
    />

    {#if showShareBanner}
      <div class="share-banner">
        <span>Share your profile with <b>{contact.displayName}</b>?</span>
        <div class="banner-actions">
          <button class="banner-btn primary" onclick={handleShareQuickProfile}
            >Share</button
          >
          <button
            class="banner-btn icon"
            onclick={closeShareBanner}
            aria-label="Dismiss"><X size={14} /></button
          >
        </div>
      </div>
    {/if}

    <div class="messages" bind:this={listEl} onscroll={handleScroll}>
      {#if visibleMessages.length === 0}
        <div class="empty-chat">
          <Avatar
            name={contact.displayName}
            src={contact.avatarBase64}
            size={72}
          />
          <h3>{contact.displayName}</h3>
          <p>This is the beginning of your encrypted conversation.</p>
          {#if !contact.verified}
            <button class="empty-verify" onclick={openSecurityCodeModal}>
              <ShieldAlert size={14} />
              Verify identity
            </button>
          {/if}
        </div>
      {:else}
        {#each messageGroups as group, gi (gi)}
          {#if firstUnreadId && group.messages[0]?.id === firstUnreadId}
            <div class="unread-separator" aria-label="New messages">
              <span>New messages</span>
            </div>
          {/if}
          <div class="msg-group" class:sent={group.direction === "sent"}>
            {#if gi === 0 || group.timestamp - messageGroups[gi - 1].timestamp > 3600000}
              <div class="time-separator">
                <span>{formatGroupTime(group.timestamp)}</span>
              </div>
            {/if}

            <div class="group-body" class:sent={group.direction === "sent"}>
              {#if group.direction === "received"}
                <div class="group-avatar">
                  <Avatar
                    name={contact.displayName}
                    src={contact.avatarBase64}
                    size={28}
                  />
                </div>
              {/if}
              <div
                class="group-messages"
                class:sent={group.direction === "sent"}
              >
                {#each group.messages as msg, mi (msg.id)}
                  {@const repliedMessage = msg.replyTo
                    ? (messageIndex.get(msg.replyTo) ?? null)
                    : null}
                  {@const reactions = messagesState.reactionsFor(
                    msg.id,
                    msg.contactId,
                  )}
                  <MessageBubble
                    {msg}
                    {repliedMessage}
                    {reactions}
                    userId={profileState.userId || ""}
                    isFirst={mi === 0}
                    isLast={mi === group.messages.length - 1}
                    {isCoarsePointer}
                    hovered={hoveredMessageId === msg.id}
                    emojiOpen={showEmojiPicker === msg.id}
                    swipeDx={swipeOffsetFor(msg.id)}
                    fileOfferState={fileOfferActionState[msg.id]}
                    transferPercent={transferPercent(msg.id)}
                    transferInProgress={!!messagesState.transferProgressFor(
                      msg.id,
                    ) && !isTransferDone(msg.id)}
                    transferDone={isTransferDone(msg.id)}
                    onHoverEnter={() => (hoveredMessageId = msg.id)}
                    onHoverLeave={() => (hoveredMessageId = null)}
                    onTouchStart={(e) => {
                      onBubbleTouchStart(e, msg.id);
                      onBubbleTouchStartLongPress(msg.id);
                    }}
                    onTouchMove={(e) => onBubbleTouchMove(e, msg.id)}
                    onTouchEnd={() => {
                      onBubbleTouchEnd(msg.id, msg);
                      cancelLongPress();
                    }}
                    onTouchCancel={() => {
                      swipeStart = { x: 0, y: 0, id: "" };
                      swipeOffset = null;
                      cancelLongPress();
                    }}
                    onReplyRefClick={handleReplyRefClick}
                    onAcceptFile={() => handleAcceptIncomingFile(msg)}
                    onToggleReact={(emoji) => toggleReaction(msg, emoji)}
                    onStartReply={() => startReply(msg)}
                    onCopy={() => copyMessageText(msg)}
                    onStartEdit={() => startEdit(msg)}
                    onDelete={() => handleDelete(msg)}
                    onResend={() => handleResend(msg)}
                    onDeleteLocal={() => handleDeleteLocal(msg)}
                    onToggleEmojiPicker={(rect) => {
                      if (showEmojiPicker === msg.id) {
                        showEmojiPicker = null;
                        emojiPickerAnchor = null;
                      } else {
                        showEmojiPicker = msg.id;
                        emojiPickerAnchor = rect;
                      }
                    }}
                  />
                {/each}
              </div>
            </div>
          </div>
        {/each}
      {/if}
    </div>

    {#if !isScrolledToBottom}
      <button
        class="scroll-to-bottom"
        onclick={() => scrollToBottom("smooth")}
        aria-label="Jump to latest"
      >
        {#if unreadCount > 0}
          <span class="scroll-badge"
            >{unreadCount > 99 ? "99+" : unreadCount}</span
          >
        {/if}
        <ChevronDown size={18} />
      </button>
    {/if}

    <MessageComposer
      {contact}
      bind:inputText
      {sending}
      {isCoarsePointer}
      replyingPreview={replyingToPreview}
      {editingPreview}
      replyActive={!!replyingToMessageId}
      editActive={!!editingMessageId}
      onSend={handleSend}
      onAttach={handleSendFile}
      onInput={onComposerInput}
      onCancelReply={cancelReply}
      onCancelEdit={cancelEdit}
      onOpenProfile={openProfileModal}
      bind:composerEl
    />
  </div>

  {#if actionSheetMsg}
    <ActionSheet
      msg={actionSheetMsg}
      onClose={() => (actionSheetMsgId = null)}
      onReact={(emoji) =>
        actionSheetMsg && toggleReaction(actionSheetMsg, emoji)}
      onMoreEmoji={() => {
        if (actionSheetMsg) {
          showEmojiPicker = actionSheetMsg.id;
          actionSheetMsgId = null;
        }
      }}
      onReply={() => actionSheetMsg && startReply(actionSheetMsg)}
      onCopy={() => actionSheetMsg && copyMessageText(actionSheetMsg)}
      onEdit={() => actionSheetMsg && startEdit(actionSheetMsg)}
      onDelete={() => actionSheetMsg && handleDelete(actionSheetMsg)}
    />
  {/if}

  {#if showSecurityCode && contact}
    <SecurityCodeModal
      contactId={contact.userId}
      contactVerified={contact.verified}
      onClose={closeSecurityCodeModal}
    />
  {/if}
  {#if showProfile && contact}
    <ProfileModal {contact} onClose={closeProfileModal} />
  {/if}

  {#if isDraggingFile}
    <div class="drop-overlay" aria-hidden="true">
      <div class="drop-overlay-inner">
        <Paperclip size={36} />
        <span>Drop file to send</span>
      </div>
    </div>
  {/if}

  {#if showEmojiPicker}
    <div
      class="emoji-picker-layer"
      style={emojiPickerPos
        ? `top:${emojiPickerPos.top}px;left:${emojiPickerPos.left}px;`
        : `top:50%;left:50%;transform:translate(-50%,-50%);`}
    >
      <EmojiPicker
        compact
        onSelect={handlePickerSelect}
        onClose={closePicker}
      />
    </div>
  {/if}

  {#if pendingFile}
    <FileConfirmModal
      filename={pendingFile.filename}
      sizeBytes={pendingFile.sizeBytes}
      sending={sendingFile}
      onConfirm={confirmSendFile}
      onCancel={cancelSendFile}
    />
  {/if}
{:else}
  <div class="loading"><Spinner /></div>
{/if}

<style>
  .chat {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
    position: relative;
    background: radial-gradient(
        1200px 600px at 50% -100px,
        rgba(99, 102, 241, 0.06),
        transparent 60%
      ),
      var(--bg-primary);
  }

  .share-banner {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    padding: 10px 16px;
    background: var(--accent-dim);
    border-bottom: 1px solid var(--border);
    font-size: 13px;
    color: var(--text-secondary);
    flex-shrink: 0;
  }
  .share-banner b {
    color: var(--text-primary);
    font-weight: 600;
  }
  .banner-actions {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .banner-btn {
    padding: 6px 12px;
    border-radius: var(--radius-sm);
    font-size: 12px;
    font-weight: 600;
    color: var(--text-secondary);
    transition: all var(--transition);
  }
  .banner-btn.icon {
    padding: 6px;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .banner-btn:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }
  .banner-btn.primary {
    background: var(--accent);
    color: #fff;
  }
  .banner-btn.primary:hover {
    background: var(--accent-hover);
  }

  .messages {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    overflow-x: hidden;
    padding: 16px 16px 48px;
    display: flex;
    flex-direction: column;
    gap: 2px;
    scroll-behavior: smooth;
    overscroll-behavior: contain;
  }

  .empty-chat {
    margin: auto;
    text-align: center;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    color: var(--text-muted);
    padding: 40px 24px;
  }
  .empty-chat h3 {
    font-size: 20px;
    color: var(--text-primary);
    margin: 4px 0 0;
    font-weight: 600;
  }
  .empty-chat p {
    font-size: 13px;
    margin: 0;
    max-width: 280px;
    line-height: 1.5;
  }
  .empty-verify {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 8px;
    padding: 8px 14px;
    background: rgba(251, 191, 36, 0.12);
    color: var(--warning);
    border-radius: 999px;
    font-size: 12px;
    font-weight: 600;
    transition: background var(--transition);
  }
  .empty-verify:hover {
    background: rgba(251, 191, 36, 0.22);
  }

  .msg-group {
    margin-top: 6px;
  }
  .msg-group + .msg-group {
    margin-top: 10px;
  }

  .time-separator {
    text-align: center;
    margin: 18px 0 8px;
    position: sticky;
    top: 4px;
    z-index: 2;
    pointer-events: none;
  }
  .time-separator span {
    font-size: 11px;
    font-weight: 600;
    color: var(--text-muted);
    background: rgba(17, 24, 39, 0.9);
    padding: 4px 12px;
    border-radius: 999px;
    border: 1px solid var(--border);
    backdrop-filter: blur(8px);
    -webkit-backdrop-filter: blur(8px);
  }

  .group-body {
    display: flex;
    gap: 8px;
    max-width: 100%;
    align-items: flex-end;
  }
  .group-body.sent {
    justify-content: flex-end;
  }

  .group-avatar {
    flex-shrink: 0;
    align-self: flex-end;
    margin-bottom: 2px;
    position: sticky;
    bottom: 4px;
    z-index: 1;
  }

  .group-messages {
    display: flex;
    flex-direction: column;
    gap: 2px;
    max-width: min(72%, 620px);
    min-width: 0;
  }
  .group-messages.sent {
    align-items: flex-end;
  }

  .scroll-to-bottom {
    position: absolute;
    right: 16px;
    bottom: calc(var(--composer-height, 64px) + 12px);
    width: 40px;
    height: 40px;
    border-radius: 50%;
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    color: var(--text-primary);
    display: flex;
    align-items: center;
    justify-content: center;
    box-shadow: 0 6px 20px rgba(0, 0, 0, 0.35);
    transition: all var(--transition);
    z-index: 15;
    animation: fadeInFab 0.18s ease;
  }
  .scroll-to-bottom:hover {
    background: var(--bg-hover);
    transform: translateY(-1px);
  }
  .scroll-to-bottom:active {
    transform: translateY(0);
  }
  @keyframes fadeInFab {
    from {
      opacity: 0;
      transform: translateY(8px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }
  .scroll-badge {
    position: absolute;
    top: -4px;
    right: -4px;
    background: var(--accent);
    color: #fff;
    font-size: 10px;
    font-weight: 700;
    min-width: 18px;
    height: 18px;
    padding: 0 5px;
    border-radius: 999px;
    display: flex;
    align-items: center;
    justify-content: center;
    border: 2px solid var(--bg-primary);
  }

  .loading {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .unread-separator {
    display: flex;
    align-items: center;
    gap: 10px;
    margin: 12px 0 6px;
    color: var(--danger);
  }
  .unread-separator::before,
  .unread-separator::after {
    content: "";
    flex: 1;
    height: 1px;
    background: currentColor;
    opacity: 0.5;
  }
  .unread-separator span {
    font-size: 10.5px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.08em;
  }

  .emoji-picker-layer {
    position: fixed;
    z-index: 250;
    animation: pickerFadeIn 0.12s ease;
  }
  @keyframes pickerFadeIn {
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }

  .drop-overlay {
    position: absolute;
    inset: 0;
    background: rgba(15, 23, 42, 0.78);
    backdrop-filter: blur(6px);
    -webkit-backdrop-filter: blur(6px);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 220;
    pointer-events: none;
    animation: fadeIn 0.12s ease;
  }
  @keyframes fadeIn {
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }
  .drop-overlay-inner {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    padding: 28px 40px;
    border: 2px dashed var(--accent);
    border-radius: var(--radius-lg);
    background: rgba(99, 102, 241, 0.08);
    color: var(--accent);
    font-weight: 600;
    font-size: 14px;
  }

  @media (max-width: 768px) {
    .messages {
      padding: 12px 10px 4px;
    }
    .group-messages {
      max-width: 82%;
    }
  }

  @media (max-width: 480px) {
    .group-messages {
      max-width: 86%;
    }
  }
</style>
