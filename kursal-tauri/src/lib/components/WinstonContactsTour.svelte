<script lang="ts">
  const stepData: Record<Step, StepData> = {
    intro: {
      img: "/winston-warm.png",
      title: "Hey! I'm Winston",
      body: "I see... it's your first time on Kursal! Let me show you how adding contacts works.",
      cta: "Show me",
    },
    "click-add": {
      img: "/winston.png",
      body: "First, hit the **Add Contact** button. There are three ways to add a friend, and I'll walk you through each one.",
    },
    "otp-detail": {
      img: "/winston-key.png",
      title: "One-Time Code",
      body: "You or your friend generates a one-off password: eight random words. The other types it in. Single-use, great for sharing via voice or text.",
      cta: "Got it",
    },
    "click-ltc": {
      img: "/winston.png",
      body: "Now check out **Long-Term Codes**. Click that tab.",
    },
    "ltc-detail": {
      img: "/winston-key.png",
      title: "Long-Term Code",
      body: "You generate a file that multiple people can import to add you. Only one is valid at a time. Auto-expires after 7 days, or revoke it whenever you want.",
      cta: "Next",
    },
    "click-nearby": {
      img: "/winston.png",
      body: "Last one. **Nearby**. This one feels magical. Click it!",
    },
    "nearby-detail": {
      img: "/winston-smug.png",
      title: "Nearby Share",
      body: "If you and your friend are on the same Wi-Fi, or close enough for Bluetooth, you'll just *appear* on each other's screens. As long as you're both on this page.",
      cta: "Great",
    },
    outro: {
      img: "/winston-warm.png",
      title: "You're all set!",
      body: "Pick whichever method fits the moment. Welcome to Kursal, hope you stick around.",
      cta: "Let's go",
    },
  };

  import { onMount, tick } from "svelte";
  import { page } from "$app/state";
  import { fly, fade, scale } from "svelte/transition";
  import { backOut } from "svelte/easing";

  type Step =
    | "intro"
    | "click-add"
    | "otp-detail"
    | "click-ltc"
    | "ltc-detail"
    | "click-nearby"
    | "nearby-detail"
    | "outro";

  const KEY = "kursal_addcontact_onboarded";

  let active = $state(false);
  let step = $state<Step>("intro");
  let rect = $state<{ x: number; y: number; w: number; h: number } | null>(
    null,
  );
  let skipReady = $state(false);

  onMount(() => {
    if (localStorage.getItem(KEY) === "done") return;
    const t = setTimeout(() => {
      active = true;
    }, 700);
    const s = setTimeout(() => {
      skipReady = true;
    }, 5700);
    return () => {
      clearTimeout(t);
      clearTimeout(s);
    };
  });

  function finish() {
    localStorage.setItem(KEY, "done");
    active = false;
  }

  function targetEl(): HTMLElement | null {
    if (step === "click-add") {
      return (document.querySelector('[data-tour="add-contact-empty"]') ??
        document.querySelector(
          '[data-tour="add-contact-btn"]',
        )) as HTMLElement | null;
    }
    if (step === "click-ltc")
      return document.querySelector(
        '[data-tour="ltc-tab"]',
      ) as HTMLElement | null;
    if (step === "click-nearby")
      return document.querySelector(
        '[data-tour="nearby-tab"]',
      ) as HTMLElement | null;
    return null;
  }

  function recompute() {
    const el = targetEl();
    if (!el) {
      rect = null;
      return;
    }
    const r = el.getBoundingClientRect();
    rect = { x: r.left, y: r.top, w: r.width, h: r.height };
  }

  // Recompute on step change, route change, or resize.
  $effect(() => {
    if (!active) return;
    void step;
    void page.url.pathname;
    void tick().then(() => requestAnimationFrame(recompute));
  });

  $effect(() => {
    if (!active) return;
    const onR = () => recompute();
    window.addEventListener("resize", onR);
    window.addEventListener("scroll", onR, true);
    const id = window.setInterval(recompute, 400);
    return () => {
      window.removeEventListener("resize", onR);
      window.removeEventListener("scroll", onR, true);
      clearInterval(id);
    };
  });

  // Auto-advance when user navigates to the target page.
  $effect(() => {
    if (!active) return;
    const p = page.url.pathname;
    if (step === "click-add" && p.startsWith("/add-contact")) {
      step = "otp-detail";
    } else if (step === "click-ltc" && p.startsWith("/add-contact/ltc")) {
      step = "ltc-detail";
    } else if (step === "click-nearby" && p.startsWith("/add-contact/nearby")) {
      step = "nearby-detail";
    }
  });

  type StepData = {
    img: string;
    title?: string;
    body: string;
    cta?: string;
  };

  function next() {
    if (step === "intro") step = "click-add";
    else if (step === "otp-detail") step = "click-ltc";
    else if (step === "ltc-detail") step = "click-nearby";
    else if (step === "nearby-detail") step = "outro";
    else if (step === "outro") finish();
  }

  function escapeHtml(s: string) {
    return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
  }
  function formatBody(s: string) {
    return escapeHtml(s)
      .replace(/\*\*(.+?)\*\*/g, "<strong>$1</strong>")
      .replace(/\*(.+?)\*/g, "<em>$1</em>");
  }

  const data = $derived(stepData[step]);
  const showSpotlight = $derived(
    step === "click-add" || step === "click-ltc" || step === "click-nearby",
  );
</script>

{#if active}
  <div class="tour" role="dialog" aria-label="Kursal tour">
    {#if showSpotlight && rect}
      <!-- 4-quadrant dim around the target — clicks pass through the cutout -->
      <div
        class="dim"
        style:top="0"
        style:left="0"
        style:right="0"
        style:height="{Math.max(0, rect.y - 8)}px"
        transition:fade={{ duration: 200 }}
      ></div>
      <div
        class="dim"
        style:top="{Math.max(0, rect.y - 8)}px"
        style:left="0"
        style:width="{Math.max(0, rect.x - 8)}px"
        style:height="{rect.h + 16}px"
        transition:fade={{ duration: 200 }}
      ></div>
      <div
        class="dim"
        style:top="{Math.max(0, rect.y - 8)}px"
        style:left="{rect.x + rect.w + 8}px"
        style:right="0"
        style:height="{rect.h + 16}px"
        transition:fade={{ duration: 200 }}
      ></div>
      <div
        class="dim"
        style:top="{rect.y + rect.h + 8}px"
        style:left="0"
        style:right="0"
        style:bottom="0"
        transition:fade={{ duration: 200 }}
      ></div>

      <div
        class="ring"
        style:left="{rect.x - 8}px"
        style:top="{rect.y - 8}px"
        style:width="{rect.w + 16}px"
        style:height="{rect.h + 16}px"
      ></div>
    {:else if !showSpotlight}
      <div class="dim full" transition:fade={{ duration: 250 }}></div>
    {/if}

    {#key step}
      <div
        class="card"
        in:fly={{ y: 30, duration: 380, easing: backOut }}
        out:fade={{ duration: 150 }}
      >
        <div class="winston-wrap">
          <img
            src={data.img}
            alt="Winston"
            class="winston"
            in:scale={{ duration: 420, start: 0.6, easing: backOut }}
          />
          <div class="winston-shadow"></div>
        </div>

        <div class="bubble">
          <div class="bubble-tail"></div>
          {#if data.title}
            <div class="title">{data.title}</div>
          {/if}
          <div class="body">{@html formatBody(data.body)}</div>

          <div class="actions">
            {#if step === "intro" && skipReady}
              <button class="skip" onclick={finish} in:fade={{ duration: 240 }}>
                Skip tour
              </button>
            {:else}
              <span class="skip-placeholder"></span>
            {/if}
            {#if data.cta}
              <button class="primary" onclick={next}>
                {data.cta}
              </button>
            {/if}
          </div>

          <div class="progress">
            {#each ["intro", "click-add", "otp-detail", "click-ltc", "ltc-detail", "click-nearby", "nearby-detail", "outro"] as s, i}
              <span
                class="dot"
                class:done={Object.keys(stepData).indexOf(step) >= i}
              ></span>
            {/each}
          </div>
        </div>
      </div>
    {/key}
  </div>
{/if}

<style>
  .tour {
    position: fixed;
    inset: 0;
    z-index: 9000;
    pointer-events: none;
  }

  .dim {
    position: fixed;
    background: rgba(2, 6, 23, 0.62);
    backdrop-filter: blur(2px);
    -webkit-backdrop-filter: blur(2px);
    pointer-events: auto;
    transition:
      top 0.32s cubic-bezier(0.4, 0, 0.2, 1),
      left 0.32s cubic-bezier(0.4, 0, 0.2, 1),
      width 0.32s cubic-bezier(0.4, 0, 0.2, 1),
      height 0.32s cubic-bezier(0.4, 0, 0.2, 1);
  }
  .dim.full {
    inset: 0;
  }

  .ring {
    position: fixed;
    border-radius: 20px;
    border: 2px solid var(--accent);
    box-shadow:
      0 0 0 4px var(--accent-dim),
      0 0 32px 4px color-mix(in srgb, var(--accent) 60%, transparent),
      inset 0 0 0 1px rgba(255, 255, 255, 0.4);
    pointer-events: none;
    animation: ring-pulse 1.6s ease-in-out infinite;
    transition:
      top 0.32s cubic-bezier(0.4, 0, 0.2, 1),
      left 0.32s cubic-bezier(0.4, 0, 0.2, 1),
      width 0.32s cubic-bezier(0.4, 0, 0.2, 1),
      height 0.32s cubic-bezier(0.4, 0, 0.2, 1);
  }

  @keyframes ring-pulse {
    0%,
    100% {
      box-shadow:
        0 0 0 4px var(--accent-dim),
        0 0 24px 2px color-mix(in srgb, var(--accent) 50%, transparent),
        inset 0 0 0 1px rgba(255, 255, 255, 0.35);
      transform: scale(1);
    }
    50% {
      box-shadow:
        0 0 0 8px color-mix(in srgb, var(--accent) 18%, transparent),
        0 0 40px 8px color-mix(in srgb, var(--accent) 65%, transparent),
        inset 0 0 0 1px rgba(255, 255, 255, 0.5);
      transform: scale(1.03);
    }
  }

  .card {
    position: fixed;
    right: 22px;
    bottom: 22px;
    display: flex;
    align-items: flex-end;
    gap: 12px;
    pointer-events: auto;
    max-width: min(440px, calc(100vw - 44px));
    z-index: 1;
  }

  .winston-wrap {
    position: relative;
    flex-shrink: 0;
    animation: float 3.4s ease-in-out infinite;
  }
  @keyframes float {
    0%,
    100% {
      transform: translateY(0);
    }
    50% {
      transform: translateY(-6px);
    }
  }
  .winston {
    width: 110px;
    height: 110px;
    object-fit: contain;
    filter: drop-shadow(0 12px 24px rgba(0, 0, 0, 0.45));
    user-select: none;
    -webkit-user-drag: none;
  }
  .winston-shadow {
    position: absolute;
    bottom: -4px;
    left: 50%;
    transform: translateX(-50%);
    width: 70px;
    height: 10px;
    background: radial-gradient(
      ellipse at center,
      rgba(0, 0, 0, 0.35) 0%,
      transparent 70%
    );
    border-radius: 50%;
    animation: shadow-pulse 3.4s ease-in-out infinite;
    pointer-events: none;
  }
  @keyframes shadow-pulse {
    0%,
    100% {
      transform: translateX(-50%) scaleX(1);
      opacity: 0.7;
    }
    50% {
      transform: translateX(-50%) scaleX(0.78);
      opacity: 0.5;
    }
  }

  .bubble {
    position: relative;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 18px;
    padding: 16px 18px 14px;
    box-shadow:
      var(--glow),
      0 0 0 1px rgba(255, 255, 255, 0.04) inset;
    backdrop-filter: blur(20px);
    -webkit-backdrop-filter: blur(20px);
    flex: 1;
    min-width: 0;
  }
  .bubble-tail {
    position: absolute;
    left: -7px;
    bottom: 26px;
    width: 14px;
    height: 14px;
    background: var(--surface);
    border-left: 1px solid var(--border);
    border-bottom: 1px solid var(--border);
    transform: rotate(45deg);
  }

  .title {
    font-size: 15px;
    font-weight: 700;
    color: var(--text-primary);
    margin-bottom: 6px;
    letter-spacing: -0.01em;
  }
  .body {
    font-size: 13.5px;
    line-height: 1.55;
    color: var(--text-secondary);
  }
  .body :global(strong) {
    color: var(--text-primary);
    font-weight: 700;
  }

  .skip-placeholder {
    flex: 1;
  }

  .actions {
    margin-top: 14px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
  }
  .skip {
    font-size: 12px;
    font-weight: 600;
    color: var(--text-muted);
    padding: 6px 4px;
    transition: color var(--transition);
  }
  .skip:hover {
    color: var(--text-secondary);
  }
  .primary {
    padding: 8px 16px;
    border-radius: 999px;
    background: var(--accent-solid);
    color: #fff;
    font-size: 13px;
    font-weight: 700;
    transition:
      transform var(--transition),
      box-shadow var(--transition),
      background var(--transition);
    box-shadow: 0 4px 14px var(--accent-dim);
  }
  .primary:hover {
    background: var(--accent-hover);
    transform: translateY(-1px);
    box-shadow: 0 8px 22px var(--accent-dim);
  }
  .primary:active {
    transform: translateY(0);
  }

  .progress {
    margin-top: 10px;
    display: flex;
    gap: 4px;
    justify-content: center;
  }
  .dot {
    width: 14px;
    height: 4px;
    border-radius: 2px;
    background: var(--border);
    transition: background var(--transition);
  }
  .dot.done {
    background: var(--accent);
  }

  @media (max-width: 768px) {
    .card {
      right: 12px;
      bottom: 12px;
      left: 12px;
      max-width: none;
      align-items: flex-end;
    }
    .winston {
      width: 84px;
      height: 84px;
    }
    .bubble {
      padding: 14px 14px 12px;
    }
  }

  @media (max-width: 480px) {
    .card {
      flex-direction: column;
      align-items: center;
      gap: 0;
    }
    .bubble-tail {
      display: none;
    }
    .bubble {
      width: 100%;
    }
    .winston {
      width: 96px;
      height: 96px;
      margin-bottom: -10px;
    }
  }
</style>
