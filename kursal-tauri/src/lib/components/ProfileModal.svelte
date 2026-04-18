<script lang="ts">
  import { X, Trash2, Ban, Shield, Copy } from "lucide-svelte";
  import Avatar from "./Avatar.svelte";
  import SecurityCodeModal from "./SecurityCodeModal.svelte";
  import { notifications } from "$lib/state/notifications.svelte";
  import { contactsState } from "$lib/state/contacts.svelte";
  import type { ContactResponse } from "$lib/types";
  import { removeContact, setContactBlocked } from "$lib/api/contacts";
  import { confirm } from "@tauri-apps/plugin-dialog";

  let {
    contact,
    onClose,
  }: { contact: ContactResponse | null; onClose: () => void } = $props();

  let showSecurityModal = $state(false);

  async function copyUserId() {
    if (!contact) return;
    try {
      await navigator.clipboard.writeText(contact.userId);
      notifications.push("User ID copied", "success");
    } catch (e) {
      console.error("Copy failed", e);
    }
  }

  async function handleToggleBlock() {
    if (!contact) return;
    const userId = contact.userId;
    const displayName = contact.displayName;
    const willBlock = !contact.blocked;

    const confirmed = await confirm(
      `Are you sure you want to ${willBlock ? "block" : "unblock"} ${displayName}?`,
      { title: `${willBlock ? "Block" : "Unblock"} Contact`, kind: "warning" },
    );

    if (!confirmed) return;

    try {
      await setContactBlocked(userId, willBlock);
      contactsState.upsert({ ...contact, blocked: willBlock });
      notifications.push(`${displayName} ${willBlock ? "blocked" : "unblocked"}`, "success");
    } catch (e) {
      notifications.push(`Failed to ${willBlock ? "block" : "unblock"} contact`, "error");
      console.error(e);
    }
  }

  async function handleRemoveContact() {
    if (!contact) return;

    const userId = contact.userId;
    const displayName = contact.displayName;

    // Tauri's confirm returns a Promise<boolean>
    const confirmed = await confirm(
      `Are you sure you want to remove ${displayName}?\nTHIS WILL DELETE ALL MESSAGES ON YOUR SIDE!`,
      { title: "Remove Contact", kind: "warning" },
    );

    if (!confirmed) return;

    try {
      await removeContact(userId);
      notifications.push(`${displayName} removed`, "success");
      onClose();
    } catch (e) {
      notifications.push("Failed to remove contact", "error");
      console.error(e);
    }
  }

  function handleBackdropClick(e: MouseEvent) {
    if (e.target === e.currentTarget) {
      onClose();
    }
  }
</script>

<div
  class="modal-backdrop"
  role="presentation"
  onclick={handleBackdropClick}
  onkeydown={(e) => {
    if (e.key === "Escape") onClose();
  }}
>
  <div class="modal" role="dialog" aria-modal="true" tabindex="-1">
    <button class="close-btn" onclick={onClose}>
      <X size={20} />
    </button>

    {#if contact}
      <div class="profile-content">
        <Avatar name={contact.displayName} src={contact.avatarBase64} size={84} />
        <h2>{contact.displayName}</h2>

        <div class="user-id-card">
          <div class="user-id-row">
            <span class="user-id-label">User ID</span>
            <button class="copy-btn" onclick={copyUserId} title="Copy User ID">
              <Copy size={13} />
            </button>
          </div>
          <code class="user-id-value">{contact.userId}</code>
        </div>
      </div>

      <div class="actions">
        <button class="secondary-btn" onclick={() => (showSecurityModal = true)}>
          <Shield size={16} />
          {contact.verified ? "View Security Code" : "Verify Security Code"}
        </button>
        <button class="danger-btn block-btn" onclick={handleToggleBlock}>
          <Ban size={16} />
          {contact.blocked ? "Unblock Contact" : "Block Contact"}
        </button>
        <button class="danger-btn" onclick={handleRemoveContact}>
          <Trash2 size={16} />
          Remove Contact
        </button>
      </div>
    {/if}
  </div>
</div>

{#if showSecurityModal && contact}
  <SecurityCodeModal
    contactId={contact.userId}
    contactVerified={contact.verified}
    onClose={() => (showSecurityModal = false)}
  />
{/if}

<style>
  .modal-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(2, 6, 23, 0.7);
    backdrop-filter: blur(8px);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
  }

  .modal {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    width: 90%;
    max-width: 320px;
    padding: 32px 24px 24px;
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 32px;
    box-shadow: var(--glow);
  }

  .close-btn {
    position: absolute;
    top: 16px;
    right: 16px;
    color: var(--text-muted);
    transition: color var(--transition);
  }

  .close-btn:hover {
    color: var(--text-primary);
  }

  .profile-content {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
    text-align: center;
  }

  .profile-content h2 {
    font-size: 20px;
    margin: 0;
  }

  .user-id-card {
    width: 100%;
    border: 1px solid var(--border);
    background: var(--bg-tertiary);
    border-radius: var(--radius-md);
    padding: 10px 12px;
    display: flex;
    flex-direction: column;
    gap: 4px;
    text-align: left;
  }

  .user-id-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }

  .user-id-label {
    font-size: 10px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--text-muted);
  }

  .copy-btn {
    width: 24px;
    height: 24px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    color: var(--text-muted);
    border-radius: 6px;
    transition: all var(--transition);
  }

  .copy-btn:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .user-id-value {
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 11px;
    color: var(--text-secondary);
    word-break: break-all;
    line-height: 1.4;
  }

  .secondary-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    width: 100%;
    padding: 12px;
    border-radius: var(--radius-md);
    background: var(--bg-tertiary);
    border: 1px solid var(--border);
    color: var(--text-primary);
    font-weight: 600;
    transition: background var(--transition);
  }

  .secondary-btn:hover {
    background: var(--bg-hover);
  }

  .actions {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .danger-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    width: 100%;
    padding: 12px;
    border-radius: var(--radius-md);
    background: rgba(251, 113, 133, 0.1);
    color: var(--danger);
    font-weight: 600;
    transition: background var(--transition);
  }

  .danger-btn:hover {
    background: rgba(251, 113, 133, 0.2);
  }
</style>
