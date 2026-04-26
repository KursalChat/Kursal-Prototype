<script lang="ts">
  import { onMount } from "svelte";
  import { Sun, Moon, Monitor, Check } from "lucide-svelte";
  import { platform } from "@tauri-apps/plugin-os";
  import {
    appearanceState,
    PALETTES,
    type PaletteId,
  } from "$lib/state/appearance.svelte";
  import { notifications } from "$lib/state/notifications.svelte";
  import SettingCard from "./SettingCard.svelte";
  import SettingRow from "./SettingRow.svelte";
  import Segmented from "./Segmented.svelte";
  import Toggle from "./Toggle.svelte";

  let autostart = $state(false);
  let autostartSupported = $state(true);
  let autostartReady = $state(false);
  let autostartBusy = $state(false);

  onMount(async () => {
    try {
      const p = platform();
      if (p === "android" || p === "ios") {
        autostartSupported = false;
        return;
      }
    } catch {
      autostartSupported = true;
    }

    try {
      const { isEnabled } = await import("@tauri-apps/plugin-autostart");
      autostart = await isEnabled();
    } catch (e) {
      console.error("Autostart plugin unavailable:", e);
      autostartSupported = false;
    } finally {
      autostartReady = true;
    }
  });

  async function toggleAutostart(enabled: boolean) {
    autostartBusy = true;
    try {
      const { enable, disable } = await import("@tauri-apps/plugin-autostart");
      if (enabled) await enable();
      else await disable();
      autostart = enabled;
      notifications.push(
        enabled ? "Auto-start enabled" : "Auto-start disabled",
        "success",
      );
    } catch (e) {
      notifications.push(`Auto-start failed: ${e}`, "error");
    } finally {
      autostartBusy = false;
    }
  }
</script>

<div class="sec-head">
  <h2>Appearance</h2>
  <p>Theme, text size, and system integration.</p>
</div>

<SettingCard title="Display">
  <SettingRow title="Theme" description="Light, dark, or follow system.">
    <Segmented
      value={appearanceState.theme}
      options={[
        { value: "light", label: "Light", icon: Sun },
        { value: "dark", label: "Dark", icon: Moon },
        { value: "system", label: "System", icon: Monitor },
      ]}
      onchange={(v) => appearanceState.setTheme(v)}
    />
  </SettingRow>
  <SettingRow title="Text size" description="Adjust interface density.">
    <Segmented
      value={appearanceState.zoom}
      options={[
        { value: "smaller", label: "Small" },
        { value: "normal", label: "Normal" },
        { value: "larger", label: "Large" },
      ]}
      onchange={(v) => appearanceState.setZoom(v)}
    />
  </SettingRow>
</SettingCard>

<SettingCard
  title="Color theme"
  description="Tinted background and accent. Classic keeps the default look."
>
  <div class="tiles" role="radiogroup" aria-label="Color theme">
    {#each PALETTES as p}
      {@const active = appearanceState.palette === p.id}
      <button
        type="button"
        role="radio"
        aria-checked={active}
        aria-label={p.label}
        class="tile"
        data-active={active}
        style="--from:{p.previewFrom}; --to:{p.previewTo}; --dot:{p.accent}"
        onclick={() => appearanceState.setPalette(p.id as PaletteId)}
      >
        <span class="tile-preview">
          <span class="tile-dot"></span>
          {#if active}
            <span class="tile-check"><Check size={12} strokeWidth={3} /></span>
          {/if}
        </span>
        <span class="tile-label">{p.label}</span>
      </button>
    {/each}
  </div>
</SettingCard>

{#if autostartSupported}
  <SettingCard title="System">
    <SettingRow
      title="Start with system"
      description="Launch Kursal automatically when you log in."
    >
      <Toggle
        checked={autostart}
        disabled={!autostartReady || autostartBusy}
        onchange={toggleAutostart}
        ariaLabel="Start with system"
      />
    </SettingRow>
  </SettingCard>
{/if}

<style>
  .tiles {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(96px, 1fr));
    gap: 10px;
    padding: 14px;
  }
  .tile {
    display: flex;
    flex-direction: column;
    align-items: stretch;
    gap: 6px;
    padding: 0;
    border-radius: 12px;
    background: transparent;
    transition: transform var(--transition);
  }
  .tile:hover {
    transform: translateY(-1px);
  }
  .tile-preview {
    position: relative;
    height: 56px;
    border-radius: 10px;
    background: radial-gradient(
        120% 100% at 10% -10%,
        color-mix(in srgb, var(--dot) 22%, transparent),
        transparent 60%
      ),
      linear-gradient(180deg, var(--from) 0%, var(--to) 100%);
    border: 2px solid transparent;
    box-shadow: inset 0 0 0 1px rgba(0, 0, 0, 0.25);
    transition:
      border-color var(--transition),
      box-shadow var(--transition);
  }
  .tile[data-active="true"] .tile-preview {
    border-color: var(--accent);
    box-shadow:
      inset 0 0 0 1px rgba(0, 0, 0, 0.25),
      0 0 0 3px var(--accent-dim);
  }
  .tile-dot {
    position: absolute;
    left: 10px;
    bottom: 10px;
    width: 14px;
    height: 14px;
    border-radius: 999px;
    background: var(--dot);
    box-shadow:
      0 0 0 1px rgba(0, 0, 0, 0.3),
      0 0 10px color-mix(in srgb, var(--dot) 60%, transparent);
  }
  .tile-check {
    position: absolute;
    top: 8px;
    right: 8px;
    width: 18px;
    height: 18px;
    border-radius: 999px;
    background: var(--accent);
    color: #fff;
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }
  .tile-label {
    font-size: 12px;
    font-weight: 600;
    color: var(--text-secondary);
    text-align: center;
  }
  .tile[data-active="true"] .tile-label {
    color: var(--text-primary);
  }
  .tile:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
    border-radius: 12px;
  }
</style>
