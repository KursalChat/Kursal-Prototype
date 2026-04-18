<script lang="ts">
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import { QrCode, FileArchive, Radar } from "lucide-svelte";

  const methods = [
    { label: "One-Time Code", href: "/add-contact/otp", icon: QrCode },
    { label: "Contact File", href: "/add-contact/ltc", icon: FileArchive },
    { label: "Nearby", href: "/add-contact/nearby", icon: Radar },
  ];

  let { children } = $props();
</script>

<div class="add-contact">
  <header class="ac-header">
    <h2>Add Contact</h2>
  </header>

  <nav class="tabs">
    {#each methods as m}
      <button
        class="tab"
        data-active={page.url.pathname.startsWith(m.href)}
        onclick={() => goto(m.href)}
      >
        <m.icon size={15} />
        <span class="tab-text">{m.label}</span>
      </button>
    {/each}
  </nav>

  <div class="ac-body">
    {@render children()}
  </div>
</div>

<style>
  .add-contact {
    height: 100%;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }

  .ac-header {
    height: var(--header-height);
    padding: 0 16px;
    display: flex;
    align-items: center;
    border-bottom: 1px solid var(--border);
    background: var(--bg-secondary);
    flex-shrink: 0;
  }

  .ac-header h2 {
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

  .ac-body {
    flex: 1;
    overflow-y: auto;
    padding: 20px 16px;
  }

  @media (max-width: 768px) {
    .tabs { padding: 8px 12px; }
    .tab-text { display: none; }
    .ac-body { padding: 16px 12px; }
  }
</style>
