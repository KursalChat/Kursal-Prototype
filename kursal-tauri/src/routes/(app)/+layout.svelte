<script lang="ts">
  import { goto, afterNavigate } from "$app/navigation";
  import { onMount } from "svelte";
  import { page } from "$app/state";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { contactsState } from "$lib/state/contacts.svelte";
  import { messagesState } from "$lib/state/messages.svelte";
  import { profileState } from "$lib/state/profile.svelte";
  import { getLocalPeerId, getLocalUserProfile } from "$lib/api/identity";
  import { UserPlus, Settings as SettingsIcon } from "lucide-svelte";
  import Avatar from "$lib/components/Avatar.svelte";
  import StatusDot from "$lib/components/StatusDot.svelte";

  let { children } = $props();

  // Track last visited chat and persist it
  afterNavigate(({ to }) => {
    if (to?.route.id?.includes("[id]") && to?.params?.id) {
      localStorage.setItem("kursal_last_chat", to.params.id);
    }
  });

  function handleAddContact() {
    goto("/add-contact");
  }

  function handleSettings() {
    goto("/settings");
  }

  const totalUnread = $derived(messagesState.totalUnread());

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

    // sync cleanup — Svelte calls this on destroy
    return () => {
      unlisten?.();
    };
  });

  const currentChatId = $derived(page.params.id);
</script>

<div class="shell" class:chat-active={!!currentChatId}>
  <aside class="sidebar">
    <div class="header glass">
      <div class="brand-block">
        <h1>Kursal</h1>
      </div>
      <div class="header-actions">
        <button
          class="action-btn action-primary"
          title="Add contact"
          aria-label="Add contact"
          onclick={handleAddContact}
        >
          <UserPlus size={16} />
        </button>
        <button
          class="action-btn"
          title="Settings"
          aria-label="Settings"
          onclick={handleSettings}
        >
          <SettingsIcon size={16} />
        </button>
      </div>
    </div>

    <div class="contacts">
      {#if contactsState.loading}
        <div class="loading-message">Loading contacts...</div>
      {:else if contactsState.contacts.length === 0}
        <div class="empty-message">
          No contacts yet. Tap Add Contact to start.
        </div>
      {:else}
        {#each contactsState.contacts as contact (contact.userId)}
          <button
            class="contact-item"
            class:active={currentChatId === contact.userId}
            onclick={() => goto("/chat/" + contact.userId)}
          >
            <Avatar name={contact.displayName} src={contact.avatarBase64} size={36} />
            <div class="contact-info">
              <div class="contact-name-row">
                <div class="contact-name">{contact.displayName}</div>
                {#if messagesState.unreadFor(contact.userId) > 0}
                  <span class="unread-pill" aria-label={`${messagesState.unreadFor(contact.userId)} unread messages`}>
                    {messagesState.unreadFor(contact.userId)}
                  </span>
                {/if}
              </div>
              <div class="contact-status">
                {#if contactsState.connectionStatus[contact.userId]}
                  <StatusDot
                    status={contactsState.connectionStatus[contact.userId]}
                  />
                  <span class="status-text">
                    {contactsState.connectionStatus[contact.userId]}
                  </span>
                {:else}
                  <StatusDot status="disconnected" />
                  <span class="status-text">offline</span>
                {/if}
              </div>
            </div>
          </button>
        {/each}
      {/if}
    </div>

    <div class="user-panel">
      <Avatar name={profileState.displayName} src={profileState.avatarBase64} size={42} />
      <div class="user-meta">
        <p class="user-name">{profileState.displayName}</p>
        <p class="user-id">
          {profileState.peerId
            ? profileState.peerId.slice(0, 12) + "..."
            : "Loading ID..."}
        </p>
      </div>
      {#if totalUnread > 0}
        <span class="total-unread" aria-label={`${totalUnread} total unread messages`}>{totalUnread}</span>
      {/if}
    </div>
  </aside>

  <main class="content">
    {@render children()}
  </main>
</div>

<style>
  .shell {
    display: flex;
    height: 100dvh;
    overflow: hidden;
    padding: 0;
    gap: 0;
  }

  .sidebar {
    width: var(--sidebar-width);
    background: var(--surface);
    backdrop-filter: blur(18px);
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    border-right: 1px solid var(--border);
    overflow: hidden;
  }

  .content {
    flex: 1;
    min-width: 0;
    position: relative;
    background: var(--bg-primary);
  }

  @media (max-width: 768px) {
    .sidebar {
      width: 100%;
    }
    .shell.chat-active .sidebar {
      display: none;
    }
    .shell:not(.chat-active) .content {
      display: none;
    }
  }

  .glass {
    background: rgba(15, 23, 42, 0.5);
  }

  .header {
    min-height: var(--header-height);
    padding: 0 14px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    border-bottom: 1px solid var(--border);
    backdrop-filter: blur(20px);
  }

  .brand-block {
    display: grid;
    gap: 2px;
  }

  .header h1 {
    margin: 0;
    font-size: 22px;
    font-weight: 700;
    letter-spacing: -0.02em;
  }

  .header-actions {
    display: flex;
    gap: 6px;
    align-items: center;
  }

  .action-btn {
    border: 1px solid var(--border);
    background: rgba(51, 65, 85, 0.62);
    color: var(--text-secondary);
    width: 36px;
    height: 36px;
    padding: 0;
    border-radius: 10px;
    line-height: 1;
    cursor: pointer;
    transition: all var(--transition);
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }

  .action-btn:hover {
    background: rgba(71, 85, 105, 0.9);
    color: var(--text-primary);
  }

  .action-primary {
    background: linear-gradient(135deg, #6366f1, #7c83f6);
    color: #eef2ff;
    border-color: rgba(165, 180, 252, 0.35);
  }

  .action-primary:hover {
    background: linear-gradient(135deg, #7a7ef8, #9198fc);
    color: #eef2ff;
  }

  .contacts {
    flex: 1;
    overflow-y: auto;
    padding: 12px;
  }

  .user-panel {
    margin: 0 12px 12px;
    border: 1px solid rgba(148, 163, 184, 0.24);
    background: rgba(15, 23, 42, 0.5);
    backdrop-filter: blur(24px);
    border-radius: 12px;
    padding: 12px 14px;
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .user-meta {
    min-width: 0;
  }

  .user-name {
    margin: 0;
    font-size: 14px;
    font-weight: 600;
    color: var(--text-primary);
  }

  .user-id {
    margin: 2px 0 0;
    font-size: 11px;
    color: var(--text-muted);
    font-family: "Monaco", "Courier New", monospace;
  }

  .empty-message,
  .loading-message {
    padding: 18px 12px;
    text-align: left;
    color: var(--text-muted);
    font-size: 12px;
    border: 1px dashed rgba(148, 163, 184, 0.28);
    border-radius: 12px;
    background: rgba(30, 41, 59, 0.42);
  }

  .contact-item {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px;
    background: rgba(30, 41, 59, 0.5);
    border: 1px solid transparent;
    color: inherit;
    cursor: pointer;
    border-radius: 12px;
    text-align: left;
    transition: all var(--transition);
    margin-bottom: 6px;
  }

  .contact-item:hover {
    background: rgba(51, 65, 85, 0.8);
    border-color: rgba(148, 163, 184, 0.42);
    transform: translateX(2px);
  }

  .contact-item.active {
    background: linear-gradient(
      135deg,
      rgba(67, 56, 202, 0.42),
      rgba(79, 70, 229, 0.26)
    );
    border-color: rgba(129, 140, 248, 0.55);
    box-shadow: inset 0 0 0 1px rgba(129, 140, 248, 0.25);
  }

  .contact-info {
    flex: 1;
    min-width: 0;
  }

  .contact-name-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    min-width: 0;
  }

  .contact-name {
    font-size: 14px;
    font-weight: 500;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .unread-pill {
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

  .total-unread {
    margin-left: auto;
    min-width: 24px;
    height: 24px;
    border-radius: 999px;
    padding: 0 7px;
    background: var(--accent);
    color: #fff;
    font-size: 12px;
    font-weight: 700;
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }

  .contact-status {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 11px;
    color: var(--text-muted);
    margin-top: 3px;
  }

  .status-text {
    text-transform: capitalize;
  }

  .content {
    flex: 1;
    overflow: hidden;
    display: flex;
    flex-direction: column;
    background: var(--surface);
    backdrop-filter: blur(12px);
  }

  @media (max-width: 960px) {
    .shell {
      padding: 0;
      gap: 0;
    }

    .sidebar {
      width: 272px;
    }
  }
</style>
