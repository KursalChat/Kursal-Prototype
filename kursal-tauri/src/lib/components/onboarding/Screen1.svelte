<script lang="ts">
  const primaryText = "Hi. I'm Winston.";
  const subtitleText = "And I believe privacy is fundamental.";

  import Winston from "./Winston.svelte";

  type Props = {
    onNext: () => void;
    onSkip: () => void;
  };

  let { onNext, onSkip }: Props = $props();

  let showPrimary = $state(false);
  let showSubtitle = $state(false);
  let showNext = $state(false);
  let showSkip = $state(false);

  const WINSTON_LAND = 900;
  const PRIMARY_AT = WINSTON_LAND + 300;
  const SUBTITLE_AT = PRIMARY_AT + 2200;
  const NEXT_AT = SUBTITLE_AT + 1800;
  const SKIP_AT = NEXT_AT + 2500;

  $effect(() => {
    const t1 = setTimeout(() => (showPrimary = true), PRIMARY_AT);
    const t2 = setTimeout(() => (showSubtitle = true), SUBTITLE_AT);
    const t3 = setTimeout(() => (showNext = true), NEXT_AT);
    const t4 = setTimeout(() => (showSkip = true), SKIP_AT);
    return () => {
      clearTimeout(t1);
      clearTimeout(t2);
      clearTimeout(t3);
      clearTimeout(t4);
    };
  });
</script>

<div class="screen">
  <div class="stage">
    <Winston size={200} />

    <div class="text-block">
      {#if showPrimary}
        <h1 class="primary">
          {#each primaryText.split("") as ch, i}
            <span class="char" style="animation-delay: {i * 55}ms"
              >{ch === " " ? "\u00A0" : ch}</span
            >
          {/each}
        </h1>
      {/if}

      {#if showSubtitle}
        <p class="subtitle">
          {#each subtitleText.split("") as ch, i}
            <span class="char" style="animation-delay: {i * 30}ms"
              >{ch === " " ? "\u00A0" : ch}</span
            >
          {/each}
        </p>
      {/if}
    </div>
  </div>

  <div class="actions">
    <button
      class="next"
      class:show={showNext}
      onclick={onNext}
      tabindex={showNext ? 0 : -1}
      aria-hidden={!showNext}
    >
      <span>Continue</span>
      <svg
        width="18"
        height="18"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="2.5"
        stroke-linecap="round"
        stroke-linejoin="round"><path d="M5 12h14M13 6l6 6-6 6" /></svg
      >
    </button>

    <div class="skip-wrap" class:show={showSkip} aria-hidden={!showSkip}>
      <button class="skip" onclick={onSkip} tabindex={showSkip ? 0 : -1}>
        I know what I'm doing
      </button>
      <div class="tooltip">you probably don't! follow the guide :p</div>
    </div>
  </div>
</div>

<style>
  .screen {
    width: 100%;
    min-height: 100%;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 60px;
    padding: 40px;
  }

  .stage {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 36px;
  }

  .text-block {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 14px;
    min-height: 100px;
    text-align: center;
  }

  .primary {
    font-size: 36px;
    font-weight: 700;
    letter-spacing: -0.03em;
    color: #e8eeff;
    margin: 0;
  }

  .subtitle {
    font-size: 17px;
    color: rgba(200, 215, 255, 0.65);
    letter-spacing: 0.01em;
    margin: 0;
  }

  .char {
    display: inline-block;
    opacity: 0;
    transform: translateY(6px);
    animation: charIn 600ms ease-out forwards;
  }

  @keyframes charIn {
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }

  .actions {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 16px;
    min-height: 90px;
  }

  .next {
    display: inline-flex;
    align-items: center;
    gap: 10px;
    padding: 12px 26px;
    background: linear-gradient(135deg, #2e5bd7 0%, #1e50e5 100%);
    color: #fff;
    border-radius: 999px;
    font-size: 15px;
    font-weight: 600;
    letter-spacing: 0.01em;
    box-shadow:
      0 0 0 1px rgba(123, 163, 247, 0.4),
      0 0 24px rgba(46, 91, 215, 0.55),
      0 8px 24px rgba(0, 0, 0, 0.4);
    opacity: 0;
    pointer-events: none;
    transition:
      opacity 500ms ease,
      transform 150ms ease,
      box-shadow 150ms ease;
  }
  .next.show {
    opacity: 1;
    pointer-events: auto;
  }

  .next:hover {
    transform: translateY(-1px);
    box-shadow:
      0 0 0 1px rgba(123, 163, 247, 0.6),
      0 0 32px rgba(46, 91, 215, 0.75),
      0 10px 28px rgba(0, 0, 0, 0.5);
  }

  .next:active {
    transform: translateY(0);
  }

  .skip-wrap {
    position: relative;
    opacity: 0;
    pointer-events: none;
    transition: opacity 600ms ease;
  }
  .skip-wrap.show {
    opacity: 1;
    pointer-events: auto;
  }

  .skip {
    font-size: 12px;
    color: rgba(180, 195, 230, 0.4);
    letter-spacing: 0.02em;
    padding: 6px 10px;
    transition: color 150ms ease;
  }

  .skip:hover {
    color: rgba(200, 215, 255, 0.75);
  }

  .tooltip {
    position: absolute;
    bottom: calc(100% + 8px);
    left: 50%;
    transform: translateX(-50%) translateY(4px);
    background: rgba(20, 26, 50, 0.95);
    border: 1px solid rgba(123, 163, 247, 0.25);
    color: #e8eeff;
    padding: 8px 12px;
    border-radius: 8px;
    font-size: 12px;
    white-space: nowrap;
    opacity: 0;
    pointer-events: none;
    transition:
      opacity 180ms ease,
      transform 180ms ease;
    box-shadow: 0 0 20px rgba(46, 91, 215, 0.3);
  }

  .tooltip::after {
    content: "";
    position: absolute;
    top: 100%;
    left: 50%;
    transform: translateX(-50%);
    border: 5px solid transparent;
    border-top-color: rgba(20, 26, 50, 0.95);
  }

  .skip-wrap:hover .tooltip {
    opacity: 1;
    transform: translateX(-50%) translateY(0);
  }

  @media (max-width: 640px) {
    .screen {
      gap: 36px;
      padding: 24px 20px;
    }
    .primary {
      font-size: 28px;
    }
    .subtitle {
      font-size: 15px;
    }
    .actions {
      min-height: 80px;
    }
  }
</style>
