<script lang="ts" generics="T extends string">
  let {
    value,
    options,
    onchange,
    disabled = false,
    size = "md",
    fullWidth = false,
  }: {
    value: T;
    options: { value: T; label: string; icon?: any }[];
    onchange?: (value: T) => void;
    disabled?: boolean;
    size?: "sm" | "md";
    fullWidth?: boolean;
  } = $props();
</script>

<div class="segmented" data-size={size} data-full={fullWidth}>
  {#each options as opt}
    <button
      type="button"
      class="seg"
      data-active={value === opt.value}
      disabled={disabled}
      onclick={() => onchange?.(opt.value)}
    >
      {#if opt.icon}
        {@const Icon = opt.icon}
        <Icon size={size === "sm" ? 12 : 14} />
      {/if}
      <span>{opt.label}</span>
    </button>
  {/each}
</div>

<style>
  .segmented {
    display: inline-flex;
    padding: 3px;
    background: var(--bg-input);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    gap: 2px;
  }
  .segmented[data-full="true"] { display: flex; width: 100%; }
  .seg {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 5px;
    padding: 6px 12px;
    border-radius: 7px;
    font-size: 13px;
    font-weight: 500;
    color: var(--text-secondary);
    transition: background var(--transition), color var(--transition);
    white-space: nowrap;
    flex: 1;
  }
  .segmented[data-size="sm"] .seg {
    padding: 4px 10px;
    font-size: 12px;
  }
  .seg:hover:not(:disabled):not([data-active="true"]) {
    color: var(--text-primary);
  }
  .seg[data-active="true"] {
    background: var(--bg-tertiary);
    color: var(--text-primary);
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.15);
  }
  .seg:disabled { opacity: 0.5; cursor: not-allowed; }
</style>
