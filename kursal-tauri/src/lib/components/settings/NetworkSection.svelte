<script lang="ts">
  import { onMount } from "svelte";
  import { Save } from "lucide-svelte";
  import { type RelayConfig } from "$lib/api/settings";
  import { settingsState } from "$lib/state/settings.svelte";
  import { notifications } from "$lib/state/notifications.svelte";
  import Button from "$lib/components/Button.svelte";
  import SettingCard from "./SettingCard.svelte";
  import SettingRow from "./SettingRow.svelte";
  import Toggle from "./Toggle.svelte";
  import TextInput from "./TextInput.svelte";

  const DEFAULT_PORT = "4891";
  let relay = $state<RelayConfig>({ ...settingsState.relay });
  let relaySaving = $state(false);
  let port = $state<string>(
    settingsState.listeningPort === null
      ? DEFAULT_PORT
      : String(settingsState.listeningPort),
  );
  let portSaving = $state(false);
  let initialized = $state(settingsState.loaded);

  onMount(async () => {
    await settingsState.load();
    if (!initialized) {
      relay = { ...settingsState.relay };
      port =
        settingsState.listeningPort === null
          ? DEFAULT_PORT
          : String(settingsState.listeningPort);
      initialized = true;
    }
  });

  const relayDirty = $derived(
    relay.enabled !== settingsState.relay.enabled ||
      relay.maxConnections !== settingsState.relay.maxConnections ||
      relay.maxConnectionsPerIp !== settingsState.relay.maxConnectionsPerIp,
  );
  const portDirty = $derived(
    port.trim() !==
      (settingsState.listeningPort === null
        ? DEFAULT_PORT
        : String(settingsState.listeningPort)),
  );
  const nearby = $derived(settingsState.nearbyShare);

  async function saveRelay() {
    relaySaving = true;
    try {
      const clean: RelayConfig = {
        ...relay,
        maxConnections: Math.max(1, Math.floor(relay.maxConnections || 0)),
        maxConnectionsPerIp: Math.max(
          1,
          Math.floor(relay.maxConnectionsPerIp || 0),
        ),
      };
      await settingsState.setRelay(clean);
      relay = { ...clean };
      notifications.push("Relay settings saved", "success");
    } catch (e) {
      notifications.push(`Failed: ${e}`, "error");
    } finally {
      relaySaving = false;
    }
  }

  async function savePort() {
    const trimmed = port.trim();
    let parsed: number | null = null;
    if (trimmed.length > 0) {
      const n = Number(trimmed);
      if (!Number.isInteger(n) || n < 1 || n > 65535) {
        notifications.push("Port must be 1–65535 or empty", "error");
        return;
      }
      parsed = n;
    }
    portSaving = true;
    try {
      await settingsState.setPort(parsed);
      notifications.push("Port saved — restart may be required", "success");
    } catch (e) {
      notifications.push(`Failed: ${e}`, "error");
    } finally {
      portSaving = false;
    }
  }

  async function toggleNearby(value: boolean) {
    try {
      await settingsState.setNearby(value);
    } catch (e) {
      notifications.push(`Failed: ${e}`, "error");
    }
  }
</script>

<div class="sec-head">
  <h2>Network</h2>
  <p>Relay, transport, and local discovery.</p>
</div>

<SettingCard title="Relay">
  <SettingRow
    title="Run as relay"
    description="Help other peers connect by relaying encrypted traffic. Messages stay E2E encrypted."
  >
    <Toggle
      checked={relay.enabled}
      onchange={(v) => (relay = { ...relay, enabled: v })}
      ariaLabel="Run as relay"
    />
  </SettingRow>
  {#if relay.enabled}
    <SettingRow
      title="Max connections"
      description="Total concurrent relayed peers."
    >
      <TextInput
        type="number"
        min={1}
        max={100000}
        width="96px"
        value={String(relay.maxConnections)}
        onchange={(v) => (relay = { ...relay, maxConnections: Number(v) || 0 })}
      />
    </SettingRow>
    <SettingRow
      title="Max per IP"
      description="Limit how many connections come from the same address."
    >
      <TextInput
        type="number"
        min={1}
        max={10000}
        width="96px"
        value={String(relay.maxConnectionsPerIp)}
        onchange={(v) =>
          (relay = { ...relay, maxConnectionsPerIp: Number(v) || 0 })}
      />
    </SettingRow>
  {/if}
  {#snippet footer()}
    <Button onclick={saveRelay} loading={relaySaving} disabled={!relayDirty}>
      <Save size={13} /> Save relay
    </Button>
  {/snippet}
</SettingCard>

<SettingCard title="Transport">
  <SettingRow
    title="Listening port"
    description="Which port Kursal listens on. Clear the field to pick a random port."
  >
    <TextInput
      type="text"
      placeholder="random"
      width="110px"
      bind:value={port}
    />
    <Button onclick={savePort} loading={portSaving} disabled={!portDirty}
      >Save</Button
    >
  </SettingRow>
</SettingCard>

<SettingCard title="Discovery">
  <SettingRow
    title="Nearby share"
    description="Allow local discovery via mDNS. Disabling this will only enable Bluetooth when on the Nearby tab."
  >
    <Toggle checked={nearby} onchange={toggleNearby} ariaLabel="Nearby share" />
  </SettingRow>
</SettingCard>
