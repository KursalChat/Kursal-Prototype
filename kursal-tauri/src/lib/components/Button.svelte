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
    background: var(--accent);
    color: #fff;
  }

  .button.primary:hover:not(:disabled) {
    background: var(--accent-hover);
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
  }

  .button.danger:hover:not(:disabled) {
    opacity: 0.9;
  }

  .button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
</style>
