<script lang="ts">
  import { User, Palette, ShieldCheck, Wifi, HardDrive, Zap } from "lucide-svelte";
  import AccountSection from "$lib/components/settings/AccountSection.svelte";
  import AppearanceSection from "$lib/components/settings/AppearanceSection.svelte";
  import PrivacySection from "$lib/components/settings/PrivacySection.svelte";
  import NetworkSection from "$lib/components/settings/NetworkSection.svelte";
  import StorageSection from "$lib/components/settings/StorageSection.svelte";
  import AdvancedSection from "$lib/components/settings/AdvancedSection.svelte";

  type Category =
    | "account"
    | "appearance"
    | "privacy"
    | "network"
    | "storage"
    | "advanced";

  let activeCategory = $state<Category>("account");
  let bodyEl = $state<HTMLElement | null>(null);

  $effect(() => {
    activeCategory;
    bodyEl?.scrollTo({ top: 0, behavior: "instant" });
  });

  const categories = [
    { id: "account" as const, label: "Account", icon: User },
    { id: "appearance" as const, label: "Appearance", icon: Palette },
    { id: "privacy" as const, label: "Privacy & Security", icon: ShieldCheck },
    { id: "network" as const, label: "Network", icon: Wifi },
    { id: "storage" as const, label: "Storage", icon: HardDrive },
    { id: "advanced" as const, label: "Advanced", icon: Zap },
  ];
</script>

<div class="settings">
  <header class="settings-header" data-tauri-drag-region>
    <h2>Settings</h2>
  </header>

  <div class="settings-layout">
    <nav class="sidenav" aria-label="Settings categories">
      {#each categories as cat}
        <button
          class="nav-item"
          data-active={activeCategory === cat.id}
          onclick={() => (activeCategory = cat.id)}
        >
          <cat.icon size={15} />
          <span>{cat.label}</span>
        </button>
      {/each}
    </nav>

    <section class="settings-body" bind:this={bodyEl}>
      <div class="settings-content">
        {#if activeCategory === "account"}
          <AccountSection />
        {:else if activeCategory === "appearance"}
          <AppearanceSection />
        {:else if activeCategory === "privacy"}
          <PrivacySection />
        {:else if activeCategory === "network"}
          <NetworkSection />
        {:else if activeCategory === "storage"}
          <StorageSection />
        {:else if activeCategory === "advanced"}
          <AdvancedSection />
        {/if}
      </div>
    </section>
  </div>
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
    padding: 0 20px;
    display: flex;
    align-items: center;
    background: var(--panel);
    backdrop-filter: blur(20px) saturate(140%);
    -webkit-backdrop-filter: blur(20px) saturate(140%);
    box-shadow: inset 0 -1px 0 var(--panel-border);
    flex-shrink: 0;
  }

  .settings-header h2 {
    margin: 0;
    font-size: 18px;
    font-weight: 700;
  }

  .settings-layout {
    flex: 1;
    display: flex;
    min-height: 0;
    overflow: hidden;
  }

  .sidenav {
    flex-shrink: 0;
    width: 220px;
    padding: 16px 10px;
    background: var(--panel);
    backdrop-filter: blur(20px) saturate(140%);
    -webkit-backdrop-filter: blur(20px) saturate(140%);
    box-shadow: inset -1px 0 0 var(--panel-border);
    display: flex;
    flex-direction: column;
    gap: 2px;
    overflow-y: auto;
  }

  .nav-item {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 12px;
    border-radius: var(--radius-md);
    font-size: 13px;
    font-weight: 500;
    color: var(--text-secondary);
    text-align: left;
    transition: background var(--transition), color var(--transition);
    white-space: nowrap;
  }
  .nav-item:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }
  .nav-item[data-active="true"] {
    background: var(--accent-dim);
    color: var(--accent);
  }
  :global(:root[data-theme="light"]) .nav-item[data-active="true"] {
    color: var(--accent);
  }

  .settings-body {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 36px 32px 96px;
  }

  .settings-content {
    width: 100%;
    max-width: 720px;
    margin: 0 auto;
    display: flex;
    flex-direction: column;
    gap: 26px;
  }

  @media (max-width: 900px) {
    .settings-layout { flex-direction: column; }
    .sidenav {
      width: 100%;
      flex-direction: row;
      overflow-x: auto;
      padding: 10px 12px;
      border-right: none;
      border-bottom: 1px solid var(--border);
      gap: 4px;
      scrollbar-width: none;
      -ms-overflow-style: none;
    }
    .sidenav::-webkit-scrollbar { display: none; }
    .nav-item { flex-shrink: 0; }
    .settings-body { padding: 24px 16px 84px; }
  }
</style>
