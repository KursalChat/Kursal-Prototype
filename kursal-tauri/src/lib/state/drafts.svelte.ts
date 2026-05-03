function createDraftsState() {
  let drafts = $state<Record<string, string>>({});

  function get(contactId: string): string {
    return drafts[contactId] ?? "";
  }

  function set(contactId: string, text: string) {
    if (text) drafts[contactId] = text;
    else delete drafts[contactId];
  }

  function clear(contactId: string) {
    delete drafts[contactId];
  }

  return { get, set, clear };
}

export const draftsState = createDraftsState();
