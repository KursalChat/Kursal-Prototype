<script lang="ts">
  import { onMount } from "svelte";
  import {
    RefreshCw,
    ShieldOff,
    Trash2,
    Copy,
    TriangleAlert,
  } from "lucide-svelte";
  import { writeText } from "@tauri-apps/plugin-clipboard-manager";
  import { rotatePeerId } from "$lib/api/identity";
  import {
    listBlockedContacts,
    clearMessageHistory,
    deleteAllLocalData,
    type PeerRotationInterval,
  } from "$lib/api/settings";
  import { settingsState } from "$lib/state/settings.svelte";
  import { confirmDialog } from "$lib/state/confirm.svelte";
  import type { ContactResponse } from "$lib/types";
  import { profileState } from "$lib/state/profile.svelte";
  import { contactsState } from "$lib/state/contacts.svelte";
  import { notifications } from "$lib/state/notifications.svelte";
  import { messagesState } from "$lib/state/messages.svelte";
  import Avatar from "$lib/components/Avatar.svelte";
  import Button from "$lib/components/Button.svelte";
  import SettingCard from "./SettingCard.svelte";
  import SettingRow from "./SettingRow.svelte";
  import Toggle from "./Toggle.svelte";
  import Segmented from "./Segmented.svelte";
  import Select from "./Select.svelte";
  import { setContactBlocked } from "$lib/api/contacts";

  let rotating = $state(false);
  const rotationInterval = $derived(settingsState.peerRotation);
  const typing = $derived(settingsState.typingIndicators);

  let blocked = $state<ContactResponse[]>([]);
  let blockedLoading = $state(false);

  let clearTarget = $state<string>("");
  let clearing = $state(false);
  let deleting = $state(false);

  onMount(async () => {
    void settingsState.load();
    await reloadBlocked();
  });

  async function reloadBlocked() {
    blockedLoading = true;
    try {
      blocked = await listBlockedContacts();
    } catch (e) {
      console.error(e);
    } finally {
      blockedLoading = false;
    }
  }

  async function handleRotate() {
    const ok = await confirmDialog({
      title: "Rotate peer ID?",
      message:
        "A new transport identity will be generated and broadcast to your contacts.",
      detail:
        "Only do this if you suspect your current peer ID is compromised, or you want to reset how the network finds you. Existing conversations are preserved.",
      confirmLabel: "Rotate now",
      tone: "warning",
    });
    if (!ok) return;
    rotating = true;
    try {
      await rotatePeerId();
      await profileState.refreshPeerId();
      notifications.push("Peer ID rotated", "success");
    } catch (e) {
      notifications.push("Failed to rotate peer ID", "error");
    } finally {
      rotating = false;
    }
  }

  async function handleIntervalChange(value: PeerRotationInterval) {
    try {
      await settingsState.setPeerRotation(value);
      notifications.push("Saved. Will take effect after restart.", "info");
    } catch (e) {
      notifications.push(`Failed: ${e}`, "error");
    }
  }

  async function copyPeerId() {
    if (!profileState.peerId) return;
    try {
      await writeText(profileState.peerId);
      notifications.push("Peer ID copied", "success");
    } catch (e) {
      notifications.push(`Copy failed: ${e}`, "error");
    }
  }

  async function handleTypingChange(value: boolean) {
    try {
      await settingsState.setTyping(value);
    } catch (e) {
      notifications.push(`Failed: ${e}`, "error");
    }
  }

  async function handleUnblock(id: string) {
    try {
      await setContactBlocked(id, false);
      blocked = blocked.filter((c) => c.userId !== id);
      notifications.push("Contact unblocked", "success");
    } catch (e) {
      notifications.push(`Failed: ${e}`, "error");
    }
  }

  async function handleClearHistory() {
    if (!clearTarget) return;
    const target =
      clearTarget === "*"
        ? "all contacts"
        : (contactsState.getById(clearTarget)?.displayName ?? "this contact");
    const ok = await confirmDialog({
      title: "Clear message history?",
      message: `All messages with ${target} will be deleted from this device.`,
      detail:
        "This cannot be undone. Copies on the other side are not affected.",
      confirmLabel: "Clear history",
      tone: "danger",
      holdMs: 5000,
    });
    if (!ok) return;
    clearing = true;
    try {
      await clearMessageHistory(clearTarget === "*" ? null : clearTarget);
      if (clearTarget === "*") {
        messagesState.clearAll();
      } else {
        messagesState.clearForContact(clearTarget);
      }
      notifications.push("Message history cleared", "success");
    } catch (e) {
      notifications.push(`Failed: ${e}`, "error");
    } finally {
      clearing = false;
    }
  }

  async function handleDeleteAll() {
    const first = await confirmDialog({
      title: "Delete all local data?",
      message:
        "This wipes all contacts, messages, identity keys, and cached backups.",
      detail:
        "Your identity is gone unless you have exported a backup. This cannot be undone.",
      confirmLabel: "Continue",
      tone: "danger",
    });
    if (!first) return;
    const second = await confirmDialog({
      title: "Are you absolutely sure?",
      message:
        "Once deleted, nothing can be recovered. Export a backup first if anything should be kept. This will close the app to reset it. Please open it again.",
      confirmLabel: "Delete everything",
      cancelLabel: "Go back",
      tone: "danger",
      holdMs: 5000,
    });
    if (!second) return;
    deleting = true;
    try {
      await deleteAllLocalData();
    } catch (e) {
      notifications.push(`Failed: ${e}`, "error");
      deleting = false;
    }
  }

  const intervals: { value: PeerRotationInterval; label: string }[] = [
    { value: "6h", label: "6h" },
    { value: "12h", label: "12h" },
    { value: "30h", label: "30h" },
    { value: "7d", label: "7d" },
    { value: "manual", label: "Manual" },
  ];

  const clearOptions = $derived([
    { value: "*" as const, label: "All contacts" },
    ...contactsState.contacts.map((c) => ({
      value: c.userId,
      label: c.displayName,
    })),
  ]);
</script>

<div class="sec-head">
  <h2>Privacy &amp; Security</h2>
  <p>Your identity and data controls.</p>
</div>

<SettingCard title="Identity">
  {#if profileState.peerId}
    <div class="peer-id-block">
      <div class="peer-id-head">
        <span class="peer-id-label">Current Peer ID</span>
        <div class="peer-id-actions">
          <button
            class="icon-btn"
            onclick={copyPeerId}
            aria-label="Copy peer ID"
          >
            <Copy size={13} />
          </button>
          <Button variant="secondary" loading={rotating} onclick={handleRotate}>
            <RefreshCw size={13} /> Rotate
          </Button>
        </div>
      </div>
      <code class="peer-id mono selectable">{profileState.peerId}</code>
    </div>
  {/if}
  <SettingRow
    title="Auto-rotation"
    description="How often your transport identity changes. Applied on next app restart."
  >
    <Segmented
      value={rotationInterval}
      options={intervals}
      onchange={handleIntervalChange}
      size="sm"
    />
  </SettingRow>
</SettingCard>

<!--
  App lock (password / biometric) — feature not implemented yet.
  Re-enable this block when the backend commands exist:
    - get_app_lock_config / set_app_lock / verify_app_lock
  Restore imports: getAppLockConfig, setAppLock, AppLockMethod.
-->

<SettingCard title="Messaging">
  <SettingRow
    title="Send typing indicators"
    description="Contacts see when you are typing. Disable for more privacy."
  >
    <Toggle
      checked={typing}
      onchange={handleTypingChange}
      ariaLabel="Send typing indicators"
    />
  </SettingRow>
</SettingCard>

<SettingCard
  title="Blocked contacts"
  description="People you block cannot connect to you or send you messages."
>
  {#if blockedLoading}
    <div class="empty">Loading…</div>
  {:else if blocked.length === 0}
    <div class="empty">No blocked contacts.</div>
  {:else}
    {#each blocked as c}
      <SettingRow
        title={c.displayName}
        description={c.userId.slice(0, 24) + "…"}
      >
        <Avatar name={c.displayName} src={c.avatarBase64} size={28} />
        <Button variant="secondary" onclick={() => handleUnblock(c.userId)}>
          <ShieldOff size={13} /> Unblock
        </Button>
      </SettingRow>
    {/each}
  {/if}
</SettingCard>

<SettingCard
  title="Clear history"
  description="Delete chat history for a specific contact or all of them."
>
  <SettingRow
    title="Target"
    description="Messages are permanently removed from this device."
  >
    <Select
      value={clearTarget as any}
      options={clearOptions}
      onchange={(v) => (clearTarget = v as string)}
      placeholder="Choose contact…"
      minWidth="220px"
    />
    <Button
      variant="danger"
      disabled={!clearTarget}
      loading={clearing}
      onclick={handleClearHistory}
    >
      <Trash2 size={13} /> Clear
    </Button>
  </SettingRow>
</SettingCard>

<SettingCard title="Danger zone" tone="danger">
  <div class="danger-body">
    <div class="danger-head">
      <TriangleAlert size={16} />
      <div>
        <div class="danger-title">Delete all local data</div>
        <div class="danger-desc">
          Wipes contacts, messages, keys, and everything from this device. Your
          identity is lost unless you have an exported backup.
        </div>
      </div>
    </div>
    <Button variant="danger" loading={deleting} onclick={handleDeleteAll}>
      <Trash2 size={13} /> Delete everything
    </Button>
  </div>
</SettingCard>

<style>
  .peer-id-block {
    padding: 14px;
    border-bottom: 1px solid var(--border-light);
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .peer-id-head {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 8px;
  }
  .peer-id-label {
    font-size: 12px;
    font-weight: 600;
    color: var(--text-primary);
  }
  .peer-id-actions {
    display: flex;
    gap: 6px;
    align-items: center;
  }
  .peer-id {
    font-family: ui-monospace, SFMono-Regular, monospace;
    font-size: 11px;
    color: var(--text-secondary);
    background: var(--bg-input);
    padding: 8px 10px;
    border-radius: var(--radius-sm);
    word-break: break-all;
    line-height: 1.5;
  }
  .icon-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    padding: 7px;
    border-radius: var(--radius-md);
    background: var(--bg-input);
    border: 1px solid var(--border);
    color: var(--text-secondary);
    cursor: pointer;
    transition:
      background var(--transition),
      color var(--transition);
  }
  .icon-btn:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .empty {
    padding: 20px 14px;
    text-align: center;
    font-size: 13px;
    color: var(--text-muted);
  }

  .danger-body {
    padding: 14px;
    display: flex;
    flex-direction: column;
    gap: 12px;
    align-items: flex-start;
  }
  .danger-head {
    display: flex;
    gap: 10px;
    color: #fda4af;
    align-items: flex-start;
  }
  .danger-head > :global(svg) {
    flex-shrink: 0;
    margin-top: 2px;
  }
  .danger-title {
    font-size: 14px;
    font-weight: 700;
    margin-bottom: 2px;
  }
  .danger-desc {
    font-size: 12px;
    color: var(--text-secondary);
    line-height: 1.5;
  }
</style>
