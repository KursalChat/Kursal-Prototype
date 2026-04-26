<script lang="ts">
  import Spinner from "./Spinner.svelte";

  let {
    variant = "primary",
    loading = false,
    disabled = false,
    onclick,
    children,
  }: {
    variant?: "primary" | "secondary" | "danger";
    loading?: boolean;
    disabled?: boolean;
    onclick?: () => void;
    children: any;
  } = $props();
</script>

<button class="button {variant}" {disabled} {onclick} aria-busy={loading}>
  {#if loading}
    <Spinner size={14} color="currentColor" />
  {:else}
    {@render children()}
  {/if}
</button>

<style>
  .button {
    min-height: 36px;
    padding: 8px 14px;
    border-radius: var(--radius-md);
    font-size: 13px;
    font-weight: 600;
    transition: background var(--transition), transform var(--transition), opacity var(--transition);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    white-space: nowrap;
    border: 1px solid transparent;
  }

  .button.primary {
    background: var(--accent-solid);
    color: #fff;
  }

  .button.primary:hover:not(:disabled) {
    background: color-mix(in srgb, var(--accent-solid), white 10%);
  }

  .button.secondary {
    background: var(--bg-tertiary);
    color: var(--text-primary);
    border-color: var(--border);
  }

  .button.secondary:hover:not(:disabled) {
    background: var(--bg-hover);
  }

  .button.danger {
    background: var(--danger);
    color: #fff;
    border-color: var(--danger);
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.15);
  }

  .button.danger:hover:not(:disabled) {
    background: var(--danger-hover);
    border-color: var(--danger-hover);
  }

  .button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
</style>
