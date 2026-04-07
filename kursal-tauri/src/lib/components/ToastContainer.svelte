<script lang="ts">
  import { fly } from 'svelte/transition';
  import { notifications } from '$lib/state/notifications.svelte';
</script>

<div class="container">
  {#each notifications.toasts as toast, index (toast.id)}
    <div
      in:fly={{ y: 10, duration: 200 }}
      out:fly={{ y: 10, duration: 150 }}
      class="toast {toast.kind}"
      style={`--stack-index: ${index}; --stack-count: ${notifications.toasts.length};`}
    >
      <span>{toast.message}</span>
      <button onclick={() => notifications.dismiss(toast.id)}>×</button>
    </div>
  {/each}
</div>

<style>
  .container {
    position: fixed;
    bottom: 24px;
    right: 24px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    z-index: 9999;
    pointer-events: none;
  }

  .toast {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px 16px;
    background: var(--bg-secondary);
    border-radius: var(--radius-md);
    border-left: 3px solid;
    font-size: 14px;
    pointer-events: all;
    min-width: 240px;
    position: relative;
    transition:
      transform var(--transition),
      margin var(--transition),
      opacity var(--transition),
      box-shadow var(--transition);
    box-shadow: 0 8px 18px rgba(2, 6, 23, 0.24);
  }

  .container:not(:hover) .toast {
    --depth: calc(var(--stack-count) - var(--stack-index) - 1);
    transform: translateY(calc(var(--depth) * 6px)) scale(calc(1 - var(--depth) * 0.02));
    opacity: calc(1 - var(--depth) * 0.12);
  }

  .container:not(:hover) .toast + .toast {
    margin-top: -38px;
  }

  .container:not(:hover) .toast:not(:last-child) {
    pointer-events: none;
  }

  .container:hover .toast {
    transform: none;
    opacity: 1;
  }

  .toast.success {
    border-left-color: var(--success);
  }

  .toast.error {
    border-left-color: var(--danger);
  }

  .toast.info {
    border-left-color: var(--accent);
  }

  button {
    margin-left: auto;
    opacity: 0.5;
    font-size: 18px;
    line-height: 1;
    padding: 0;
  }

  button:hover {
    opacity: 1;
  }

  @media (max-width: 640px) {
    .container {
      right: 12px;
      left: 12px;
      bottom: 12px;
    }

    .toast {
      min-width: 0;
      width: 100%;
    }
  }
</style>
