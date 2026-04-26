import type { MessageResponse } from "$lib/types";
import { getMessages } from "$lib/api/messages";

const AUTODOWNLOAD_STORAGE_KEY = "kursal:autodownloadPaths:v1";
const SEND_TIMEOUT_MS = 15000;

function loadAutodownloadStore(): Record<string, string> {
  if (typeof localStorage === "undefined") return {};
  try {
    const raw = localStorage.getItem(AUTODOWNLOAD_STORAGE_KEY);
    if (!raw) return {};
    const parsed = JSON.parse(raw);
    return parsed && typeof parsed === "object" ? parsed : {};
  } catch {
    return {};
  }
}

function saveAutodownloadStore(store: Record<string, string>) {
  if (typeof localStorage === "undefined") return;
  try {
    localStorage.setItem(AUTODOWNLOAD_STORAGE_KEY, JSON.stringify(store));
  } catch {
    // quota / unavailable — non-fatal
  }
}

function autodownloadKey(contactId: string, messageId: string) {
  return `${contactId}:${messageId}`;
}

function createMessagesState() {
  const autodownloadPaths: Record<string, string> = loadAutodownloadStore();
  // keyed by contactId
  let map = $state<Record<string, MessageResponse[]>>({});
  // keyed by contactId
  let unreadByContact = $state<Record<string, number>>({});
  // keyed by contactId — the id of the first message that arrived while the
  // chat wasn't being actively viewed. Used to draw a "New messages" separator.
  let firstUnreadByContact = $state<Record<string, string>>({});
  // keyed by `${contactId}:${messageId}`
  let reactions = $state<
    Record<string, Array<{ emoji: string; userIds: string[] }>>
  >({});
  // keyed by transfer/message id
  let transferProgress = $state<
    Record<string, { bytesTransferred: number; totalBytes: number }>
  >({});
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
        if (m.fileDetails) {
          const stored = autodownloadPaths[autodownloadKey(m.contactId, m.id)];
          if (stored) {
            m.fileDetails = { ...m.fileDetails, autodownloadPath: stored };
          }
        }
        if (m.reactions && m.reactions.length > 0) {
          const key = reactionKey(m.contactId, m.id);
          const grouped: Record<string, { emoji: string; userIds: string[] }> =
            {};
          m.reactions.forEach((r) => {
            if (!grouped[r.emoji])
              grouped[r.emoji] = { emoji: r.emoji, userIds: [] };
            if (!grouped[r.emoji].userIds.includes(r.userId)) {
              grouped[r.emoji].userIds.push(r.userId);
            }
          });
          reactions[key] = Object.values(grouped);
        }

        if (m.status === "sending" && now - m.timestamp > SEND_TIMEOUT_MS) {
          m.status = "queued";
        } else if (m.status === "sending") {
          const ts = m.timestamp;
          setTimeout(
            () => expireSendingAt(contactId, ts),
            SEND_TIMEOUT_MS - (now - m.timestamp),
          );
        }
      });
      // Filter out deleted messages
      map[contactId] = msgs.filter((m) => m.content !== "");
    } catch (e) {
      console.error("Failed to load messages for", contactId, e);
    }
    loadedContacts.add(contactId);
  }

  function forContact(contactId: string): MessageResponse[] {
    return map[contactId] ?? [];
  }

  function persistAutodownloadFromMessage(msg: MessageResponse) {
    const path = msg.fileDetails?.autodownloadPath;
    if (!path) return;
    autodownloadPaths[autodownloadKey(msg.contactId, msg.id)] = path;
    saveAutodownloadStore(autodownloadPaths);
  }

  function append(msg: MessageResponse) {
    if (msg.content === "") return;
    if (!map[msg.contactId]) map[msg.contactId] = [];
    const list = map[msg.contactId];
    // Don't add duplicate if already exists
    if (!list.find((m) => m.id === msg.id)) {
      list.push(msg);
      map[msg.contactId] = [...list];
      persistAutodownloadFromMessage(msg);
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
    persistAutodownloadFromMessage(msg);
    if (msg.direction === "sent") clearFirstUnread(msg.contactId);
    const ts = msg.timestamp;
    const cid = msg.contactId;
    setTimeout(() => expireSendingAt(cid, ts), SEND_TIMEOUT_MS);
  }

  // Mark any "sending" message in `contactId` with timestamp <= `beforeTs`
  // as "queued". Keyed by timestamp (not id) so it survives `replaceId`
  // swapping the optimistic UUID for the backend-issued real id.
  function expireSendingAt(contactId: string, beforeTs: number) {
    const list = map[contactId];
    if (!list) return;
    let changed = false;
    for (const m of list) {
      if (m.status === "sending" && m.timestamp <= beforeTs) {
        m.status = "queued";
        changed = true;
      }
    }
    if (changed) map[contactId] = [...list];
  }

  function queuedFor(contactId: string): MessageResponse[] {
    return (map[contactId] ?? []).filter((m) => m.status === "queued");
  }

  function queuedCount(contactId: string): number {
    return queuedFor(contactId).length;
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

  function replaceId(oldId: string, contactId: string, newId: string) {
    const list = map[contactId];
    if (!list) return;
    const msg = list.find((m) => m.id === oldId);
    if (!msg) return;
    msg.id = newId;
    map[contactId] = [...list];
    const oldKey = reactionKey(contactId, oldId);
    if (reactions[oldKey]) {
      const newKey = reactionKey(contactId, newId);
      reactions[newKey] = reactions[oldKey];
      const copy = { ...reactions };
      delete copy[oldKey];
      reactions = copy;
    }
    const oldAutoKey = autodownloadKey(contactId, oldId);
    if (autodownloadPaths[oldAutoKey]) {
      autodownloadPaths[autodownloadKey(contactId, newId)] =
        autodownloadPaths[oldAutoKey];
      delete autodownloadPaths[oldAutoKey];
      saveAutodownloadStore(autodownloadPaths);
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

  function clearAutodownloadEntry(contactId: string, messageId: string) {
    const key = autodownloadKey(contactId, messageId);
    if (autodownloadPaths[key]) {
      delete autodownloadPaths[key];
      saveAutodownloadStore(autodownloadPaths);
    }
  }

  function markDeleted(messageId: string, contactId: string) {
    const list = map[contactId];
    if (!list) return;
    map[contactId] = list.filter((m) => m.id !== messageId);
    clearAutodownloadEntry(contactId, messageId);
  }

  function setAutodownloadPath(
    messageId: string,
    contactId: string,
    path: string | null,
  ) {
    const key = autodownloadKey(contactId, messageId);
    if (path) {
      autodownloadPaths[key] = path;
    } else {
      delete autodownloadPaths[key];
    }
    saveAutodownloadStore(autodownloadPaths);

    const list = map[contactId];
    if (!list) return;
    const msg = list.find((m) => m.id === messageId);
    if (!msg || !msg.fileDetails) return;
    msg.fileDetails = { ...msg.fileDetails, autodownloadPath: path };
    map[contactId] = [...list];
  }

  function removeLocally(messageId: string, contactId: string) {
    const list = map[contactId];
    if (!list) return;
    map[contactId] = list.filter((m) => m.id !== messageId);
    clearAutodownloadEntry(contactId, messageId);
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

  function firstUnreadFor(contactId: string): string | null {
    return firstUnreadByContact[contactId] ?? null;
  }

  function setFirstUnread(contactId: string, messageId: string) {
    if (firstUnreadByContact[contactId]) return;
    firstUnreadByContact[contactId] = messageId;
  }

  function clearFirstUnread(contactId: string) {
    if (!firstUnreadByContact[contactId]) return;
    const next = { ...firstUnreadByContact };
    delete next[contactId];
    firstUnreadByContact = next;
  }

  function addReaction(
    messageId: string,
    contactId: string,
    emoji: string,
    userId: string,
  ) {
    const key = reactionKey(contactId, messageId);
    const current = reactions[key] ?? [];
    const index = current.findIndex((r) => r.emoji === emoji);
    if (index >= 0) {
      if (!current[index].userIds.includes(userId)) {
        current[index].userIds.push(userId);
      }
    } else {
      current.push({ emoji, userIds: [userId] });
    }
    reactions[key] = [...current];
  }

  function removeReaction(
    messageId: string,
    contactId: string,
    emoji: string,
    userId: string,
  ) {
    const key = reactionKey(contactId, messageId);
    let current = reactions[key] ?? [];
    const index = current.findIndex((r) => r.emoji === emoji);
    if (index >= 0) {
      current[index].userIds = current[index].userIds.filter(
        (u) => u !== userId,
      );

      if (current[index].userIds.length === 0) {
        current = current.filter((r) => r.emoji !== emoji);
      } else {
        current = [...current];
      }
      reactions[key] = current;
    }
  }

  function reactionsFor(
    messageId: string,
    contactId: string,
  ): Array<{ emoji: string; userIds: string[] }> {
    return reactions[reactionKey(contactId, messageId)] ?? [];
  }

  function setTransferProgress(
    transferId: string,
    bytesTransferred: number,
    totalBytes: number,
  ) {
    transferProgress[transferId] = { bytesTransferred, totalBytes };
  }

  function transferProgressFor(
    transferId: string,
  ): { bytesTransferred: number; totalBytes: number } | null {
    return transferProgress[transferId] ?? null;
  }

  function clearTransferProgress(transferId: string) {
    if (!transferProgress[transferId]) return;
    const copy = { ...transferProgress };
    delete copy[transferId];
    transferProgress = copy;
  }

  function clearForContact(contactId: string) {
    delete map[contactId];
    delete unreadByContact[contactId];
    delete firstUnreadByContact[contactId];
    const prefix = `${contactId}:`;
    Object.keys(reactions)
      .filter((k) => k.startsWith(prefix))
      .forEach((k) => delete reactions[k]);
    const autoKeys = Object.keys(autodownloadPaths).filter((k) =>
      k.startsWith(prefix),
    );
    if (autoKeys.length > 0) {
      autoKeys.forEach((k) => delete autodownloadPaths[k]);
      saveAutodownloadStore(autodownloadPaths);
    }
    loadedContacts = new Set(
      [...loadedContacts].filter((id) => id !== contactId),
    );
  }

  function clearAll() {
    Object.keys(map).forEach((k) => delete map[k]);
    Object.keys(unreadByContact).forEach((k) => delete unreadByContact[k]);
    Object.keys(firstUnreadByContact).forEach(
      (k) => delete firstUnreadByContact[k],
    );
    Object.keys(reactions).forEach((k) => delete reactions[k]);
    Object.keys(transferProgress).forEach((k) => delete transferProgress[k]);
    loadedContacts = new Set();
    Object.keys(autodownloadPaths).forEach((k) => delete autodownloadPaths[k]);
    saveAutodownloadStore(autodownloadPaths);
  }

  return {
    forContact,
    loadFor,
    append,
    appendOptimistic,
    queuedFor,
    queuedCount,
    updateStatus,
    updateStatusIfSending,
    replaceId,
    updateContent,
    markDeleted,
    setAutodownloadPath,
    removeLocally,
    unreadFor,
    totalUnread,
    markRead,
    firstUnreadFor,
    setFirstUnread,
    clearFirstUnread,
    addReaction,
    removeReaction,
    reactionsFor,
    setTransferProgress,
    transferProgressFor,
    clearTransferProgress,
    clearForContact,
    clearAll,
  };
}

export const messagesState = createMessagesState();
