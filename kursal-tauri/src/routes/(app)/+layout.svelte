<script lang="ts">
  import { goto, afterNavigate } from "$app/navigation";
  import { onMount } from "svelte";
  import { page } from "$app/state";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { contactsState } from "$lib/state/contacts.svelte";
  import { messagesState } from "$lib/state/messages.svelte";
  import { profileState } from "$lib/state/profile.svelte";
  import {
    UserPlus,
    Settings as SettingsIcon,
    Menu,
    X,
    MessageSquare,
    Search,
  } from "lucide-svelte";
  import Avatar from "$lib/components/Avatar.svelte";
  import StatusDot from "$lib/components/StatusDot.svelte";
  import WinstonContactsTour from "$lib/components/WinstonContactsTour.svelte";
  import { setBadgeCount } from "$lib/api/window";
  import { uiState } from "$lib/state/ui.svelte";

  let { children } = $props();

  afterNavigate(({ to }) => {
    if (to?.route.id?.includes("[id]") && to?.params?.id) {
      localStorage.setItem("kursal_last_chat", to.params.id);
    }
  });

  function handleAddContact() {
    uiState.mobileSidebarOpen = false;
    goto("/add-contact");
  }

  function handleSettings() {
    uiState.mobileSidebarOpen = false;
    goto("/settings");
  }

  const totalUnread = $derived(messagesState.totalUnread());

  $effect(() => {
    const count = totalUnread;
    (async () => {
      await setBadgeCount(count);
    })();
  });

  let search = $state("");

  onMount(() => {
    let unlisten: UnlistenFn | undefined;

    (async () => {
      await profileState.load();

      try {
        unlisten = await listen<string[]>("peer_id_rotated", async () => {
          await profileState.refreshPeerId();
        });

        await listen<string>("contact_removed", (e) => {
          const removedId = e.payload;
          contactsState.remove(removedId);
          if (currentChatId === removedId) {
            goto("/chat", { replaceState: true });
          }
        });
      } catch (e) {
        console.error("Failed to set up listeners:", e);
      }
    })();

    return () => {
      unlisten?.();
    };
  });

  const currentChatId = $derived(page.params.id);

  function getLastMessage(contactId: string): { text: string; ts: number } {
    const msgs = messagesState.forContact(contactId);
    if (msgs.length === 0) return { text: "", ts: 0 };
    const last = msgs[msgs.length - 1];
    const text = last.fileDetails
      ? `📎 ${last.fileDetails.filename}`
      : last.content.replace(/\s+/g, " ").trim();
    const display = text.length > 44 ? text.slice(0, 41) + "..." : text;
    return {
      text: (last.direction === "sent" ? "You: " : "") + display,
      ts: last.timestamp,
    };
  }

  function formatTimeShort(ts: number): string {
    if (!ts) return "";
    const d = new Date(ts);
    const now = new Date();
    const diffMs = now.getTime() - ts;
    if (diffMs < 24 * 3600 * 1000 && d.getDate() === now.getDate()) {
      return d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
    }
    if (diffMs < 7 * 24 * 3600 * 1000) {
      return d.toLocaleDateString([], { weekday: "short" });
    }
    return d.toLocaleDateString([], { month: "short", day: "numeric" });
  }

  function getStatusLabel(status: string | undefined): string {
    if (!status || status === "disconnected") return "Offline";
    if (status === "direct") return "Online";
    if (status === "holepunch") return "Online";
    if (status === "relay") return "Online · relay";
    return status.charAt(0).toUpperCase() + status.slice(1);
  }

  const filteredContacts = $derived.by(() => {
    const q = search.trim().toLowerCase();
    if (!q) return contactsState.contacts;
    return contactsState.contacts.filter((c) =>
      c.displayName.toLowerCase().includes(q),
    );
  });

  const sortedContacts = $derived.by(() => {
    const list = [...filteredContacts];
    list.sort((a, b) => {
      const aMsgs = messagesState.forContact(a.userId);
      const bMsgs = messagesState.forContact(b.userId);
      const aTs = aMsgs.length
        ? aMsgs[aMsgs.length - 1].timestamp
        : a.createdAt * 1000;
      const bTs = bMsgs.length
        ? bMsgs[bMsgs.length - 1].timestamp
        : b.createdAt * 1000;
      return bTs - aTs;
    });
    return list;
  });
</script>

<div class="shell" class:chat-active={!!currentChatId}>
  {#if uiState.mobileSidebarOpen}
    <div
      class="backdrop"
      onclick={() => (uiState.mobileSidebarOpen = false)}
      aria-hidden="true"
    ></div>
  {/if}

  <div class="mobile-bar" class:hidden={!!currentChatId} data-tauri-drag-region>
    <button
      class="mobile-menu"
      onclick={() => (uiState.mobileSidebarOpen = true)}
      aria-label="Open menu"
    >
      <Menu size={22} />
    </button>
    <span class="mobile-title">Kursal</span>
  </div>

  <aside class="sidebar" class:open={uiState.mobileSidebarOpen}>
    <div class="sidebar-header" data-tauri-drag-region>
      <h1>Kursal</h1>
      <div class="header-actions">
        <button
          class="icon-btn"
          onclick={handleAddContact}
          title="Add contact"
          aria-label="Add contact"
          data-tour="add-contact-btn"
        >
          <UserPlus size={17} />
        </button>
        <button
          class="icon-btn"
          onclick={handleSettings}
          title="Settings"
          aria-label="Settings"
        >
          <SettingsIcon size={17} />
        </button>
        <button
          class="icon-btn mobile-close"
          onclick={() => (uiState.mobileSidebarOpen = false)}
          aria-label="Close menu"
        >
          <X size={18} />
        </button>
      </div>
    </div>

    <div class="search-wrap">
      <Search size={14} />
      <input
        type="text"
        placeholder="Search contacts"
        bind:value={search}
        spellcheck="false"
        autocorrect="off"
        autocapitalize="off"
      />
      {#if search}
        <button
          class="clear-search"
          onclick={() => (search = "")}
          aria-label="Clear search"><X size={12} /></button
        >
      {/if}
    </div>

    <div class="contacts-list">
      {#if contactsState.loading}
        <div class="empty-state">Loading...</div>
      {:else if contactsState.contacts.length === 0}
        <div class="empty-state column">
          <MessageSquare size={24} />
          <span>No contacts yet</span>
          <button
            class="empty-cta"
            onclick={handleAddContact}
            data-tour="add-contact-empty"
          >
            <UserPlus size={14} /> Add your first contact
          </button>
        </div>
      {:else if filteredContacts.length === 0}
        <div class="empty-state">No contacts match</div>
      {:else}
        {#each sortedContacts as contact (contact.userId)}
          {@const unread = messagesState.unreadFor(contact.userId)}
          {@const status = contactsState.connectionStatus[contact.userId]}
          {@const preview = getLastMessage(contact.userId)}
          <button
            class="contact-row"
            class:active={currentChatId === contact.userId}
            class:unread={unread > 0}
            onclick={() => {
              goto("/chat/" + contact.userId);
              uiState.mobileSidebarOpen = false;
            }}
          >
            <div class="contact-avatar">
              <Avatar
                name={contact.displayName}
                src={contact.avatarBase64}
                size={42}
              />
              <StatusDot status={status ?? "disconnected"} />
            </div>
            <div class="contact-meta">
              <div class="contact-top">
                <span class="contact-name">{contact.displayName}</span>
                {#if preview.ts}
                  <span class="contact-time">{formatTimeShort(preview.ts)}</span
                  >
                {/if}
              </div>
              <div class="contact-bottom">
                {#if preview.text}
                  <span class="contact-preview">{preview.text}</span>
                {:else}
                  <span class="contact-status-text"
                    >{getStatusLabel(status)}</span
                  >
                {/if}
                {#if unread > 0}
                  <span class="badge">{unread > 99 ? "99+" : unread}</span>
                {/if}
              </div>
            </div>
          </button>
        {/each}
      {/if}
    </div>

    <button
      class="user-panel"
      onclick={handleSettings}
      aria-label="Open settings"
    >
      <Avatar
        name={profileState.displayName}
        src={profileState.avatarBase64}
        size={36}
      />
      <div class="user-info">
        <span class="user-name">{profileState.displayName}</span>
        <span class="user-id"
          >{profileState.peerId
            ? profileState.peerId.slice(0, 10) + "..."
            : "..."}</span
        >
      </div>
      {#if totalUnread > 0}
        <span class="badge total">{totalUnread > 99 ? "99+" : totalUnread}</span
        >
      {/if}
    </button>
  </aside>

  <main class="content">
    {@render children()}
  </main>
</div>

<WinstonContactsTour />

<style>
  .shell {
    display: flex;
    height: 100%;
    overflow: hidden;
  }

  .backdrop {
    display: none;
  }
  .mobile-bar {
    display: none;
  }
  .icon-btn.mobile-close {
    display: none;
  }

  /* Sidebar */
  .sidebar {
    width: var(--sidebar-width);
    background: var(--panel);
    backdrop-filter: blur(24px) saturate(140%);
    -webkit-backdrop-filter: blur(24px) saturate(140%);
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    box-shadow: inset -1px 0 0 var(--panel-border);
    overflow: hidden;
  }

  .sidebar-header {
    height: var(--header-height);
    padding: 0 12px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
  }

  :global(html.mac) .sidebar-header {
    padding-left: 78px;
  }

  .sidebar-header :global(button),
  .mobile-bar :global(button),
  .mobile-title {
    -webkit-app-region: no-drag;
  }

  .sidebar-header h1 {
    font-size: 18px;
    font-weight: 700;
    letter-spacing: -0.02em;
  }

  .header-actions {
    display: flex;
    gap: 2px;
  }

  .icon-btn {
    width: 34px;
    height: 34px;
    border-radius: var(--radius-md);
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-secondary);
    transition: all var(--transition);
  }
  .icon-btn:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }
  .icon-btn:active {
    transform: scale(0.95);
  }

  /* Search */
  .search-wrap {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 8px 10px 4px;
    padding: 7px 10px;
    background: var(--bg-input);
    border: 1px solid var(--border);
    border-radius: 10px;
    color: var(--text-muted);
    transition: border-color var(--transition);
  }
  .search-wrap:focus-within {
    border-color: var(--accent-selected);
  }
  .search-wrap input {
    flex: 1;
    background: transparent;
    border: none;
    outline: none;
    color: var(--text-primary);
    font-size: 13px;
  }
  .search-wrap input::placeholder {
    color: var(--text-muted);
  }
  .clear-search {
    width: 18px;
    height: 18px;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(148, 163, 184, 0.2);
    color: var(--text-muted);
  }
  .clear-search:hover {
    background: rgba(148, 163, 184, 0.35);
  }

  /* Contact list */
  .contacts-list {
    flex: 1;
    overflow-y: auto;
    padding: 6px 8px 8px;
  }

  .empty-state {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    padding: 24px 12px;
    color: var(--text-muted);
    font-size: 13px;
  }
  .empty-state.column {
    flex-direction: column;
    padding: 40px 16px;
    text-align: center;
  }
  .empty-cta {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    margin-top: 8px;
    padding: 8px 14px;
    border-radius: 999px;
    background: var(--accent-dim);
    color: var(--accent-hover);
    font-size: 12px;
    font-weight: 600;
    transition: background var(--transition);
  }
  .empty-cta:hover {
    background: color-mix(in srgb, var(--accent) 22%, transparent);
  }

  .contact-row {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 9px 10px;
    border-radius: var(--radius-md);
    text-align: left;
    transition: background var(--transition);
    margin-bottom: 1px;
  }
  .contact-row:hover {
    background: var(--bg-hover);
  }
  .contact-row:active {
    background: color-mix(in srgb, var(--accent-solid) 20%, transparent);
  }
  .contact-row.active {
    background: linear-gradient(
      135deg,
      var(--accent-solid),
      color-mix(in srgb, var(--accent-solid), black 18%)
    );
    color: #fff;
    box-shadow: 0 4px 12px var(--accent-dim);
  }
  .contact-row.active .contact-name,
  .contact-row.active .contact-preview,
  .contact-row.active .contact-status-text,
  .contact-row.active .contact-time {
    color: #fff;
  }
  .contact-row.active .contact-preview,
  .contact-row.active .contact-status-text {
    opacity: 0.88;
  }
  .contact-row.active .contact-time {
    opacity: 0.8;
  }
  .contact-row.active .badge {
    background: #fff;
    color: var(--accent-solid);
  }

  .contact-avatar {
    position: relative;
    flex-shrink: 0;
  }
  .contact-avatar :global(.status-dot) {
    position: absolute;
    bottom: -1px;
    right: -1px;
    border: 2px solid var(--bg-secondary);
  }
  .contact-row.active .contact-avatar :global(.status-dot) {
    border-color: var(--accent-solid);
  }

  .contact-meta {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .contact-top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }
  .contact-name {
    font-size: 14px;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--text-primary);
    flex: 1;
    min-width: 0;
  }
  .contact-row.unread .contact-name {
    font-weight: 700;
  }
  .contact-time {
    font-size: 11px;
    color: var(--text-muted);
    flex-shrink: 0;
  }
  .contact-row.unread .contact-time {
    color: var(--accent);
    font-weight: 600;
  }

  .contact-bottom {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }
  .contact-preview,
  .contact-status-text {
    font-size: 12.5px;
    color: var(--text-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    flex: 1;
    min-width: 0;
  }
  .contact-row.unread .contact-preview {
    color: var(--text-secondary);
  }

  .badge {
    flex-shrink: 0;
    min-width: 20px;
    height: 20px;
    padding: 0 6px;
    border-radius: 999px;
    background: var(--accent);
    color: #fff;
    font-size: 11px;
    font-weight: 700;
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }
  .badge.total {
    min-width: 22px;
    height: 22px;
    font-size: 11px;
    margin-left: auto;
  }

  /* User panel (as button) */
  .user-panel {
    width: 100%;
    min-height: 55px;
    padding-left: 12px;
    padding-right: 12px;
    border-top: 1px solid var(--border);
    display: flex;
    align-items: center;
    gap: 10px;
    flex-shrink: 0;
    background: transparent;
    text-align: left;
    transition: background var(--transition);
  }
  .user-panel:hover {
    background: var(--bg-hover);
  }
  .user-info {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .user-name {
    font-size: 13px;
    font-weight: 600;
    color: var(--text-primary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .user-id {
    font-size: 11px;
    color: var(--text-muted);
    font-family: ui-monospace, SFMono-Regular, monospace;
  }

  /* Main content */
  .content {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    display: flex;
    flex-direction: column;
    background: transparent;
  }

  /* Mobile responsive */
  @media (max-width: 768px) {
    .sidebar {
      position: fixed;
      top: 0;
      left: 0;
      bottom: 0;
      width: 300px;
      max-width: 86vw;
      z-index: 60;
      transform: translateX(-100%);
      transition: transform 0.26s cubic-bezier(0.4, 0, 0.2, 1);
      box-shadow: 4px 0 24px rgba(0, 0, 0, 0.45);
    }
    .sidebar.open {
      transform: translateX(0);
    }

    .backdrop {
      display: block;
      position: fixed;
      inset: 0;
      background: rgba(0, 0, 0, 0.6);
      z-index: 50;
      animation: fadeIn 0.2s ease;
    }
    @keyframes fadeIn {
      from {
        opacity: 0;
      }
      to {
        opacity: 1;
      }
    }

    .mobile-bar {
      display: flex;
      align-items: center;
      gap: 8px;
      position: fixed;
      top: 0;
      left: 0;
      right: 0;
      height: var(--header-height);
      padding: 0 6px;
      background: var(--panel);
      backdrop-filter: blur(20px) saturate(140%);
      -webkit-backdrop-filter: blur(20px) saturate(140%);
      border-bottom: 1px solid var(--border);
      z-index: 30;
    }
    .mobile-bar.hidden {
      display: none;
    }
    .mobile-menu {
      width: 40px;
      height: 40px;
      display: flex;
      align-items: center;
      justify-content: center;
      color: var(--text-primary);
      border-radius: var(--radius-md);
    }
    .mobile-menu:active {
      background: var(--bg-hover);
    }
    .mobile-title {
      flex: 1;
      text-align: center;
      font-size: 16px;
      font-weight: 700;
    }

    .icon-btn.mobile-close {
      display: flex;
    }

    .content {
      width: 100%;
    }
    .shell:not(.chat-active) .content {
      padding-top: var(--header-height);
    }
    .contact-row {
      padding: 11px 10px;
    }
    .icon-btn {
      width: 38px;
      height: 38px;
    }
  }

  @media (max-width: 960px) and (min-width: 769px) {
    .sidebar {
      width: 240px;
    }
  }
</style>
