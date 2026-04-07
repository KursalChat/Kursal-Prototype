import { invoke } from '@tauri-apps/api/core';
import type { MessageResponse } from '$lib/types';

export const sendText = (contactId: string, text: string, replyTo: string | null = null): Promise<string> =>
  invoke('send_text', { contactId, text, replyTo });

export const getMessages = (
  contactId: string,
  limit = 100,
  before: string | null = null,
): Promise<MessageResponse[]> =>
  invoke('get_messages', { contactId, limit, before });
