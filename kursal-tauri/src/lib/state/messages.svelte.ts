import type { MessageResponse } from "$lib/types";
import { getMessages } from "$lib/api/messages";

function createMessagesState() {
  // keyed by contactId
  let map = $state<Record<string, MessageResponse[]>>({});
  // keyed by contactId
  let unreadByContact = $state<Record<string, number>>({});
  // keyed by `${contactId}:${messageId}`
  let reactions = $state<Record<string, string[]>>({});
  let loadedContacts = $state<Set<string>>(new Set());

  function reactionKey(contactId: string, messageId: string) {
    return `${contactId}:${messageId}`;
  }

  async function loadFor(contactId: string) {
    if (loadedContacts.has(contactId)) return;
    try {
      const msgs = await getMessages(contactId);
      const now = Date.now();
      msgs.forEach((m) => {
        if (m.status === "sending" && now - m.timestamp > 30000) {
          m.status = "failed";
        } else if (m.status === "sending") {
          setTimeout(
            () => {
              updateStatusIfSending(m.id, contactId, "failed");
            },
            30000 - (now - m.timestamp),
          );
        }
      });
      map[contactId] = msgs;
    } catch (e) {
      console.error("Failed to load messages for", contactId, e);
    }
    loadedContacts.add(contactId);
  }

  function forContact(contactId: string): MessageResponse[] {
    return map[contactId] ?? [];
  }

  function append(msg: MessageResponse) {
    if (!map[msg.contactId]) map[msg.contactId] = [];
    const list = map[msg.contactId];
    // Don't add duplicate if already exists
    if (!list.find((m) => m.id === msg.id)) {
      list.push(msg);
      map[msg.contactId] = [...list];
      if (msg.direction === "received") {
        unreadByContact[msg.contactId] =
          (unreadByContact[msg.contactId] ?? 0) + 1;
      }
    }
  }

  function appendOptimistic(msg: MessageResponse) {
    if (!map[msg.contactId]) map[msg.contactId] = [];
    map[msg.contactId].push(msg);
    map[msg.contactId] = [...map[msg.contactId]];
    setTimeout(() => {
      updateStatusIfSending(msg.id, msg.contactId, "failed");
    }, 30000);
  }

  function updateStatus(
    messageId: string,
    contactId: string,
    status: MessageResponse["status"],
  ) {
    const list = map[contactId];
    if (!list) return;
    const msg = list.find((m) => m.id === messageId);
    if (msg) {
      msg.status = status;
      map[contactId] = [...list];
    }
  }

  function updateStatusIfSending(
    messageId: string,
    contactId: string,
    status: MessageResponse["status"],
  ) {
    const list = map[contactId];
    if (!list) return;
    const msg = list.find((m) => m.id === messageId);
    if (msg && msg.status === "sending") {
      msg.status = status;
      map[contactId] = [...list];
    }
  }

  function updateContent(
    messageId: string,
    contactId: string,
    content: string,
  ) {
    const list = map[contactId];
    if (!list) return;
    const msg = list.find((m) => m.id === messageId);
    if (msg) {
      msg.content = content;
      map[contactId] = [...list];
    }
  }

  function markDeleted(messageId: string, contactId: string) {
    updateContent(messageId, contactId, "[message deleted]");
  }

  function removeLocally(messageId: string, contactId: string) {
    const list = map[contactId];
    if (!list) return;
    map[contactId] = list.filter((m) => m.id !== messageId);
  }

  function unreadFor(contactId: string): number {
    return unreadByContact[contactId] ?? 0;
  }

  function totalUnread(): number {
    return Object.values(unreadByContact).reduce((acc, n) => acc + n, 0);
  }

  function markRead(contactId: string) {
    if (!unreadByContact[contactId]) return;
    unreadByContact[contactId] = 0;
  }

  function addReaction(messageId: string, contactId: string, emoji: string) {
    const key = reactionKey(contactId, messageId);
    const current = reactions[key] ?? [];
    if (!current.includes(emoji)) {
      reactions[key] = [...current, emoji];
    }
  }

  function removeReaction(messageId: string, contactId: string, emoji: string) {
    const key = reactionKey(contactId, messageId);
    const current = reactions[key] ?? [];
    reactions[key] = current.filter((value) => value !== emoji);
  }

  function reactionsFor(messageId: string, contactId: string): string[] {
    return reactions[reactionKey(contactId, messageId)] ?? [];
  }

  return {
    forContact,
    loadFor,
    append,
    appendOptimistic,
    updateStatus,
    updateStatusIfSending,
    updateContent,
    markDeleted,
    removeLocally,
    unreadFor,
    totalUnread,
    markRead,
    addReaction,
    removeReaction,
    reactionsFor,
  };
}

export const messagesState = createMessagesState();
