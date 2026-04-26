<script lang="ts">
  import { onMount } from "svelte";
  import {
    Upload,
    Download,
    FolderOpen,
    Trash2,
    Save,
    Bell,
    BellRing,
    Copy,
  } from "lucide-svelte";
  import {
    save as saveDialog,
    open as openDialog,
  } from "@tauri-apps/plugin-dialog";
  import { writeFile, readFile } from "@tauri-apps/plugin-fs";
  import { broadcastProfile } from "$lib/api/identity";
  import { exportBackup, importBackup } from "$lib/api/backup";
  import {
    ensurePermission,
    getPermission,
    sendTestNotification,
  } from "$lib/api/system-notify";
  import { profileState } from "$lib/state/profile.svelte";
  import {
    prefsState,
    type NotificationPreview,
    type DndSchedule,
  } from "$lib/state/prefs.svelte";
  import { notifications } from "$lib/state/notifications.svelte";
  import { confirmDialog } from "$lib/state/confirm.svelte";
  import Avatar from "$lib/components/Avatar.svelte";
  import AvatarPicker from "$lib/components/AvatarPicker.svelte";
  import Button from "$lib/components/Button.svelte";
  import SettingCard from "./SettingCard.svelte";
  import SettingRow from "./SettingRow.svelte";
  import Toggle from "./Toggle.svelte";
  import Segmented from "./Segmented.svelte";
  import TextInput from "./TextInput.svelte";
  import DndDial from "./DndDial.svelte";
  import { writeText } from "@tauri-apps/plugin-clipboard-manager";

  let displayName = $state("You");
  let avatarBase64 = $state<string | null>(null);
  let avatarBytes = $state<number[] | null>(null);
  let savingProfile = $state(false);
  let exporting = $state(false);
  let importing = $state(false);

  let exportOpen = $state(false);
  let exportPwd = $state("");
  let exportPwd2 = $state("");

  let importOpen = $state(false);
  let importPwd = $state("");
  let pendingImportBytes = $state<Uint8Array | null>(null);

  let preview = $state<NotificationPreview>("content");
  let dnd = $state<DndSchedule>({
    enabled: false,
    start: "22:00",
    end: "06:00",
  });

  let notifPermission = $state<boolean>(false);
  let requestingPerm = $state(false);
  let sendingTest = $state(false);

  const profileDirty = $derived(
    displayName.trim() !== profileState.displayName.trim() ||
      avatarBase64 !== profileState.avatarBase64,
  );

  onMount(async () => {
    await profileState.load();
    displayName = profileState.displayName;
    avatarBase64 = profileState.avatarBase64;
    avatarBytes = profileState.avatarBytes;
    preview = prefsState.notificationPreview;
    dnd = { ...prefsState.dnd };
    notifPermission = await getPermission(true);
  });

  function handleAvatarChange(b64: string, bytes: number[]) {
    avatarBase64 = b64;
    avatarBytes = bytes;
  }

  async function saveProfile() {
    const nameToSave = displayName.trim() || "You";
    savingProfile = true;
    try {
      await broadcastProfile(nameToSave, avatarBytes);
      profileState.update(nameToSave, avatarBase64, avatarBytes);
      notifications.push("Profile saved", "success");
    } catch (e) {
      console.error(e);
      notifications.push("Saved locally but broadcast failed", "error");
    } finally {
      savingProfile = false;
    }
  }

  function resetProfile() {
    displayName = profileState.displayName;
    avatarBase64 = profileState.avatarBase64;
    avatarBytes = profileState.avatarBytes;
  }

  function resetExport() {
    exportOpen = false;
    exportPwd = "";
    exportPwd2 = "";
  }

  function resetImport() {
    importOpen = false;
    importPwd = "";
    pendingImportBytes = null;
  }

  async function handleExportBackup() {
    if (exportPwd.length < 8) {
      notifications.push("Password must be 8+ characters", "error");
      return;
    }
    if (exportPwd !== exportPwd2) {
      notifications.push("Passwords don't match", "error");
      return;
    }
    exporting = true;
    try {
      const bytes = await exportBackup(exportPwd);
      const path = await saveDialog({
        defaultPath: "backup.kursal",
        filters: [{ name: "Kursal Backup", extensions: ["kursal"] }],
      });
      if (!path) return;
      await writeFile(path, new Uint8Array(bytes));
      notifications.push("Backup saved", "success");
      resetExport();
    } catch (e) {
      console.error(e);
      notifications.push(`Export failed: ${e}`, "error");
    } finally {
      exporting = false;
    }
  }

  async function pickImportFile() {
    try {
      const picked = await openDialog({
        multiple: false,
        filters: [{ name: "Kursal Backup", extensions: ["kursal"] }],
      });
      if (!picked || Array.isArray(picked)) return;
      pendingImportBytes = await readFile(picked as string);
      importOpen = true;
    } catch (e) {
      notifications.push(`Read failed: ${e}`, "error");
    }
  }

  async function handleImportBackup() {
    if (!pendingImportBytes) return;
    if (importPwd.length === 0) {
      notifications.push("Enter the backup password", "error");
      return;
    }
    const confirmed = await confirmDialog({
      title: "Replace current app data?",
      message:
        "This will overwrite your current identity, contacts, and messages with the backup. This cannot be undone.",
      confirmLabel: "Replace everything",
      cancelLabel: "Cancel",
      tone: "danger",
      holdMs: 5000,
    });
    if (!confirmed) return;
    importing = true;
    try {
      await importBackup(Array.from(pendingImportBytes), importPwd);
      notifications.push(`Imported backup sucessfully`, "success");
      resetImport();
    } catch (e) {
      console.error(e);
      notifications.push(`Import failed: ${e}`, "error");
    } finally {
      importing = false;
    }
  }

  function updatePreview(value: NotificationPreview) {
    preview = value;
    prefsState.setPreview(value);
  }

  function updateDnd(changes: Partial<DndSchedule>) {
    dnd = { ...dnd, ...changes };
    prefsState.setDnd(dnd);
  }

  async function handleEnableNotifications() {
    requestingPerm = true;
    try {
      const ok = await ensurePermission();
      notifPermission = ok;
      if (!ok) {
        notifications.push(
          "Permission denied. Enable in system settings.",
          "error",
        );
      }
    } finally {
      requestingPerm = false;
    }
  }

  async function handleTestNotification() {
    sendingTest = true;
    try {
      const sent = await sendTestNotification();
      notifPermission = await getPermission(true);
      if (!sent) {
        notifications.push(
          "Permission denied. Enable in system settings.",
          "error",
        );
      }
    } finally {
      sendingTest = false;
    }
  }

  async function copyUserId() {
    if (!profileState.userId) return;
    try {
      await writeText(profileState.userId);
      notifications.push("Peer ID copied", "success");
    } catch (e) {
      notifications.push(`Copy failed: ${e}`, "error");
    }
  }
</script>

<div class="sec-head">
  <h2>Account</h2>
  <p>Manage your identity, backups, and notifications.</p>
</div>

<SettingCard title="Profile">
  <div class="profile">
    <AvatarPicker onChange={handleAvatarChange}>
      {#snippet children(open)}
        <div class="avatar-wrap">
          <Avatar name={displayName || "You"} src={avatarBase64} size={72} />
          <button
            type="button"
            class="avatar-edit"
            aria-label="Change photo"
            onclick={open}
          >
            <Upload size={13} />
          </button>
        </div>
      {/snippet}
    </AvatarPicker>

    <div class="profile-fields">
      <div class="field">
        <span class="field-label">Display name</span>
        <TextInput
          bind:value={displayName}
          placeholder="Your name"
          width="260px"
        />
      </div>
      {#if avatarBase64}
        <button
          type="button"
          class="remove-avatar"
          onclick={() => {
            avatarBase64 = null;
            avatarBytes = null;
          }}
        >
          <Trash2 size={12} /> Remove photo
        </button>
      {/if}
    </div>
  </div>

  {#snippet footer()}
    {#if profileDirty}
      <Button variant="secondary" onclick={resetProfile}>Cancel</Button>
    {/if}
    <Button
      onclick={saveProfile}
      loading={savingProfile}
      disabled={!profileDirty}
    >
      <Save size={13} /> Save
    </Button>
  {/snippet}

  {#if profileState.userId}
    <div class="user-id-block">
      <div class="user-id-head">
        <span class="user-id-label">User ID</span>
        <div class="user-id-actions">
          <button
            class="icon-btn"
            onclick={copyUserId}
            aria-label="Copy user ID"
          >
            <Copy size={13} />
          </button>
        </div>
      </div>
      <code class="user-id mono selectable">{profileState.userId}</code>
    </div>
  {/if}
</SettingCard>

<SettingCard
  title="Backup"
  description="Encrypted snapshot of your identity, contacts, messages, and settings."
>
  <SettingRow
    title="Export backup"
    description="Saves a password-protected .kursal file."
  >
    {#if exportOpen}
      <Button variant="secondary" onclick={resetExport}>Cancel</Button>
    {:else}
      <Button variant="secondary" onclick={() => (exportOpen = true)}>
        <Download size={13} /> Export
      </Button>
    {/if}
  </SettingRow>
  {#if exportOpen}
    <div class="pwd-block">
      <div class="field">
        <span class="field-label">Password</span>
        <TextInput
          type="password"
          bind:value={exportPwd}
          placeholder="At least 8 characters"
          width="260px"
        />
      </div>
      <div class="field">
        <span class="field-label">Confirm</span>
        <TextInput
          type="password"
          bind:value={exportPwd2}
          placeholder="Repeat password"
          width="260px"
        />
      </div>
      <p class="pwd-warn">
        If you forget this password, the backup is unrecoverable.
      </p>
      <div class="pwd-actions">
        <Button onclick={handleExportBackup} loading={exporting}>
          <Save size={13} /> Encrypt &amp; save
        </Button>
      </div>
    </div>
  {/if}

  <SettingRow
    title="Import backup"
    description="Restore from a previous .kursal file. Replaces current identity."
  >
    {#if importOpen}
      <Button variant="secondary" onclick={resetImport}>Cancel</Button>
    {:else}
      <Button variant="secondary" onclick={pickImportFile}>
        <FolderOpen size={13} /> Choose file
      </Button>
    {/if}
  </SettingRow>
  {#if importOpen}
    <div class="pwd-block">
      <div class="field">
        <span class="field-label">Password</span>
        <TextInput
          type="password"
          bind:value={importPwd}
          placeholder="Backup password"
          width="260px"
        />
      </div>
      <div class="pwd-actions">
        <Button onclick={handleImportBackup} loading={importing}>
          <Upload size={13} /> Decrypt &amp; restore
        </Button>
      </div>
    </div>
  {/if}
</SettingCard>

<SettingCard title="Notifications">
  <SettingRow
    title="System permission"
    description={notifPermission
      ? "Kursal can show desktop notifications."
      : "Kursal is not allowed to show desktop notifications."}
  >
    <div class="notif-perm">
      <span class="perm-dot" data-on={notifPermission}></span>
      {#if notifPermission}
        <Button
          variant="secondary"
          onclick={handleTestNotification}
          loading={sendingTest}
        >
          <BellRing size={13} /> Send test
        </Button>
      {:else}
        <Button onclick={handleEnableNotifications} loading={requestingPerm}>
          <Bell size={13} /> Enable
        </Button>
      {/if}
    </div>
  </SettingRow>
  <SettingRow
    title="Message preview"
    description="What system notifications show when a message arrives."
  >
    <Segmented
      value={preview}
      options={[
        { value: "content", label: "Full" },
        { value: "sender", label: "Sender" },
        { value: "generic", label: "Generic" },
        { value: "none", label: "Off" },
      ]}
      onchange={updatePreview}
      size="sm"
    />
  </SettingRow>
  <SettingRow
    title="Do not disturb"
    description="Mute notifications during a scheduled window."
  >
    <Toggle
      checked={dnd.enabled}
      ariaLabel="Enable do not disturb"
      onchange={(v) => updateDnd({ enabled: v })}
    />
  </SettingRow>
  {#if dnd.enabled}
    <div class="schedule-block">
      <DndDial
        start={dnd.start}
        end={dnd.end}
        onchange={(s, e) => updateDnd({ start: s, end: e })}
      />
    </div>
  {/if}
</SettingCard>

<style>
  .user-id-block {
    padding: 14px;
    border-bottom: 1px solid var(--border-light);
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .user-id-head {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 8px;
  }
  .user-id-label {
    font-size: 12px;
    font-weight: 600;
    color: var(--text-primary);
  }
  .user-id-actions {
    display: flex;
    gap: 6px;
    align-items: center;
  }
  .user-id {
    font-family: ui-monospace, SFMono-Regular, monospace;
    font-size: 11px;
    color: var(--text-secondary);
    background: var(--bg-input);
    padding: 8px 10px;
    border-radius: var(--radius-sm);
    word-break: break-all;
    line-height: 1.5;
  }
  .profile {
    display: flex;
    gap: 20px;
    padding: 16px;
    align-items: flex-start;
  }
  .avatar-wrap {
    position: relative;
    flex-shrink: 0;
  }
  .avatar-edit {
    position: absolute;
    right: -2px;
    bottom: -2px;
    width: 26px;
    height: 26px;
    border-radius: 50%;
    background: var(--accent);
    color: #fff;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border: 2px solid var(--bg-tertiary);
    cursor: pointer;
    transition: background var(--transition);
  }
  .avatar-edit:hover {
    background: var(--accent-hover);
  }
  .profile-fields {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 10px;
    min-width: 0;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 5px;
  }
  .field-label {
    font-size: 12px;
    font-weight: 600;
    color: var(--text-secondary);
  }
  .remove-avatar {
    align-self: flex-start;
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 12px;
    font-weight: 500;
    color: var(--danger);
    border: none;
    background: none;
    padding: 2px 0;
    cursor: pointer;
  }
  .remove-avatar:hover {
    opacity: 0.8;
  }

  .schedule-block {
    padding: 14px 14px 18px;
    border-top: 1px solid var(--border-light);
  }
  .pwd-block {
    padding: 14px;
    border-top: 1px solid var(--border-light);
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .pwd-warn {
    font-size: 12px;
    color: var(--text-muted);
    margin: 2px 0 0;
  }
  .pwd-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
  .notif-perm {
    display: inline-flex;
    align-items: center;
    gap: 10px;
  }
  .perm-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--danger);
    box-shadow: 0 0 0 2px rgba(239, 68, 68, 0.18);
  }
  .perm-dot[data-on="true"] {
    background: var(--success, #22c55e);
    box-shadow: 0 0 0 2px rgba(34, 197, 94, 0.22);
  }
</style>
