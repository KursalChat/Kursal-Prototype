<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { writeText } from "@tauri-apps/plugin-clipboard-manager";
  import {
    runOtpBenchmark,
    cancelBenchmark,
    isBenchmarkRunning,
    type BenchmarkProgress,
    type BenchmarkResult,
  } from "$lib/api/benchmark";
  import { notifications } from "$lib/state/notifications.svelte";
  import Button from "$lib/components/Button.svelte";
  import { Copy } from "lucide-svelte";

  let {
    name = "System Benchmark",
    description = "Run cryptographic operations to benchmark device performance.",
  } = $props<{ name?: string; description?: string }>();

  let iterations = $state(100);
  let isRunning = $state(false);
  let progress = $state<BenchmarkProgress | null>(null);
  let result = $state<BenchmarkResult | null>(null);
  let error = $state<string | null>(null);
  let unlisten: () => void;

  onMount(async () => {
    isRunning = await isBenchmarkRunning();

    const unlistenFn = await listen<BenchmarkProgress>(
      "benchmark-progress",
      (event) => {
        progress = event.payload;
      },
    );
    unlisten = unlistenFn;
  });

  onDestroy(() => {
    if (unlisten) unlisten();
  });

  async function start() {
    if (isRunning) return;

    isRunning = true;
    progress = null;
    result = null;
    error = null;

    try {
      result = await runOtpBenchmark(iterations);
    } catch (err: any) {
      error = err.toString();
    } finally {
      isRunning = false;
    }
  }

  async function cancel() {
    if (!isRunning) return;
    try {
      await cancelBenchmark();
    } catch (err: any) {
      console.error("Cancel failed:", err);
    }
  }

  async function copyResults() {
    if (!result) return;

    const text =
      `Benchmark Results (${name})\n` +
      `Iterations: ${result.iterations}\n` +
      `Total Time: ${(result.total_ms / 1000).toFixed(2)}s\n` +
      `Time per Iter (Threaded): ${result.average_with_threading_ms.toFixed(2)}ms\n` +
      `Time per Iter (CPU): ${result.average_per_iteration_ms.toFixed(2)}ms\n` +
      `Iterations / Sec: ${result.iterations_per_second.toFixed(2)} iter/sec`;

    try {
      if (navigator.clipboard) {
        await navigator.clipboard.writeText(text);
        notifications.push("Benchmark results copied to clipboard!", "success");
      } else {
        notifications.push("Clipboard API not available", "error");
      }
    } catch (err) {
      console.error("Failed to copy:", err);
      notifications.push("Failed to copy to clipboard", "error");
    }
  }
</script>

<div class="info-card">
  <div
    style="display: flex; justify-content: space-between; align-items: start; margin-bottom: 0.5rem;"
  >
    <h4 style="margin: 0; color: var(--color-primary);">{name}</h4>
  </div>

  <p
    style="margin-top: 0; margin-bottom: 1rem; color: var(--text-secondary); font-size: 0.9rem;"
  >
    {description}
  </p>

  <div
    class="benchmark-controls"
    style="display: flex; align-items: flex-end; gap: 1rem; margin-bottom: 1.5rem; flex-wrap: wrap;"
  >
    <div class="form-group" style="margin-bottom: 0; width: 120px;">
      <label
        for="iterations"
        style="font-size: 0.85rem; margin-bottom: 0.25rem; display: block;"
        >Iterations</label
      >
      <input
        id="iterations"
        type="number"
        bind:value={iterations}
        min="1"
        max="10000"
        disabled={isRunning}
        style="width: 100%; padding: 0.4rem 0.5rem; font-size: 0.95rem; background: var(--bg-surface, rgba(0,0,0,0.2)); border: 1px solid var(--border-color, rgba(255,255,255,0.1)); color: var(--text-primary); border-radius: 6px;"
      />
    </div>

    <div style="display: flex; gap: 0.5rem;">
      <Button onclick={start} loading={isRunning}>
        {isRunning ? "Running..." : "Run Benchmark"}
      </Button>
      {#if isRunning}
        <Button variant="danger" onclick={cancel}>Cancel</Button>
      {/if}
    </div>
  </div>

  {#if error}
    <div style="color: var(--color-danger); margin-bottom: 1rem;">
      Error: {error}
    </div>
  {/if}

  {#if progress && isRunning}
    <div
      style="margin-top: 1rem; padding: 1rem; background: rgba(0, 0, 0, 0.15); border-radius: 8px;"
    >
      <div
        style="display: flex; justify-content: space-between; font-size: 0.85rem; margin-bottom: 0.5rem; color: #a1a1aa;"
      >
        <span>{progress.current} / {progress.total} hashes completed</span>
        <span>{(progress.elapsed_ms / 1000).toFixed(1)}s elapsed</span>
      </div>
      <progress
        value={progress.current}
        max={progress.total}
        style="width: 100%; height: 8px; border-radius: 4px; overflow: hidden;"
      ></progress>
    </div>
  {/if}

  {#if result}
    <div
      style="background: rgba(0, 0, 0, 0.15); padding: 1.25rem; border-radius: 8px; border: 1px solid rgba(255, 255, 255, 0.1); margin-top: 1rem;"
    >
      <div
        style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 1rem;"
      >
        <h4
          style="margin: 0; font-size: 1rem; color: #e2e8f0; display: flex; align-items: center; gap: 0.5rem;"
        >
          <span
            style="display: inline-block; width: 8px; height: 8px; background-color: #10b981; border-radius: 50%;"
          ></span>
          Benchmark Results
        </h4>
        <button
          class="copy-btn"
          onclick={copyResults}
          aria-label="Copy Results"
          title="Copy Results"
        >
          <Copy size={14} />
        </button>
      </div>
      <div
        style="display: grid; grid-template-columns: repeat(auto-fit, minmax(180px, 1fr)); gap: 1rem;"
      >
        <div>
          <div
            style="font-size: 0.75rem; color: #94a3b8; margin-bottom: 0.25rem;"
          >
            Iterations
          </div>
          <div
            style="font-size: 1.1rem; font-family: monospace; font-weight: 500;"
          >
            {result.iterations}
          </div>
        </div>
        <div>
          <div
            style="font-size: 0.75rem; color: #94a3b8; margin-bottom: 0.25rem;"
          >
            Total Time
          </div>
          <div
            style="font-size: 1.1rem; font-family: monospace; font-weight: 500;"
          >
            {(result.total_ms / 1000).toFixed(2)}<span
              style="font-size: 0.8rem; color: #64748b;">s</span
            >
          </div>
        </div>
        <div>
          <div
            style="font-size: 0.75rem; color: #94a3b8; margin-bottom: 0.25rem;"
          >
            Time per Iter (Threaded)
          </div>
          <div
            style="font-size: 1.1rem; font-family: monospace; font-weight: 500;"
          >
            {result.average_with_threading_ms.toFixed(2)}<span
              style="font-size: 0.8rem; color: #64748b;">ms</span
            >
          </div>
        </div>
        <div>
          <div
            style="font-size: 0.75rem; color: #94a3b8; margin-bottom: 0.25rem;"
          >
            Time per Iter (CPU)
          </div>
          <div
            style="font-size: 1.1rem; font-family: monospace; font-weight: 500;"
          >
            {result.average_per_iteration_ms.toFixed(2)}<span
              style="font-size: 0.8rem; color: #64748b;">ms</span
            >
          </div>
        </div>
        <div>
          <div
            style="font-size: 0.75rem; color: #94a3b8; margin-bottom: 0.25rem;"
          >
            Iterations / Sec
          </div>
          <div
            style="font-size: 1.1rem; font-family: monospace; font-weight: 500;"
          >
            {result.iterations_per_second.toFixed(2)}<span
              style="font-size: 0.8rem; color: #64748b;"
            >
              iter/sec</span
            >
          </div>
        </div>
      </div>
    </div>
  {/if}
</div>

<style>
  .copy-btn {
    background: transparent;
    border: none;
    font-size: 0.85rem;
    color: #94a3b8;
    cursor: pointer;
    display: flex;
    align-items: center;
    gap: 0.25rem;
    padding: 0.25rem 0.5rem;
    border-radius: 4px;
    transition:
      color 0.1s ease,
      background-color 0.1s ease;
  }

  .copy-btn:hover {
    color: #f8fafc;
    background-color: rgba(255, 255, 255, 0.05);
  }
</style>
