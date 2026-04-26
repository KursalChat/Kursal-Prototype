<script lang="ts">
  import { AlertTriangle, Info, ShieldAlert } from "lucide-svelte";
  import { confirmState } from "$lib/state/confirm.svelte";
  import Button from "./Button.svelte";

  let holdProgress = $state(1); // 0..1, starts at 0 if holdMs set, else 1 (unlocked)
  let holdRaf = 0;

  const holdMs = $derived(confirmState.options?.holdMs ?? 0);
  const locked = $derived(holdMs > 0 && holdProgress < 1);

  function onKey(e: KeyboardEvent) {
    if (!confirmState.open) return;
    if (e.key === "Escape") {
      e.preventDefault();
      confirmState.cancel();
    } else if (e.key === "Enter") {
      e.preventDefault();
      if (!locked) confirmState.confirm();
    }
  }

  $effect(() => {
    if (!confirmState.open) return;
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  $effect(() => {
    if (!confirmState.open) {
      cancelAnimationFrame(holdRaf);
      holdProgress = 1;
      return;
    }
    if (holdMs <= 0) {
      holdProgress = 1;
      return;
    }
    holdProgress = 0;
    const start = performance.now();
    const tick = (now: number) => {
      const p = Math.min(1, (now - start) / holdMs);
      holdProgress = p;
      if (p < 1) holdRaf = requestAnimationFrame(tick);
    };
    holdRaf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(holdRaf);
  });
</script>

{#if confirmState.open && confirmState.options}
  {@const o = confirmState.options}
  {@const tone = o.tone ?? "default"}
  <div
    class="backdrop"
    onclick={() => confirmState.cancel()}
    role="presentation"
  ></div>
  <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="confirm-title">
    <div class="head" data-tone={tone}>
      <div class="icon-wrap" data-tone={tone}>
        {#if tone === "danger"}
          <ShieldAlert size={18} />
        {:else if tone === "warning"}
          <AlertTriangle size={18} />
        {:else}
          <Info size={18} />
        {/if}
      </div>
      <h3 id="confirm-title">{o.title}</h3>
    </div>
    <div class="body">
      {#if o.message}
        <p class="message">{o.message}</p>
      {/if}
      {#if o.detail}
        <p class="detail">{o.detail}</p>
      {/if}
    </div>
    <div class="foot">
      <Button variant="secondary" onclick={() => confirmState.cancel()}>
        {o.cancelLabel ?? "Cancel"}
      </Button>
      {#if holdMs > 0}
        <button
          class="hold-btn"
          data-tone={tone}
          disabled={locked}
          onclick={() => confirmState.confirm()}
          aria-busy={locked}
        >
          <span
            class="hold-fill"
            style="transform: scaleX({holdProgress});"
            aria-hidden="true"
          ></span>
          <span class="hold-label">{o.confirmLabel ?? "Confirm"}</span>
        </button>
      {:else}
        <Button
          variant={tone === "danger" ? "danger" : "primary"}
          onclick={() => confirmState.confirm()}
        >
          {o.confirmLabel ?? "Confirm"}
        </Button>
      {/if}
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.55);
    z-index: 1000;
    animation: fadein 150ms ease;
  }
  .dialog {
    position: fixed;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    width: calc(100% - 32px);
    max-width: 420px;
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    box-shadow: 0 24px 64px rgba(0, 0, 0, 0.5);
    z-index: 1001;
    overflow: hidden;
    animation: pop 180ms cubic-bezier(0.2, 0.9, 0.3, 1.2);
  }
  .head {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 18px 20px 14px;
  }
  .head h3 {
    margin: 0;
    font-size: 16px;
    font-weight: 700;
    color: var(--text-primary);
  }
  .icon-wrap {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 34px;
    height: 34px;
    border-radius: 10px;
    flex-shrink: 0;
  }
  .icon-wrap[data-tone="default"] {
    background: var(--accent-dim);
    color: var(--accent);
  }
  .icon-wrap[data-tone="warning"] {
    background: rgba(251, 191, 36, 0.15);
    color: var(--warning);
  }
  .icon-wrap[data-tone="danger"] {
    background: var(--danger-dim);
    color: var(--danger);
  }
  .body {
    padding: 0 20px 18px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .message {
    margin: 0;
    font-size: 14px;
    color: var(--text-primary);
    line-height: 1.55;
  }
  .detail {
    margin: 0;
    font-size: 13px;
    color: var(--text-secondary);
    line-height: 1.55;
  }
  .foot {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding: 14px 20px 18px;
    border-top: 1px solid var(--border-light);
    background: var(--surface-soft);
  }
  @keyframes fadein {
    from { opacity: 0; }
    to { opacity: 1; }
  }
  @keyframes pop {
    from { opacity: 0; transform: translate(-50%, -46%) scale(0.96); }
    to { opacity: 1; transform: translate(-50%, -50%) scale(1); }
  }

  .hold-btn {
    position: relative;
    overflow: hidden;
    min-height: 36px;
    padding: 8px 14px;
    border-radius: var(--radius-md);
    font-size: 13px;
    font-weight: 600;
    border: 1px solid transparent;
    background: var(--accent-solid);
    color: #fff;
    cursor: pointer;
    isolation: isolate;
    transition: opacity var(--transition);
  }
  .hold-btn[data-tone="danger"] {
    background: var(--danger);
    border-color: var(--danger);
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.15);
  }
  .hold-btn:disabled {
    cursor: not-allowed;
    opacity: 0.85;
  }
  .hold-btn:not(:disabled):hover {
    background: var(--danger-hover, color-mix(in srgb, var(--accent-solid), white 10%));
  }
  .hold-fill {
    position: absolute;
    inset: 0;
    background: rgba(255, 255, 255, 0.22);
    transform-origin: left center;
    transform: scaleX(0);
    z-index: 0;
    pointer-events: none;
  }
  .hold-label {
    position: relative;
    z-index: 1;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-variant-numeric: tabular-nums;
  }
</style>
