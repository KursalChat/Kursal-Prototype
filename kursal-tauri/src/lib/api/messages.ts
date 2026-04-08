import { invoke } from '@tauri-apps/api/core';
import type { MessageResponse } from '$lib/types';

export const sendText = (contactId: string, text: string, replyTo: string | null = null): Promise<string> =>
  invoke('send_text', { contactId, text, replyTo });

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
