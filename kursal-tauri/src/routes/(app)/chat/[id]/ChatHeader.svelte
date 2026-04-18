<script lang="ts">
  import { goto } from "$app/navigation";
  import { ArrowLeft, ShieldAlert } from "lucide-svelte";
  import Avatar from "$lib/components/Avatar.svelte";
  import StatusDot from "$lib/components/StatusDot.svelte";
  import { contactsState } from "$lib/state/contacts.svelte";
  import { getStatusLabel } from "./chat-utils";
  import type { ContactResponse } from "$lib/types";

  interface Props {
    contact: ContactResponse;
    onOpenProfile: () => void;
    onOpenSecurity: () => void;
  }

  let { contact, onOpenProfile, onOpenSecurity }: Props = $props();
</script>

<header class="chat-header" data-tauri-drag-region>
  <div class="header-left">
    <button class="back-btn" onclick={() => goto("/chat")} aria-label="Back">
      <ArrowLeft size={20} />
    </button>
    <button
      class="header-profile"
      onclick={onOpenProfile}
      aria-label="View profile"
    >
      <Avatar
        name={contact.displayName}
        src={contact.avatarBase64}
        size={34}
      />
      <div class="header-info">
        <span class="header-name">{contact.displayName}</span>
        <span class="header-status">
          <StatusDot
            status={contactsState.connectionStatus[contact.userId] ??
              "disconnected"}
          />
          {getStatusLabel(contactsState.connectionStatus[contact.userId])}
        </span>
      </div>
    </button>
  </div>
  <div class="header-right">
    {#if !contact.verified}
      <button
        class="verify-btn"
        title="Verify identity"
        onclick={onOpenSecurity}
        aria-label="Verify identity"
      >
        <ShieldAlert size={16} />
      </button>
    {/if}
  </div>
</header>

<style>
  .chat-header {
    height: var(--header-height);
    padding: 0 12px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    border-bottom: 1px solid var(--border);
    background: rgba(17, 24, 39, 0.85);
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
  .back-btn {
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
  .back-btn:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }
  .back-btn:active {
    background: var(--bg-hover);
    transform: scale(0.96);
  }
  @media (max-width: 768px) {
    .back-btn {
      display: flex;
    }
  }

  .header-profile {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 4px 8px;
    border-radius: var(--radius-md);
    transition: background var(--transition);
    min-width: 0;
    flex: 1;
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
  }
  .header-name {
    font-size: 14px;
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
    display: flex;
    align-items: center;
    gap: 5px;
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
