import type { MessageResponse } from "$lib/types";
import { getMessages } from "$lib/api/messages";

function createMessagesState() {
  // keyed by contactId
  let map = $state<Record<string, MessageResponse[]>>({});
  // keyed by contactId
  let unreadByContact = $state<Record<string, number>>({});
  // keyed by `${contactId}:${messageId}`
  let reactions = $state<Record<string, Array<{ emoji: string, userIds: string[] }>>>({});
  // keyed by transfer/message id
  let transferProgress = $state<Record<string, { bytesTransferred: number; totalBytes: number }>>({});
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
        if (m.reactions && m.reactions.length > 0) {
          const key = reactionKey(m.contactId, m.id);
          const grouped: Record<string, { emoji: string, userIds: string[] }> = {};
          m.reactions.forEach(r => {
            if (!grouped[r.emoji]) grouped[r.emoji] = { emoji: r.emoji, userIds: [] };
            if (!grouped[r.emoji].userIds.includes(r.userId)) {
              grouped[r.emoji].userIds.push(r.userId);
            }
          });
          reactions[key] = Object.values(grouped);
        }

        if (m.status === "sending" && now - m.timestamp > 15000) {
          m.status = "failed";
        } else if (m.status === "sending") {
          setTimeout(
            () => {
              updateStatusIfSending(m.id, contactId, "failed");
            },
            15000 - (now - m.timestamp),
          );
        }
      });
      // Filter out deleted messages
      map[contactId] = msgs.filter(m => m.content !== "");
    } catch (e) {
      console.error("Failed to load messages for", contactId, e);
    }
    loadedContacts.add(contactId);
  }

  function forContact(contactId: string): MessageResponse[] {
    return map[contactId] ?? [];
  }

  function append(msg: MessageResponse) {
    if (msg.content === "") return;
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
    }, 15000);
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
      msg.edited = true;
      map[contactId] = [...list];
    }
  }

  function markDeleted(messageId: string, contactId: string) {
    const list = map[contactId];
    if (!list) return;
    map[contactId] = list.filter((m) => m.id !== messageId);
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

  function addReaction(messageId: string, contactId: string, emoji: string, userId: string) {
    const key = reactionKey(contactId, messageId);
    const current = reactions[key] ?? [];
    const index = current.findIndex(r => r.emoji === emoji);
    if (index >= 0) {
      if (!current[index].userIds.includes(userId)) {
        current[index].userIds.push(userId);
      }
    } else {
      current.push({ emoji, userIds: [userId] });
    }
    reactions[key] = [...current];
  }

  function removeReaction(messageId: string, contactId: string, emoji: string, userId: string) {
    const key = reactionKey(contactId, messageId);
    let current = reactions[key] ?? [];
    const index = current.findIndex(r => r.emoji === emoji);
    if (index >= 0) {
      current[index].userIds = current[index].userIds.filter(u => u !== userId);
      
      if (current[index].userIds.length === 0) {
        current = current.filter(r => r.emoji !== emoji);
      } else {
        current = [...current];
      }
      reactions[key] = current;
    }
  }

  function reactionsFor(messageId: string, contactId: string): Array<{ emoji: string, userIds: string[] }> {
    return reactions[reactionKey(contactId, messageId)] ?? [];
  }

  function setTransferProgress(transferId: string, bytesTransferred: number, totalBytes: number) {
    transferProgress[transferId] = { bytesTransferred, totalBytes };
  }

  function transferProgressFor(transferId: string): { bytesTransferred: number; totalBytes: number } | null {
    return transferProgress[transferId] ?? null;
  }

  function clearTransferProgress(transferId: string) {
    if (!transferProgress[transferId]) return;
    const copy = { ...transferProgress };
    delete copy[transferId];
    transferProgress = copy;
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
    setTransferProgress,
    transferProgressFor,
    clearTransferProgress,
  };
}

export const messagesState = createMessagesState();
