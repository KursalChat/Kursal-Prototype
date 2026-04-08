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
    } catch (e) {
      console.error("Failed to load app version", e);
      appVersion = "Unknown";
    }
  });

  async function handleAvatarSelection(event: Event) {
    const input = event.target as HTMLInputElement;
    if (!input.files || input.files.length === 0) return;

    const file = input.files[0];
    if (!file.type.startsWith("image/")) {
      notifications.push("Selected file is not an image", "error");
      return;
    }

    try {
      const bitmap = await createImageBitmap(file);
      const canvas = document.createElement("canvas");
      const MAX_SIZE = 256;

      let width = bitmap.width;
      let height = bitmap.height;

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
      if (!ctx) throw new Error("Failed to get canvas 2D context");
      ctx.drawImage(bitmap, 0, 0, width, height);

      // Compress to webp at 0.8 quality
      const dataUrl = canvas.toDataURL("image/webp", 0.8);
      // Ensure it is not too large
      if (dataUrl.length > 55000) {
        // ~40KB base64 encoded
        notifications.push(
          "Image too complex, please try a simpler or smaller image",
          "error",
        );
        return;
      }

      // Store raw Base64 (strip the prefix structure `data:image/webp;base64,`)
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
      notifications.push("Profile saved and broadcasted", "success");
    } catch (e) {
      console.error("Failed to broadcast profile", e);
      notifications.push("Saved locally but failed to broadcast", "error");
    } finally {
      savingProfile = false;
    }
  }

  async function handleRotate() {
    const isConfirmed = await confirm(
      "Rotate your peer ID? Your contacts will be notified automatically.",
      { title: "Rotate Peer ID", kind: "warning" },
    );
    if (!isConfirmed) return;

    rotating = true;
    try {
      await rotatePeerId();
      await profileState.refreshPeerId();
      notifications.push("Peer ID rotated successfully", "success");
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

<div class="settings-container">
  <div class="settings-header">
    <h2>Settings</h2>
  </div>

  <nav class="settings-tabs">
    {#each categories as category}
      <button
        class="settings-tab"
        class:active={activeCategory === category.id}
        onclick={() => (activeCategory = category.id)}
        aria-label={category.label}
      >
        <category.icon size={16} />
        <span>{category.label}</span>
      </button>
    {/each}
  </nav>

  <section class="settings-content">
    {#if activeCategory === "account"}
      <div class="settings-section">
        <div class="section-header">
          <h3>Account</h3>
          <p>Manage your local profile.</p>
        </div>

        <div class="form-group form-avatar-row">
          <div class="avatar-preview">
            <Avatar name={displayName || "You"} src={avatarBase64} size={72} />
          </div>
          <div class="avatar-actions">
            <label class="btn-secondary">
              <Upload size={16} />
              Change Photo
              <input
                type="file"
                accept="image/*"
                onchange={handleAvatarSelection}
                style="display: none;"
              />
            </label>
            {#if avatarBase64}
              <button
                class="text-btn danger-text"
                onclick={() => {
                  avatarBase64 = null;
                  avatarBytes = null;
                }}
              >
                Remove
              </button>
            {/if}
          </div>
        </div>

        <div class="form-group">
          <label for="displayName">Display Name</label>
          <input
            id="displayName"
            bind:value={displayName}
            placeholder="Enter your display name"
          />
          <p class="form-hint">
            Shown in conversation headers on your device and sent to your
            contacts.
          </p>
        </div>

        <div class="form-actions">
          <Button onclick={saveProfile} loading={savingProfile}
            >Save Profile</Button
          >
        </div>

        {#if profileState.peerId}
          <div class="info-card">
            <p class="info-label">Your Peer ID</p>
            <p class="mono">{profileState.peerId}</p>
          </div>
        {/if}
      </div>
    {:else if activeCategory === "security"}
      <div class="settings-section">
        <div class="section-header">
          <h3>Security</h3>
          <p>Manage your cryptographic identity.</p>
        </div>

        <div class="info-card">
          <p class="info-label">Encryption</p>
          <p>
            All messages are end-to-end encrypted using PQXDH + Double Ratchet
            with post-quantum Kyber1024 and Dilithium-5 signatures.
          </p>
        </div>

        <div class="danger-zone">
          <h4>Rotate Peer ID</h4>
          <p>
            Generates a new transport identity and notifies your contacts. Use
            this if you believe your network identity has been compromised.
          </p>
          <Button variant="danger" loading={rotating} onclick={handleRotate}
            >Rotate Peer ID</Button
          >
        </div>
      </div>
    {:else if activeCategory === "network"}
      <div class="settings-section">
        <div class="section-header">
          <h3>Network</h3>
          <p>Control how Kursal connects to peers.</p>
        </div>

        <div class="info-card">
          <p class="info-label">Connection Methods</p>
          <p>
            Kursal attempts direct peer-to-peer connections first and may use
            additional network fallback strategies when needed. All traffic
            remains end-to-end encrypted regardless of route.
          </p>
        </div>
      </div>
    {:else if activeCategory === "benchmarks"}
      <div class="settings-section">
        <div class="section-header">
          <h3>Benchmarks</h3>
          <p>
            Test the cryptographic and computational capabilities of your
            device.
          </p>
        </div>

        <Benchmark
          name="OTP Hashing"
          description="Measures the device's capability to generate and hash OTPs using Argon2id."
        />
      </div>
    {:else if activeCategory === "advanced"}
      <div class="settings-section">
        <div class="section-header">
          <h3>Advanced</h3>
          <p>
            App info and diagnostics. The settings tab will be completely
            reworked in the future.
          </p>
        </div>

        <div class="app-info">
          <div class="info-item">
            <span>Version</span><span>Kursal {appVersion}</span>
          </div>
          <div class="info-item">
            <span>Contacts</span><span>{contactsState.contacts.length}</span>
          </div>
          <div class="info-item"><span>License</span><span>AGPL-3.0</span></div>
        </div>

        <div class="info-card" style="margin-top: 24px;">
          <p class="info-label">Software Updates</p>
          <p style="margin-bottom: 12px; font-size: 14px; color: var(--text-secondary);">Check to see if there is a newer version of Kursal available to download.</p>
          <Button onclick={handleCheckForUpdates} loading={checkingForUpdates}
            >Check for Updates</Button
          >
        </div>
      </div>
    {/if}
  </section>
</div>

<style>
  .settings-container {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
    overflow: hidden;
    background: rgba(2, 6, 23, 0.35);
  }

  .form-avatar-row {
    flex-direction: row;
    align-items: center;
    gap: 16px;
    margin-bottom: 24px;
  }

  .avatar-actions {
    display: flex;
    flex-direction: column;
    gap: 8px;
    align-items: flex-start;
  }

  .btn-secondary {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    padding: 8px 16px;
    border-radius: var(--radius-md);
    background: rgba(255, 255, 255, 0.08);
    border: 1px solid rgba(255, 255, 255, 0.1);
    color: var(--text-primary);
    font-size: 13px;
    font-weight: 500;
    cursor: pointer;
    transition: background var(--transition);
  }

  .btn-secondary:hover {
    background: rgba(255, 255, 255, 0.12);
  }

  .text-btn {
    border: none;
    background: none;
    padding: 0;
    font-size: 13px;
    cursor: pointer;
    font-weight: 500;
  }

  .danger-text {
    color: var(--danger);
  }

  .danger-text:hover {
    opacity: 0.8;
  }

  .settings-header {
    min-height: var(--header-height);
    padding: 0 18px;
    display: flex;
    align-items: center;
    border-bottom: 1px solid var(--border);
    background: rgba(15, 23, 42, 0.5);
    backdrop-filter: blur(20px);
    flex-shrink: 0;
  }

  .settings-header h2 {
    margin: 0;
    font-size: 20px;
    font-weight: 700;
    letter-spacing: -0.02em;
  }

  .settings-tabs {
    display: flex;
    gap: 4px;
    padding: 10px 18px;
    border-bottom: 1px solid var(--border);
    background: rgba(15, 23, 42, 0.35);
    flex-shrink: 0;
  }

  .settings-tab {
    border: 1px solid transparent;
    background: transparent;
    color: var(--text-secondary);
    display: flex;
    align-items: center;
    gap: 8px;
    border-radius: 10px;
    padding: 8px 14px;
    cursor: pointer;
    text-align: left;
    transition: all var(--transition);
    font-size: 13px;
    font-weight: 600;
  }

  .settings-tab:hover {
    background: rgba(51, 65, 85, 0.56);
    color: var(--text-primary);
  }

  .settings-tab.active {
    background: linear-gradient(
      135deg,
      rgba(67, 56, 202, 0.4),
      rgba(79, 70, 229, 0.25)
    );
    border-color: rgba(129, 140, 248, 0.5);
    color: #eef2ff;
  }

  .settings-content {
    flex: 1;
    overflow-y: auto;
    padding: 22px;
    min-width: 0;
  }

  .settings-section {
    max-width: 680px;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  .section-header h3 {
    margin: 0;
    font-size: 22px;
    letter-spacing: -0.02em;
  }

  .section-header p {
    margin: 6px 0 0;
    color: var(--text-muted);
    font-size: 14px;
  }

  .form-group {
    display: flex;
    flex-direction: column;
    gap: 7px;
  }

  label {
    font-size: 13px;
    color: var(--text-secondary);
    font-weight: 600;
  }

  input {
    border-radius: 10px;
    border: 1px solid var(--border);
    background: rgba(15, 23, 42, 0.8);
    color: var(--text-primary);
    padding: 10px 12px;
    font-size: 14px;
    outline: none;
  }

  input:focus {
    border-color: rgba(129, 140, 248, 0.6);
    box-shadow: 0 0 0 3px rgba(99, 102, 241, 0.2);
  }

  .form-hint {
    margin: 0;
    font-size: 12px;
    color: var(--text-muted);
  }

  .form-actions {
    display: flex;
    gap: 8px;
  }

  .info-card {
    border: 1px solid rgba(148, 163, 184, 0.24);
    border-radius: 12px;
    padding: 14px;
    background: rgba(15, 23, 42, 0.45);
    font-size: 14px;
    line-height: 1.6;
    color: var(--text-secondary);
  }

  .info-label {
    margin: 0 0 6px;
    font-size: 11px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.1em;
    color: var(--text-muted);
  }

  .mono {
    margin: 0;
    font-family: "Monaco", "Courier New", monospace;
    font-size: 12px;
    word-break: break-all;
  }

  .app-info {
    border: 1px solid rgba(148, 163, 184, 0.24);
    border-radius: 12px;
    padding: 0 12px;
    background: rgba(15, 23, 42, 0.45);
  }

  .info-item {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 12px 0;
    border-bottom: 1px solid rgba(148, 163, 184, 0.2);
    font-size: 14px;
  }

  .info-item:last-child {
    border-bottom: none;
  }

  .info-item span:first-child {
    color: var(--text-secondary);
  }

  .danger-zone {
    border: 1px solid rgba(244, 63, 94, 0.35);
    background: rgba(190, 24, 93, 0.08);
    border-radius: 12px;
    padding: 14px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .danger-zone h4 {
    margin: 0;
    color: #fda4af;
  }

  .danger-zone p {
    margin: 0;
    color: var(--text-secondary);
    font-size: 13px;
    line-height: 1.5;
  }

  @media (max-width: 900px) {
    .settings-tabs {
      overflow-x: auto;
      padding: 8px 12px;
    }

    .settings-tab {
      white-space: nowrap;
    }

    .settings-content {
      padding: 14px;
    }
  }
</style>
