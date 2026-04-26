<script lang="ts">
  import { UserPlus } from "lucide-svelte";
  import { goto } from "$app/navigation";
  import { contactsState } from "$lib/state/contacts.svelte";

  const hasContacts = $derived(contactsState.contacts.length > 0);
</script>

<div class="empty" data-tauri-drag-region>
  <div class="empty-inner">
    <div class="icon-wrap">
      <img src="/winston.png" alt="Kursal Mascott" width="88" height="88" />
    </div>
    {#if hasContacts}
      <h2>Select a conversation</h2>
      <p>
        Pick a contact from the sidebar to start chatting. All your messages are
        end-to-end encrypted.
      </p>
    {:else}
      <h2>Welcome to Kursal</h2>
      <p>Add your first contact to start sending encrypted messages.</p>
      <button class="add-btn" onclick={() => goto("/add-contact")}>
        <UserPlus size={15} /> Add a contact
      </button>
    {/if}
  </div>
</div>

<style>
  .empty {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 24px;
    background: transparent;
  }

  .empty-inner {
    text-align: center;
    max-width: 360px;
    color: var(--text-muted);
  }

  .icon-wrap {
    width: 64px;
    height: 64px;
    display: flex;
    align-items: center;
    justify-content: center;
    margin: 0 auto 18px;
  }

  h2 {
    font-size: 20px;
    font-weight: 600;
    color: var(--text-primary);
    margin: 0 0 8px;
    letter-spacing: -0.01em;
  }

  p {
    margin: 0 0 14px;
    font-size: 13.5px;
    line-height: 1.55;
    color: var(--text-secondary);
  }

  .add-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    margin-top: 4px;
    padding: 10px 18px;
    background: var(--accent);
    color: #fff;
    border-radius: var(--radius-md);
    font-size: 13.5px;
    font-weight: 600;
    transition: all var(--transition);
  }
  .add-btn:hover {
    background: var(--accent-hover);
    transform: translateY(-1px);
  }
  .add-btn:active {
    transform: translateY(0);
  }
</style>
