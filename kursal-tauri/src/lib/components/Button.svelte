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
    <Spinner size={16} color="currentColor" />
  {:else}
    {@render children()}
  {/if}
</button>

<style>
  .button {
    min-height: 40px;
    padding: 10px 16px;
    border-radius: var(--radius-md);
    font-size: 13px;
    font-weight: 600;
    letter-spacing: 0.01em;
    transition: transform var(--transition), background var(--transition), box-shadow var(--transition), opacity var(--transition);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    white-space: nowrap;
    border: 1px solid transparent;
  }

  .button.primary {
    background: linear-gradient(135deg, #6366f1, #7c83f6);
    color: #eef2ff;
    box-shadow: 0 10px 24px rgba(79, 70, 229, 0.34);
  }

  .button.primary:hover:not(:disabled) {
    background: linear-gradient(135deg, #7a7ef8, #9097fb);
    transform: translateY(-1px);
  }

  .button.secondary {
    background: rgba(30, 41, 59, 0.74);
    color: var(--text-secondary);
    border-color: rgba(148, 163, 184, 0.26);
  }

  .button.secondary:hover:not(:disabled) {
    background: rgba(51, 65, 85, 0.82);
    color: var(--text-primary);
    transform: translateY(-1px);
  }

  .button.danger {
    background: linear-gradient(135deg, #f43f5e, #fb7185);
    color: #fff1f2;
    box-shadow: 0 10px 28px rgba(225, 29, 72, 0.3);
  }

  .button.danger:hover:not(:disabled) {
    transform: translateY(-1px);
  }

  .button:disabled {
    opacity: 0.55;
    cursor: not-allowed;
    transform: none;
    box-shadow: none;
  }
</style>
