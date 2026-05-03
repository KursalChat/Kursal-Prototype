<script lang="ts">
  import { page } from "$app/state";
  import { onMount, tick, untrack } from "svelte";
  import { browser } from "$app/environment";
  import { goto } from "$app/navigation";
  import { stat } from "@tauri-apps/plugin-fs";
  import { t } from "$lib/i18n";
  import { contactsState } from "$lib/state/contacts.svelte";
  import { messagesState } from "$lib/state/messages.svelte";
  import { profileState } from "$lib/state/profile.svelte";
  import { settingsState } from "$lib/state/settings.svelte";
  import { draftsState } from "$lib/state/drafts.svelte";
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
  import { OS } from "$lib/api/window";
  import {
    pickFileForSend,
    pickFileForReceive,
    prepareOfferSourcePath,
    registerDeferredReceiveTarget,
  } from "$lib/utils/file-transfer-paths";
  import type { MessageResponse } from "$lib/types";
  import { notifications } from "$lib/state/notifications.svelte";
  import { typingState } from "$lib/state/typing.svelte";
  import * as haptics from "$lib/utils/haptics";
  import { confirm } from "@tauri-apps/plugin-dialog";
  import { ShieldAlert, X, Paperclip, ChevronDown } from "lucide-svelte";
  import Avatar from "$lib/components/Avatar.svelte";
  import Spinner from "$lib/components/Spinner.svelte";
  import SecurityCodeModal from "$lib/components/SecurityCodeModal.svelte";
  import ProfileModal from "$lib/components/ProfileModal.svelte";
  import ChatHeader from "./ChatHeader.svelte";
  import MessageBubble from "./MessageBubble.svelte";
  import MessageComposer from "./MessageComposer.svelte";
  import OfflineQueueBar from "./OfflineQueueBar.svelte";
  import ActionSheet from "./ActionSheet.svelte";
  import SelectTextModal from "./SelectTextModal.svelte";
  import FileConfirmModal from "./FileConfirmModal.svelte";
  import MediaViewer from "./MediaViewer.svelte";
  import { convertFileSrc } from "@tauri-apps/api/core";
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
  let selectTextMsgId = $state<string | null>(null);
  let fileOfferActionState = $state<
    Record<string, "idle" | "accepting" | "accepted">
  >({});
  const completedFileTimers = new Map<string, ReturnType<typeof setTimeout>>();
  let isCoarsePointer = $state(false);
  let listEl = $state<HTMLElement | null>(null);
  let composerEl = $state<HTMLTextAreaElement | null>(null);
  let composerHostEl = $state<HTMLElement | null>(null);
  let composerHeight = $state(76);

  let isScrolledToBottom = $state(true);
  let isAtMaxBottom = $state(true);
  let unreadCount = $state(0);
  let isDraggingFile = $state(false);
  let mediaViewer = $state<{
    src: string;
    path: string;
    kind: "image" | "video";
    filename: string;
  } | null>(null);
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

  let flushingQueue = $state(false);
  const queuedList = $derived(
    contactId ? messagesState.queuedFor(contactId) : [],
  );
  const queuedCount = $derived(queuedList.length);
  const QUEUE_THRESHOLD = 15;
  const QUEUE_INTERVAL_MS = 5 * 60 * 1000;

  async function handleShareQuickProfile() {
    if (!browser || !contactId) return;
    try {
      await shareProfile(
        profileState.displayName,
        profileState.avatarBytes,
        contactId,
      );
      const status = contactsState.connectionStatus[contactId];
      const online = status === "direct" || status === "holepunch" || status === "relay";
      if (contact) contactsState.upsert({ ...contact, profileShared: true });
      notifications.push(
        online
          ? t("chat.conversation.successProfileShared")
          : t("chat.conversation.successProfileQueued", { name: contact?.displayName ?? "" }),
        "success",
      );
    } catch (e) {
      notifications.push(t("chat.conversation.errorProfileShare"), "error");
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
    if (editingMessageId) {
      lastTypingSentAt = 0;
      return;
    }
    if (!settingsState.typingIndicators) {
      lastTypingSentAt = 0;
      return;
    }
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
      await confirm(t("chat.conversation.deleteConfirmMessage"), {
        title: t("chat.conversation.deleteConfirmTitle"),
        kind: "warning",
      })
    ) {
      try {
        await deleteMessage(contactId, msg.id);
        messagesState.markDeleted(msg.id, contactId);
      } catch (e) {
        console.error("Delete failed", e);
        notifications.push(t("chat.conversation.errorDeleteMessage"), "error");
      }
    }
  }

  async function toggleReaction(msg: MessageResponse, emoji: string) {
    void haptics.select();
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
      notifications.push(t("chat.conversation.successCopied"), "success");
    } catch {
      notifications.push(t("chat.conversation.errorCopy"), "error");
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

    const flushInterval = setInterval(() => {
      if (queuedCount > 0 && !flushingQueue) void flushQueue();
    }, QUEUE_INTERVAL_MS);

    let contactMissingTimer: ReturnType<typeof setTimeout> | null = null;
    if (!contact && !contactsState.loading) {
      contactMissingTimer = setTimeout(() => {
        if (contactId && !contactsState.getById(contactId)) {
          notifications.push(t("chat.conversation.errorContactNotFound"), "error");
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
      clearInterval(flushInterval);
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
    const id = contactId;
    if (!id) return;
    // Read draft non-reactively so this effect only fires on contact
    // switch — not when handleSend clears the draft (which would race
    // and re-load stale text into the composer).
    inputText = untrack(() => draftsState.get(id));
    replyingToMessageId = null;
    editingMessageId = null;
    return () => {
      if (editingMessageId) return;
      draftsState.set(id, inputText);
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
    if (!composerHostEl) return;
    const el = composerHostEl;
    const ro = new ResizeObserver(() => {
      composerHeight = el.offsetHeight;
    });
    ro.observe(el);
    composerHeight = el.offsetHeight;
    return () => ro.disconnect();
  });

  // Re-pin to bottom when message content grows (image loads, file progress
  // appearing, etc.) and the user was already at the bottom.
  $effect(() => {
    if (!listEl) return;
    const el = listEl;
    const ro = new ResizeObserver(() => {
      if (isScrolledToBottom) {
        el.scrollTop = el.scrollHeight;
      }
    });
    for (const child of Array.from(el.children)) {
      ro.observe(child);
    }
    const mo = new MutationObserver((mutations) => {
      for (const mut of mutations) {
        for (const node of mut.addedNodes) {
          if (node instanceof Element) ro.observe(node);
        }
      }
    });
    mo.observe(el, { childList: true });
    return () => {
      ro.disconnect();
      mo.disconnect();
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

  const MAX_MESSAGE_LENGTH = 10000;

  async function handleSend() {
    const text = inputText.trim();
    if (!text) return;
    if (text.length > MAX_MESSAGE_LENGTH) {
      notifications.push(
        t("chat.conversation.errorMessageTooLong", { length: text.length, max: MAX_MESSAGE_LENGTH }),
        "error",
      );
      return;
    }

    if (editingMessageId) {
      if (!contactId) return;
      sending = true;
      try {
        await editMessage(contactId, editingMessageId, text);
        messagesState.updateContent(editingMessageId, contactId, text);
        inputText = "";
        editingMessageId = null;
      } catch (e) {
        notifications.push(t("chat.conversation.errorEditMessage"), "error");
        console.error("Edit failed:", e);
      } finally {
        sending = false;
      }
      return;
    }

    if (!contactId) return;
    const replyTo = replyingToMessageId;
    inputText = "";
    draftsState.clear(contactId);
    lastTypingSentAt = 0;
    cancelReply();

    const cid = contactId;
    const pendingId = crypto.randomUUID().replace(/-/g, "");
    messagesState.appendOptimistic({
      id: pendingId,
      contactId: cid,
      direction: "sent",
      content: text,
      status: "sending",
      timestamp: Date.now(),
      replyTo,
    });

    void haptics.impact("medium");

    void sendText(cid, text, replyTo)
      .then((realId) => messagesState.replaceId(pendingId, cid, realId))
      .catch((e) => {
        messagesState.updateStatusIfSending(pendingId, cid, "queued");
        void haptics.notify("warning");
        console.error("Send failed, queued for offline:", e);
      });
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
        t("chat.conversation.errorOpenFile", { error: e instanceof Error ? e.message : String(e) }),
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
        t("chat.conversation.errorPrepareFile", { error: e instanceof Error ? e.message : String(e) }),
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
        fileDetails: {
          filename: file.filename,
          sizeBytes: fileSize,
          autodownloadPath: file.backendPath,
        },
      });
      pendingFile = null;
    } catch (e) {
      notifications.push(
        t("chat.conversation.errorSendFile", { error: e instanceof Error ? e.message : String(e) }),
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
      } else {
        messagesState.setAutodownloadPath(
          msg.id,
          msg.contactId,
          resolved.backendPath,
        );
      }
      fileOfferActionState[msg.id] = "accepted";
      notifications.push(t("chat.conversation.successFileAccepted"), "success");
    } catch (e) {
      fileOfferActionState[msg.id] = "idle";
      const raw = e instanceof Error ? e.message : String(e);
      const lower = raw.toLowerCase();
      const isAccessDenied =
        OS === "windows" &&
        (lower.includes("accès refusé") ||
          lower.includes("acces refuse") ||
          lower.includes("access is denied") ||
          lower.includes("access denied") ||
          lower.includes("permission denied") ||
          lower.includes("os error 5"));
      const hint = isAccessDenied
        ? t("chat.conversation.errorWindowsFile")
        : t("chat.conversation.errorAcceptFile", { error: raw });
      notifications.push(hint, "error");
      console.error("Accept file offer failed", e);
    }
  }

  async function flushQueue() {
    if (!contactId || flushingQueue) return;
    const cid = contactId;
    const items = messagesState.queuedFor(cid);
    if (items.length === 0) return;
    flushingQueue = true;
    try {
      for (const msg of items) {
        const pendingId = msg.id;
        messagesState.updateStatus(pendingId, cid, "sending");
        try {
          if (msg.fileDetails) {
            messagesState.updateStatus(pendingId, cid, "queued");
            continue;
          }
          const realId = await sendText(cid, msg.content, msg.replyTo);
          messagesState.replaceId(pendingId, cid, realId);
          // Stay "sending" — backend will emit a delivery event once
          // the peer actually acks. Eagerly marking "delivered" here
          // would lie when the peer is offline.
        } catch (e) {
          messagesState.updateStatus(pendingId, cid, "queued");
          console.error("Queue flush: send failed", e);
        }
      }
    } finally {
      flushingQueue = false;
    }
  }

  let lastFlushAt = 0;
  $effect(() => {
    if (!contactId) return;
    if (queuedCount >= QUEUE_THRESHOLD && !flushingQueue) {
      if (Date.now() - lastFlushAt < 30000) return;
      lastFlushAt = Date.now();
      void flushQueue();
    }
  });

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
          fileDetails: {
            filename: prepared.filename,
            sizeBytes: fileSize,
            autodownloadPath: prepared.backendPath,
          },
        });
      } catch (e) {
        notifications.push(t("chat.conversation.errorResendFile"), "error");
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
      notifications.push(t("chat.conversation.errorResend"), "error");
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
      void haptics.impact("medium");
      startReply(msg);
    }
    swipeStart = { x: 0, y: 0, id: "" };
    swipeOffset = null;
  }

  let longPressTimer: ReturnType<typeof setTimeout> | null = null;
  function onBubbleTouchStartLongPress(msgId: string) {
    if (!isCoarsePointer) return;
    const m = messageIndex.get(msgId);
    if (m && (m.status === "sending" || m.status === "failed")) return;
    if (longPressTimer) clearTimeout(longPressTimer);
    longPressTimer = setTimeout(() => {
      actionSheetMsgId = msgId;
      void haptics.impact("medium");
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
  const selectTextMsg = $derived(
    selectTextMsgId ? messageIndex.get(selectTextMsgId) : null,
  );

  function openSelectText(msg: MessageResponse) {
    selectTextMsgId = msg.id;
    actionSheetMsgId = null;
  }
</script>

{#if contact}
  <div class="chat" style="--composer-h: {composerHeight}px;">
    <ChatHeader
      {contact}
      onOpenProfile={openProfileModal}
      onOpenSecurity={openSecurityCodeModal}
    />

    {#if showShareBanner}
      <div class="share-banner">
        <span>{t("chat.conversation.shareBanner", { name: contact.displayName })}</span>
        <div class="banner-actions">
          <button class="banner-btn primary" onclick={handleShareQuickProfile}
            >{t("chat.conversation.shareBannerButton")}</button
          >
          <button
            class="banner-btn icon"
            onclick={closeShareBanner}
            aria-label={t("chat.conversation.dismissBannerAriaLabel")}><X size={14} /></button
          >
        </div>
      </div>
    {/if}

    <div class="messages" bind:this={listEl} onscroll={handleScroll}>
      {#if visibleMessages.length === 0}
        <div class="empty-chat">
          <div class="empty-avatar">
            <span class="empty-avatar-glow"></span>
            <Avatar
              name={contact.displayName}
              src={contact.avatarBase64}
              size={84}
            />
          </div>
          <h3>{t("chat.conversation.emptyHeading", { name: contact.displayName })}</h3>
          <p class="empty-trust">
            <ShieldAlert size={13} />
            {t("chat.conversation.emptyEncrypted")}
          </p>
          {#if !contact.verified}
            <button class="empty-verify" onclick={openSecurityCodeModal}>
              <ShieldAlert size={14} />
              {t("chat.conversation.verifyButton")}
            </button>
          {/if}
        </div>
      {:else}
        {#each messageGroups as group, gi (gi)}
          {#if firstUnreadId && group.messages[0]?.id === firstUnreadId}
            <div class="unread-separator" aria-label={t("chat.conversation.newMessages")}>
              <span>{t("chat.conversation.newMessages")}</span>
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
                    onOpenMedia={(path, kind, filename) => {
                      mediaViewer = {
                        src: convertFileSrc(path),
                        path,
                        kind,
                        filename,
                      };
                    }}
                  />
                {/each}
              </div>
            </div>
          </div>
        {/each}
      {/if}

      {#if typingState.isTyping(contact.userId)}
        <div class="typing-row" aria-label={t("chat.composer.typingIndicator", { name: contact.displayName })}>
          <Avatar
            name={contact.displayName}
            src={contact.avatarBase64}
            size={22}
          />
          <span class="typing-dots">
            <span></span><span></span><span></span>
          </span>
        </div>
      {/if}
    </div>

    {#if !isScrolledToBottom}
      <button
        class="scroll-to-bottom"
        class:has-unread={unreadCount > 0}
        onclick={() => scrollToBottom("smooth")}
        aria-label={t("chat.conversation.jumpToLatest")}
      >
        {#if unreadCount > 0}
          <Avatar
            name={contact.displayName}
            src={contact.avatarBase64}
            size={22}
          />
          <span class="scroll-badge"
            >{unreadCount > 99 ? "99+" : unreadCount}</span
          >
        {:else}
          <ChevronDown size={18} />
        {/if}
      </button>
    {/if}

    <div class="composer-host" bind:this={composerHostEl}>
      <OfflineQueueBar
        count={queuedCount}
        flushing={flushingQueue}
        onFlush={flushQueue}
      />

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
      onSelectText={() => actionSheetMsg && openSelectText(actionSheetMsg)}
      onEdit={() => actionSheetMsg && startEdit(actionSheetMsg)}
      onDelete={() => actionSheetMsg && handleDelete(actionSheetMsg)}
    />
  {/if}

  {#if selectTextMsg}
    <SelectTextModal
      text={selectTextMsg.content}
      onClose={() => (selectTextMsgId = null)}
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
        <span>{t("chat.conversation.dropFileOverlay")}</span>
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

  {#if mediaViewer}
    <MediaViewer
      src={mediaViewer.src}
      path={mediaViewer.path}
      kind={mediaViewer.kind}
      filename={mediaViewer.filename}
      onClose={() => (mediaViewer = null)}
    />
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
    background: transparent;
  }

  .share-banner {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    padding: 10px 16px;
    background: var(--accent-dim);
    font-size: 13px;
    color: var(--text-secondary);
    flex-shrink: 0;
    animation: share-in 280ms cubic-bezier(0.34, 1.56, 0.64, 1);
  }
  @keyframes share-in {
    from { opacity: 0; transform: translateY(-4px); }
    to { opacity: 1; transform: translateY(0); }
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
    padding: 16px 16px calc(var(--composer-h, 76px) + 24px);
    display: flex;
    flex-direction: column;
    gap: 2px;
    scroll-behavior: smooth;
    overscroll-behavior: contain;
    -webkit-mask-image: linear-gradient(
      to bottom,
      transparent 0,
      black 14px
    );
    mask-image: linear-gradient(
      to bottom,
      transparent 0,
      black 14px
    );
    scrollbar-width: thin;
  }

  .composer-host {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    padding: 0 8px calc(8px + env(safe-area-inset-bottom, 0px));
    display: flex;
    flex-direction: column;
    align-items: stretch;
    gap: 8px;
    pointer-events: none;
    z-index: 12;
  }
  .composer-host::before {
    content: "";
    position: absolute;
    inset: -14px 0 0 0;
    z-index: -1;
    pointer-events: none;
    background: linear-gradient(
      to top,
      var(--bg-secondary),
      transparent
    );
  }
  .composer-host > :global(*) {
    pointer-events: auto;
  }

  .empty-chat {
    margin: auto;
    text-align: center;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
    color: var(--text-muted);
    padding: 40px 24px;
  }
  .empty-avatar {
    position: relative;
    display: inline-flex;
    margin-bottom: 4px;
  }
  .empty-avatar-glow {
    position: absolute;
    inset: -16px;
    border-radius: 50%;
    background: radial-gradient(
      circle,
      color-mix(in srgb, var(--accent) 35%, transparent) 0%,
      transparent 70%
    );
    animation: empty-breathe 3.4s ease-in-out infinite;
    pointer-events: none;
  }
  @keyframes empty-breathe {
    0%, 100% { opacity: 0.7; transform: scale(1); }
    50% { opacity: 1; transform: scale(1.08); }
  }
  .empty-chat h3 {
    font-size: 22px;
    color: var(--text-primary);
    margin: 4px 0 0;
    font-weight: 600;
    letter-spacing: -0.01em;
  }
  .empty-trust {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 12.5px;
    color: var(--text-muted);
    margin: 0;
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
    display: flex;
    align-items: center;
    gap: 10px;
    margin: 22px 4px 10px;
    pointer-events: none;
  }
  .time-separator::before,
  .time-separator::after {
    content: "";
    flex: 1;
    height: 1px;
    background: linear-gradient(
      to right,
      transparent,
      var(--border) 30%,
      var(--border) 70%,
      transparent
    );
  }
  .time-separator span {
    font-size: 11px;
    font-weight: 600;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.04em;
    font-variant-numeric: tabular-nums;
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
    bottom: calc(var(--composer-h, 76px) + env(safe-area-inset-bottom, 0px) + 16px);
    height: 36px;
    min-width: 36px;
    border-radius: 999px;
    background: var(--surface);
    backdrop-filter: blur(12px);
    -webkit-backdrop-filter: blur(12px);
    border: 1px solid var(--border-light);
    color: var(--text-primary);
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    padding: 0 4px;
    box-shadow: 0 4px 14px rgba(0, 0, 0, 0.12);
    transition: transform var(--transition), background var(--transition);
    z-index: 15;
    animation: fadeInFab 0.22s cubic-bezier(0.34, 1.56, 0.64, 1);
    font-size: 13px;
    font-weight: 600;
  }
  .scroll-to-bottom.has-unread {
    padding: 0 10px 0 4px;
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
    background: var(--accent);
    color: #fff;
    font-size: 11px;
    font-weight: 700;
    min-width: 18px;
    height: 18px;
    padding: 0 6px;
    border-radius: 999px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }

  .loading {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .typing-row {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    margin: 6px 0 0 4px;
    padding: 4px 10px 4px 4px;
    border-radius: 999px;
    align-self: flex-start;
    background: var(--bg-hover);
    animation: typing-in 220ms cubic-bezier(0.34, 1.56, 0.64, 1);
  }
  .typing-dots {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    color: var(--text-muted);
  }
  .typing-dots span {
    width: 4px;
    height: 4px;
    border-radius: 50%;
    background: currentColor;
    animation: typing-bounce 1.2s ease-in-out infinite;
  }
  .typing-dots span:nth-child(2) { animation-delay: 0.16s; }
  .typing-dots span:nth-child(3) { animation-delay: 0.32s; }
  @keyframes typing-bounce {
    0%, 60%, 100% { transform: translateY(0); opacity: 0.45; }
    30% { transform: translateY(-2px); opacity: 1; }
  }
  @keyframes typing-in {
    from { opacity: 0; transform: translateY(4px); }
    to { opacity: 1; transform: translateY(0); }
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
    background: color-mix(in srgb, var(--surface) 85%, transparent);
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
      padding: 12px 10px 48px;
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
