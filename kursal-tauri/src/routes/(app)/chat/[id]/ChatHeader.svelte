<script lang="ts">
  import { Menu, ShieldAlert } from "lucide-svelte";
  import Avatar from "$lib/components/Avatar.svelte";
  import { contactsState } from "$lib/state/contacts.svelte";
  import { uiState } from "$lib/state/ui.svelte";
  import type { ContactResponse } from "$lib/types";
  import { t } from "$lib/i18n";

  interface Props {
    contact: ContactResponse;
    onOpenProfile: () => void;
    onOpenSecurity: () => void;
  }

  let { contact, onOpenProfile, onOpenSecurity }: Props = $props();

  const status = $derived(
    contactsState.connectionStatus[contact.userId] ?? "disconnected",
  );

  const statusLabel = $derived.by(() => {
    if (status === "direct" || status === "holepunch") return t("chat.header.statusOnline");
    if (status === "relay") return t("chat.header.statusRelay");
    if (status === "connecting") return t("chat.header.statusConnecting");
    return t("chat.header.statusOffline");
  });
</script>

<header class="chat-header" data-tauri-drag-region>
  <div class="header-left">
    <button
      class="menu-btn"
      onclick={() => (uiState.mobileSidebarOpen = true)}
      aria-label={t("chat.header.openSidebar")}
    >
      <Menu size={20} />
    </button>
    <button
      class="header-profile"
      onclick={onOpenProfile}
      aria-label={t("chat.header.viewProfile")}
    >
      <Avatar
        name={contact.displayName}
        src={contact.avatarBase64}
        size={34}
        {status}
        showStatus
      />
      <div class="header-info">
        <span class="header-name">{contact.displayName}</span>
        <span class="header-status">{statusLabel}</span>
      </div>
    </button>
  </div>
  <div class="header-right">
    {#if !contact.verified}
      <button
        class="verify-btn"
        title={t("chat.header.verifyIdentity")}
        onclick={onOpenSecurity}
        aria-label={t("chat.header.verifyIdentity")}
      >
        <ShieldAlert size={16} />
      </button>
    {/if}
  </div>
</header>

<style>
  .chat-header {
    height: calc(var(--header-height) + env(safe-area-inset-top, 0px));
    padding: env(safe-area-inset-top, 0px) 12px 0;
    display: flex;
    align-items: center;
    justify-content: space-between;
    border-bottom: 1px solid var(--border);
    background: var(--surface);
    backdrop-filter: blur(16px);
    -webkit-backdrop-filter: blur(16px);
    flex-shrink: 0;
    z-index: 10;
  }
  .header-left {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
  }
  .menu-btn {
    display: none;
    width: 36px;
    height: 36px;
    align-items: center;
    justify-content: center;
    border-radius: var(--radius-md);
    color: var(--text-secondary);
    transition: background var(--transition);
    -webkit-app-region: no-drag;
  }
  .menu-btn:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }
  .menu-btn:active {
    background: var(--bg-hover);
    transform: scale(0.96);
  }
  @media (max-width: 768px) {
    .menu-btn {
      display: flex;
    }
  }

  .header-profile {
    display: inline-flex;
    align-items: center;
    gap: 12px;
    padding: 4px 10px 4px 6px;
    border-radius: 999px;
    transition: background var(--transition);
    min-width: 0;
    max-width: min(320px, 60vw);
    -webkit-app-region: no-drag;
  }
  .header-profile:hover {
    background: var(--bg-hover);
  }
  .header-info {
    display: flex;
    flex-direction: column;
    min-width: 0;
    align-items: flex-start;
    gap: 1px;
  }
  .header-name {
    font-size: 14.5px;
    font-weight: 600;
    line-height: 1.2;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 100%;
  }
  .header-status {
    font-size: 12px;
    color: var(--text-muted);
    line-height: 1.2;
  }
  .header-right {
    display: flex;
    gap: 4px;
    -webkit-app-region: no-drag;
  }
  .verify-btn {
    width: 36px;
    height: 36px;
    border-radius: var(--radius-md);
    background: rgba(251, 191, 36, 0.12);
    color: var(--warning);
    display: flex;
    align-items: center;
    justify-content: center;
    transition: background var(--transition);
  }
  .verify-btn:hover {
    background: rgba(251, 191, 36, 0.22);
  }
  .verify-btn:active {
    transform: scale(0.95);
  }
</style>
