<script lang="ts">
  import { onMount } from "svelte";
  import {
    Copy,
    ExternalLink,
    RefreshCw,
    ChevronDown,
    Eye,
    EyeOff,
    Activity,
    KeyRound,
  } from "lucide-svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { getVersion } from "@tauri-apps/api/app";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { writeText } from "@tauri-apps/plugin-clipboard-manager";
  import {
    generateLocalApiToken,
    getUpdaterEnabled,
    setUpdaterEnabled,
    type LocalApiConfig,
  } from "$lib/api/settings";
  import { settingsState } from "$lib/state/settings.svelte";
  import { notifications } from "$lib/state/notifications.svelte";
  import Button from "$lib/components/Button.svelte";
  import Benchmark from "$lib/components/Benchmark.svelte";
  import SettingCard from "./SettingCard.svelte";
  import SettingRow from "./SettingRow.svelte";
  import Toggle from "./Toggle.svelte";
  import TextInput from "./TextInput.svelte";

  let appVersion = $state("...");
  let checkingForUpdates = $state(false);
  let autoUpdater = $state(true);

  let api = $state<LocalApiConfig>({ ...settingsState.localApi });
  let apiSaving = $state(false);
  let apiInitialized = $state(settingsState.loaded);
  let newToken = $state<string | null>(null);
  let tokenVisible = $state(false);
  let generatingToken = $state(false);

  let benchmarksOpen = $state(false);

  onMount(async () => {
    try {
      appVersion = `v${await getVersion()}`;
    } catch {
      appVersion = "Unknown";
    }
    try {
      autoUpdater = await getUpdaterEnabled();
    } catch {
      /* default true */
    }
    await settingsState.load();
    if (!apiInitialized) {
      api = { ...settingsState.localApi };
      apiInitialized = true;
    }
  });

  const apiDirty = $derived(
    api.enabled !== settingsState.localApi.enabled ||
      api.hostOnNetwork !== settingsState.localApi.hostOnNetwork ||
      api.port !== settingsState.localApi.port,
  );

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

  async function handleAutoUpdater(value: boolean) {
    autoUpdater = value;
    try {
      await setUpdaterEnabled(value);
    } catch (e) {
      notifications.push(`Failed: ${e}`, "error");
    }
  }

  async function saveApi() {
    apiSaving = true;
    try {
      const clean: LocalApiConfig = {
        ...api,
        port: Math.max(1, Math.min(65535, Math.floor(api.port || 4892))),
      };
      await settingsState.setLocalApi(clean);
      api = { ...clean };
      notifications.push(
        "Local API saved — restart Kursal to apply changes",
        "info",
      );
    } catch (e) {
      notifications.push(`Failed: ${e}`, "error");
    } finally {
      apiSaving = false;
    }
  }

  async function generateToken() {
    generatingToken = true;
    try {
      newToken = await generateLocalApiToken();
      tokenVisible = true;
      notifications.push("New token generated — copy it now", "info");
    } catch (e) {
      notifications.push(`Failed: ${e}`, "error");
    } finally {
      generatingToken = false;
    }
  }

  function hideToken() {
    newToken = null;
    tokenVisible = false;
  }

  async function copyToken() {
    if (!newToken) return;
    try {
      await writeText(newToken);
      notifications.push("Token copied", "success");
    } catch (e) {
      notifications.push(`Copy failed: ${e}`, "error");
    }
  }

  async function openLink(url: string) {
    try {
      await openUrl(url);
    } catch (e) {
      console.error(e);
    }
  }
</script>

<div class="sec-head">
  <h2>Advanced</h2>
  <p>Updates, developer integrations, and diagnostics.</p>
</div>

<SettingCard title="About">
  <SettingRow title="Version">
    <span class="value">Kursal {appVersion}</span>
  </SettingRow>
  <SettingRow title="License">
    <span class="value">AGPL-3.0</span>
  </SettingRow>
</SettingCard>

<SettingCard title="Updates">
  <SettingRow
    title="Check for updates"
    description="Look for a newer version of Kursal now."
  >
    <Button onclick={handleCheckForUpdates} loading={checkingForUpdates}>
      <RefreshCw size={13} /> Check
    </Button>
  </SettingRow>
  <SettingRow
    title="Auto-updater"
    description="Install updates automatically when available."
  >
    <Toggle
      checked={autoUpdater}
      onchange={handleAutoUpdater}
      ariaLabel="Auto-updater"
    />
  </SettingRow>
</SettingCard>

<SettingCard
  title="Local API server"
  description="Expose a local HTTP server for automation and custom clients. Treat the auth token as a password."
>
  <SettingRow title="Enable" description="Start an HTTP API on this machine.">
    <Toggle
      checked={api.enabled}
      onchange={(v) => (api = { ...api, enabled: v })}
      ariaLabel="Enable local API"
    />
  </SettingRow>
  {#if api.enabled}
    <SettingRow
      title="Host on network"
      description="Expose beyond localhost. Anyone on your network can reach it."
    >
      <Toggle
        checked={api.hostOnNetwork}
        onchange={(v) => (api = { ...api, hostOnNetwork: v })}
        ariaLabel="Host on network"
      />
    </SettingRow>
    <SettingRow title="Port">
      <TextInput
        type="number"
        min={1}
        max={65535}
        width="96px"
        value={String(api.port)}
        onchange={(v) => (api = { ...api, port: Number(v) || 4892 })}
      />
    </SettingRow>
    <SettingRow
      title="Auth token"
      description="Generate a new token. Old tokens become invalid."
    >
      <Button
        variant="secondary"
        loading={generatingToken}
        onclick={generateToken}
      >
        <KeyRound size={13} /> New token
      </Button>
    </SettingRow>
    {#if newToken}
      <div class="token-display">
        <div class="token-head">
          <span class="token-label">New token — shown once</span>
          <button
            class="icon-btn"
            onclick={() => (tokenVisible = !tokenVisible)}
            aria-label="Toggle visibility"
          >
            {#if tokenVisible}<EyeOff size={13} />{:else}<Eye size={13} />{/if}
          </button>
        </div>
        <code class="token-value mono selectable">
          {tokenVisible ? newToken : "•".repeat(newToken.length)}
        </code>
        <div class="token-actions">
          <Button variant="secondary" onclick={copyToken}>
            <Copy size={13} /> Copy
          </Button>
          <Button variant="secondary" onclick={hideToken}>Dismiss</Button>
        </div>
      </div>
    {/if}
  {/if}
  {#snippet footer()}
    <Button onclick={saveApi} loading={apiSaving} disabled={!apiDirty}
      >Save</Button
    >
  {/snippet}
</SettingCard>

<SettingCard
  title="Benchmarks"
  description="Test device cryptographic performance."
>
  <div class="collapser">
    <button
      class="collapse-head"
      onclick={() => (benchmarksOpen = !benchmarksOpen)}
      aria-expanded={benchmarksOpen}
    >
      <div class="collapse-left">
        <Activity size={14} />
        <span>OTP hashing</span>
      </div>
      <ChevronDown
        size={14}
        class="chev"
        style="transform: rotate({benchmarksOpen
          ? 0
          : -90}deg); transition: transform 150ms ease"
      />
    </button>
    {#if benchmarksOpen}
      <div class="collapse-body">
        <Benchmark
          name="OTP Hashing"
          description="Measures Argon2id OTP hashing speed."
        />
      </div>
    {/if}
  </div>
</SettingCard>

<SettingCard title="Credits">
  <ul class="credits">
    <li>
      <div class="credit-name">
        <button
          class="link"
          onclick={() => openLink("https://github.com/KodeurKubik")}
        >
          Kodeur_Kubik <ExternalLink size={11} />
        </button>
      </div>
      <span class="credit-role">Coding &amp; paper</span>
    </li>
    <li>
      <div class="credit-name">Arlo</div>
      <span class="credit-role">Paper</span>
    </li>
    <li>
      <div class="credit-name">
        <button
          class="link"
          onclick={() => openLink("https://www.youtube.com/@ChoosingBerry")}
        >
          ChoosingBerry <ExternalLink size={11} />
        </button>
      </div>
      <span class="credit-role">Art</span>
    </li>
  </ul>
</SettingCard>

<style>
  .value {
    font-size: 13px;
    color: var(--text-primary);
    font-weight: 500;
  }

  .token-display {
    padding: 14px;
    border-top: 1px solid var(--border-light);
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .token-head {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }
  .token-label {
    font-size: 11px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--accent);
  }
  .token-value {
    background: var(--bg-input);
    padding: 10px 12px;
    border-radius: var(--radius-sm);
    font-family: ui-monospace, SFMono-Regular, monospace;
    font-size: 12px;
    color: var(--text-primary);
    word-break: break-all;
    border: 1px solid var(--border);
  }
  .token-actions {
    display: flex;
    gap: 6px;
  }
  .icon-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    padding: 6px;
    border-radius: var(--radius-sm);
    background: var(--bg-input);
    border: 1px solid var(--border);
    color: var(--text-secondary);
    cursor: pointer;
  }
  .icon-btn:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .collapser {
    display: flex;
    flex-direction: column;
  }
  .collapse-head {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 12px 14px;
    background: none;
    border: none;
    color: var(--text-primary);
    cursor: pointer;
    font-size: 13px;
    font-weight: 600;
  }
  .collapse-head:hover {
    background: var(--bg-hover);
  }
  .collapse-left {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .collapse-body {
    padding: 12px 14px;
    border-top: 1px solid var(--border-light);
  }

  .credits {
    list-style: none;
    display: flex;
    flex-direction: column;
  }
  .credits li {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 12px 14px;
    border-bottom: 1px solid var(--border-light);
    font-size: 13px;
  }
  .credits li:last-child {
    border-bottom: none;
  }
  .credit-name {
    color: var(--text-primary);
    font-weight: 600;
  }
  .credit-role {
    color: var(--text-muted);
    font-size: 12px;
  }
  .link {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    color: var(--accent);
    background: none;
    border: none;
    padding: 0;
    font: inherit;
    font-weight: 600;
    cursor: pointer;
  }
  .link:hover {
    color: var(--accent-hover);
  }
</style>
