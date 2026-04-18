<script lang="ts">
  import { browser } from "$app/environment";
  import { onMount } from "svelte";
  import { Shield, User, Zap, Wifi, Upload, Activity } from "lucide-svelte";
  import { rotatePeerId, broadcastProfile } from "$lib/api/identity";
  import { profileState } from "$lib/state/profile.svelte";
  import { contactsState } from "$lib/state/contacts.svelte";
  import { notifications } from "$lib/state/notifications.svelte";
  import Button from "$lib/components/Button.svelte";
  import Avatar from "$lib/components/Avatar.svelte";
  import Benchmark from "$lib/components/Benchmark.svelte";
  import { confirm } from "@tauri-apps/plugin-dialog";
  import { getVersion } from "@tauri-apps/api/app";
  import { invoke } from "@tauri-apps/api/core";

  type SettingsCategory =
    | "account"
    | "security"
    | "network"
    | "benchmarks"
    | "advanced";

  let activeCategory = $state<SettingsCategory>("account");

  const categories = [
    { id: "account" as const, label: "Account", icon: User },
    { id: "security" as const, label: "Security", icon: Shield },
    { id: "network" as const, label: "Network", icon: Wifi },
    { id: "benchmarks" as const, label: "Benchmarks", icon: Activity },
    { id: "advanced" as const, label: "Advanced", icon: Zap },
  ];

  let displayName = $state("You");
  let avatarBase64 = $state<string | null>(null);
  let avatarBytes = $state<number[] | null>(null);
  let rotating = $state(false);
  let savingProfile = $state(false);
  let appVersion = $state("...");
  let checkingForUpdates = $state(false);

  onMount(async () => {
    if (!browser) return;
    await profileState.load();
    displayName = profileState.displayName;
    avatarBase64 = profileState.avatarBase64;
    avatarBytes = profileState.avatarBytes;
    try {
      appVersion = `v${await getVersion()}`;
    } catch {
      appVersion = "Unknown";
    }
  });

  async function handleAvatarSelection(event: Event) {
    const input = event.target as HTMLInputElement;
    if (!input.files || input.files.length === 0) return;
    const file = input.files[0];
    if (!file.type.startsWith("image/")) {
      notifications.push("Not an image", "error");
      return;
    }
    try {
      const bitmap = await createImageBitmap(file);
      const canvas = document.createElement("canvas");
      const MAX_SIZE = 256;
      let width = bitmap.width,
        height = bitmap.height;
      if (width > height) {
        if (width > MAX_SIZE) {
          height *= MAX_SIZE / width;
          width = MAX_SIZE;
        }
      } else {
        if (height > MAX_SIZE) {
          width *= MAX_SIZE / height;
          height = MAX_SIZE;
        }
      }
      canvas.width = width;
      canvas.height = height;
      const ctx = canvas.getContext("2d");
      if (!ctx) throw new Error("No 2D context");
      ctx.drawImage(bitmap, 0, 0, width, height);
      const dataUrl = canvas.toDataURL("image/webp", 0.8);
      if (dataUrl.length > 55000) {
        notifications.push("Image too large, try a smaller one", "error");
        return;
      }
      const base64Data = dataUrl.split(",")[1];
      avatarBase64 = base64Data;
      avatarBytes = Array.from(
        Uint8Array.from(atob(base64Data), (c) => c.charCodeAt(0)),
      );
    } catch (e) {
      console.error("Avatar compression failed", e);
      notifications.push("Failed to process image", "error");
    }
  }

  async function saveProfile() {
    if (!browser) return;
    const nameToSave = displayName.trim() || "You";
    savingProfile = true;
    try {
      await broadcastProfile(nameToSave, avatarBytes);
      profileState.update(nameToSave, avatarBase64, avatarBytes);
      notifications.push("Profile saved", "success");
    } catch (e) {
      console.error("Broadcast failed", e);
      notifications.push("Saved locally but broadcast failed", "error");
    } finally {
      savingProfile = false;
    }
  }

  async function handleRotate() {
    const ok = await confirm(
      "Rotate your peer ID? Contacts will be notified.",
      { title: "Rotate Peer ID", kind: "warning" },
    );
    if (!ok) return;
    rotating = true;
    try {
      await rotatePeerId();
      await profileState.refreshPeerId();
      notifications.push("Peer ID rotated", "success");
    } catch (e) {
      notifications.push("Failed to rotate peer ID", "error");
      console.error("Rotate failed:", e);
    } finally {
      rotating = false;
    }
  }

  async function handleCheckForUpdates() {
    checkingForUpdates = true;
    try {
      await invoke("check_for_updates");
    } catch (e) {
      notifications.push(`Update check failed: ${e}`, "error");
    } finally {
      checkingForUpdates = false;
    }
  }
</script>

<div class="settings">
  <header class="settings-header">
    <h2>Settings</h2>
  </header>

  <nav class="tabs">
    {#each categories as cat}
      <button
        class="tab"
        data-active={activeCategory === cat.id}
        onclick={() => (activeCategory = cat.id)}
      >
        <cat.icon size={15} />
        <span class="tab-text">{cat.label}</span>
      </button>
    {/each}
  </nav>

  <section class="settings-body">
    {#if activeCategory === "account"}
      <div class="section">
        <h3>Account</h3>
        <p class="section-desc">Manage your local profile.</p>

        <div class="avatar-row">
          <Avatar name={displayName || "You"} src={avatarBase64} size={64} />
          <div class="avatar-actions">
            <label class="file-btn">
              <Upload size={14} />
              Change Photo
              <input
                type="file"
                accept="image/*"
                onchange={handleAvatarSelection}
                style="display:none"
              />
            </label>
            {#if avatarBase64}
              <button
                class="text-btn danger"
                onclick={() => {
                  avatarBase64 = null;
                  avatarBytes = null;
                }}>Remove</button
              >
            {/if}
          </div>
        </div>

        <div class="field">
          <label for="displayName">Display Name</label>
          <input
            id="displayName"
            bind:value={displayName}
            placeholder="Your name"
          />
          <span class="hint">Visible to your contacts.</span>
        </div>

        <Button onclick={saveProfile} loading={savingProfile}
          >Save Profile</Button
        >

        {#if profileState.peerId}
          <div class="card">
            <span class="card-label">Peer ID</span>
            <code class="mono">{profileState.peerId}</code>
          </div>
        {/if}
      </div>
    {:else if activeCategory === "security"}
      <div class="section">
        <h3>Security</h3>
        <p class="section-desc">Manage your cryptographic identity.</p>

        <div class="card">
          <span class="card-label">Encryption</span>
          <p>
            PQXDH + Double Ratchet with Kyber1024 and Dilithium-5 signatures.
            All messages are end-to-end encrypted.
          </p>
        </div>

        <div class="danger-zone">
          <h4>Rotate Peer ID</h4>
          <p>
            Generates a new transport identity. Use if your network identity may
            be compromised.
          </p>
          <Button variant="danger" loading={rotating} onclick={handleRotate}
            >Rotate Peer ID</Button
          >
        </div>
      </div>
    {:else if activeCategory === "network"}
      <div class="section">
        <h3>Network</h3>
        <p class="section-desc">Connection settings.</p>
        <div class="card">
          <span class="card-label">Connection Methods</span>
          <p>
            Kursal attempts direct P2P connections first, falling back to relays
            when needed. All traffic remains E2E encrypted.
          </p>
        </div>
      </div>
    {:else if activeCategory === "benchmarks"}
      <div class="section">
        <h3>Benchmarks</h3>
        <p class="section-desc">
          Test your device's cryptographic performance.
        </p>
        <Benchmark
          name="OTP Hashing"
          description="Measures Argon2id OTP hashing speed."
        />
      </div>
    {:else if activeCategory === "advanced"}
      <div class="section">
        <h3>Advanced</h3>
        <p class="section-desc">App info and diagnostics.</p>

        <div class="info-list">
          <div class="info-row">
            <span>Version</span><span>Kursal {appVersion}</span>
          </div>
          <div class="info-row">
            <span>Contacts</span><span>{contactsState.contacts.length}</span>
          </div>
          <div class="info-row"><span>License</span><span>AGPL-3.0</span></div>
        </div>

        <div class="card">
          <span class="card-label">Updates</span>
          <p>Check for newer versions of Kursal.</p>
          <Button onclick={handleCheckForUpdates} loading={checkingForUpdates}
            >Check for Updates</Button
          >
        </div>

        <div class="card">
          <span class="card-label">Logs</span>
          <p>Open application log directory for troubleshooting.</p>
          <Button onclick={async () => await invoke("open_log_folder")}
            >Open Log Folder</Button
          >
        </div>
      </div>
    {/if}
  </section>
</div>

<style>
  .settings {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }

  .settings-header {
    height: var(--header-height);
    padding: 0 16px;
    display: flex;
    align-items: center;
    border-bottom: 1px solid var(--border);
    background: var(--bg-secondary);
    flex-shrink: 0;
  }

  .settings-header h2 {
    margin: 0;
    font-size: 18px;
    font-weight: 700;
  }

  .tabs {
    display: flex;
    gap: 4px;
    padding: 8px 16px;
    border-bottom: 1px solid var(--border);
    background: var(--bg-secondary);
    flex-shrink: 0;
    overflow-x: auto;
  }

  .tab {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 7px 12px;
    border-radius: var(--radius-md);
    font-size: 13px;
    font-weight: 600;
    color: var(--text-secondary);
    transition: all var(--transition);
    white-space: nowrap;
    border: 1px solid transparent;
  }

  .tab:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }
  .tab[data-active="true"],
  .tab[data-active="true"]:hover {
    background: var(--accent-selected);
    color: #fff;
    border-color: rgba(165, 180, 252, 0.7);
  }

  .settings-body {
    flex: 1;
    overflow-y: auto;
    padding: 20px 16px;
  }

  .section {
    max-width: 600px;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  .section h3 {
    margin: 0;
    font-size: 20px;
    font-weight: 700;
  }
  .section-desc {
    margin: -8px 0 4px;
    font-size: 13px;
    color: var(--text-muted);
  }

  .avatar-row {
    display: flex;
    align-items: center;
    gap: 16px;
  }

  .avatar-actions {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .file-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 6px 12px;
    border-radius: var(--radius-md);
    background: var(--bg-tertiary);
    border: 1px solid var(--border);
    color: var(--text-primary);
    font-size: 13px;
    font-weight: 500;
    cursor: pointer;
    transition: background var(--transition);
  }

  .file-btn:hover {
    background: var(--bg-hover);
  }

  .text-btn {
    border: none;
    background: none;
    padding: 0;
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
  }

  .text-btn.danger {
    color: var(--danger);
  }
  .text-btn.danger:hover {
    opacity: 0.8;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 5px;
  }

  .field label {
    font-size: 12px;
    font-weight: 600;
    color: var(--text-secondary);
  }

  .field input {
    border-radius: var(--radius-md);
    border: 1px solid var(--border);
    background: var(--bg-input);
    color: var(--text-primary);
    padding: 8px 12px;
    font-size: 14px;
    outline: none;
    transition: border-color var(--transition);
  }

  .field input:focus {
    border-color: rgba(129, 140, 248, 0.5);
  }

  .hint {
    font-size: 11px;
    color: var(--text-muted);
  }

  .card {
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    padding: 12px;
    background: var(--bg-tertiary);
    font-size: 13px;
    line-height: 1.5;
    color: var(--text-secondary);
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .card-label {
    font-size: 11px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-muted);
  }

  .mono {
    font-family: ui-monospace, SFMono-Regular, monospace;
    font-size: 11px;
    word-break: break-all;
    margin: 0;
  }

  .info-list {
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    background: var(--bg-tertiary);
    overflow: hidden;
  }

  .info-row {
    display: flex;
    justify-content: space-between;
    padding: 10px 12px;
    font-size: 13px;
    border-bottom: 1px solid var(--border-light);
  }

  .info-row:last-child {
    border-bottom: none;
  }
  .info-row span:first-child {
    color: var(--text-secondary);
  }

  .danger-zone {
    border: 1px solid rgba(248, 113, 113, 0.25);
    background: var(--danger-dim);
    border-radius: var(--radius-md);
    padding: 14px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .danger-zone h4 {
    margin: 0;
    color: #fda4af;
    font-size: 14px;
  }
  .danger-zone p {
    margin: 0;
    font-size: 13px;
    color: var(--text-secondary);
    line-height: 1.5;
  }

  @media (max-width: 768px) {
    .tabs {
      padding: 8px 12px;
    }
    .tab-text {
      display: none;
    }
    .settings-body {
      padding: 16px 12px;
    }
  }
</style>
