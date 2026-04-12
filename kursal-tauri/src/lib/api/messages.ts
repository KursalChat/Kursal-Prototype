import { invoke } from '@tauri-apps/api/core';
import type { MessageResponse } from '$lib/types';

export const sendText = (contactId: string, text: string, replyTo: string | null = null): Promise<string> =>
  invoke('send_text', { contactId, text, replyTo });

export const deleteLocalMessage = (contactId: string, messageId: string): Promise<void> =>
  invoke('delete_local_message', { contactId, messageId });

export const deleteMessage = (contactId: string, messageId: string): Promise<void> =>
  invoke('delete_message_for_everyone', { contactId, messageId });

export const editMessage = (contactId: string, messageId: string, newContent: string): Promise<void> =>
  invoke('edit_message', { contactId, messageId, newContent });

export const addReaction = (contactId: string, messageId: string, emoji: string): Promise<void> =>
  invoke('add_reaction', { contactId, messageId, emoji });

export const removeReaction = (contactId: string, messageId: string, emoji: string): Promise<void> =>
  invoke('remove_reaction', { contactId, messageId, emoji });

export const sendFileOffer = (contactId: string, filePath: string): Promise<[string, number]> =>
  invoke('send_file_offer', { contactId, filePath });

export const acceptFileOffer = (contactId: string, offerId: string, savePath: string): Promise<void> =>
  invoke('accept_file_offer', { contactId, offerId, savePath });

export const getMessages = async (
  contactId: string,
  limit = 100,
  before: string | null = null,
): Promise<MessageResponse[]> => {
  const msgs = await invoke<MessageResponse[]>('get_messages', { contactId, limit, before });
  // The Rust backend stores and returns timestamps in seconds.
  // We convert them to JS milliseconds at the API boundary so the entire frontend
  // can reliably expect 'timestamp' to be in milliseconds (e.g. for new Date()).
  return msgs.map(m => {
    m.timestamp = m.timestamp * 1000;
    return m;
  });
};
