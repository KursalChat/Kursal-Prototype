import type { ContactResponse, ConnectionChangedPayload } from '$lib/types';
import { getContacts } from '$lib/api/contacts';

function bytesToBase64(bytes: number[]): string {
    let binary = '';
    const len = bytes.length;
    for (let i = 0; i < len; i++) {
        binary += String.fromCharCode(bytes[i]);
    }
    return btoa(binary);
}

function createContactsState() {
  let contacts = $state<ContactResponse[]>([]);
  let loading = $state(false);
  // Map contactId → connection status
  let connectionStatus = $state<Record<string, ConnectionChangedPayload['status']>>({});

  async function load() {
    loading = true;
    try {
      const result = await getContacts();
      contacts = result.map(c => {
        if (c.avatarBytes && !c.avatarBase64) {
          c.avatarBase64 = bytesToBase64(c.avatarBytes);
        }
        return c;
      });
    } catch (e) {
      console.error('Failed to load contacts:', e);
    } finally {
      loading = false;
    }
  }

  function upsert(contact: ContactResponse) {
    if (contact.avatarBytes && !contact.avatarBase64) {
      contact.avatarBase64 = bytesToBase64(contact.avatarBytes);
    }
    
    const idx = contacts.findIndex(c => c.userId === contact.userId);
    if (idx >= 0) contacts[idx] = contact;
    else contacts.push(contact);
  }

  function remove(contactId: string) {
    const idx = contacts.findIndex(c => c.userId === contactId);
    if (idx >= 0) contacts.splice(idx, 1);
    delete connectionStatus[contactId];
  }

  function markVerified(contactId: string) {
    const c = contacts.find(c => c.userId === contactId);
    if (c) c.verified = true;
  }

  function setConnectionStatus(contactId: string, status: ConnectionChangedPayload['status']) {
    connectionStatus[contactId] = status;
  }

  function getById(id: string) {
    return contacts.find(c => c.userId === id);
  }

  return {
    get contacts() { return contacts; },
    get loading() { return loading; },
    get connectionStatus() { return connectionStatus; },
    load, upsert, remove, markVerified, setConnectionStatus, getById,
  };
}

export const contactsState = createContactsState();
