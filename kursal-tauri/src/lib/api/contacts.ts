import { invoke } from '@tauri-apps/api/core';
import type { ContactResponse } from '$lib/types';

export const getContacts = (): Promise<ContactResponse[]> =>
  invoke('get_contacts');

export const getSecurityCode = (contactId: string): Promise<string> =>
  invoke('get_security_code', { contactId });

export const removeContact = (contactId: string): Promise<void> =>
  invoke('remove_contact', { contactId });

export const confirmSecurityCode = (contactId: string): Promise<void> =>
  invoke('confirm_security_code', { contactId });

export const setContactBlocked = (contactId: string, value: boolean): Promise<void> =>
  invoke('set_contact_blocked', { contactId, value });
