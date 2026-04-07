<script lang="ts">
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import { QrCode, FileArchive, Radar } from "lucide-svelte";

  const methods = [
    { label: "One-Time Code", href: "/add-contact/otp", icon: QrCode },
    { label: "Contact File", href: "/add-contact/ltc", icon: FileArchive },
    { label: "Nearby Devices", href: "/add-contact/nearby", icon: Radar },
  ];

  let { children } = $props();
</script>

<div class="container">
  <div class="page-header">
    <div class="header-copy">
      <h1>Add Contact</h1>
      <p class="subtitle">Choose OTP, a contact file, or nearby pairing.</p>
    </div>
  </div>

  <nav class="tab-bar" aria-label="Add contact methods">
    {#each methods as method}
      <button
        type="button"
        class="tab"
        class:active={page.url.pathname.startsWith(method.href)}
        aria-current={page.url.pathname.startsWith(method.href)
          ? "page"
          : undefined}
        onclick={() => goto(method.href)}
      >
        <method.icon size={16} strokeWidth={2.2} />
        <span class="tab-label">{method.label}</span>
      </button>
    {/each}
  </nav>

  <div class="content">
    {@render children()}
  </div>
</div>

<style>
  .container {
    height: 100%;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }

  .page-header {
    padding: 18px;
    display: flex;
    align-items: center;
    border-bottom: 1px solid var(--border);
    background: rgba(15, 23, 42, 0.56);
    backdrop-filter: blur(18px);
    flex-shrink: 0;
  }

  .header-copy {
    display: grid;
    gap: 2px;
  }

  .page-header h1 {
    margin: 0;
    font-size: 22px;
    font-weight: 700;
  }

  .subtitle {
    margin: 0;
    color: var(--text-secondary);
    font-size: 13px;
    line-height: 1.45;
  }

  .tab-bar {
    display: flex;
    gap: 8px;
    padding: 12px 18px;
    border-bottom: 1px solid var(--border);
    background: rgba(11, 17, 32, 0.48);
    overflow-x: auto;
    flex-shrink: 0;
  }

  .tab {
    background: rgba(15, 23, 42, 0.4);
    border: 1px solid rgba(148, 163, 184, 0.22);
    color: var(--text-secondary);
    padding: 10px 13px;
    cursor: pointer;
    font-size: 13px;
    font-weight: 600;
    border-radius: 10px;
    display: flex;
    align-items: center;
    gap: 8px;
    white-space: nowrap;
    transition: all var(--transition);
  }

  .tab:hover {
    background: rgba(30, 41, 59, 0.72);
    border-color: rgba(148, 163, 184, 0.42);
    color: var(--text-primary);
  }

  .tab.active {
    background: rgba(51, 65, 85, 0.9);
    border-color: rgba(129, 140, 248, 0.5);
    color: var(--text-primary);
  }

  .content {
    flex: 1;
    overflow-y: auto;
    padding: 20px 18px;
  }

  @media (max-width: 900px) {
    .page-header {
      padding: 14px;
    }

    .page-header h1 {
      font-size: 20px;
    }

    .subtitle {
      font-size: 12px;
    }

    .tab-bar {
      padding: 10px 14px;
      gap: 8px;
    }

    .tab-label {
      display: none;
    }

    .tab {
      padding: 9px 11px;
    }

    .content {
      padding: 14px;
    }
  }
</style>
