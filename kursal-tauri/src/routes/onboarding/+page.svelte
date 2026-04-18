<script lang="ts">
  import { goto } from "$app/navigation";
  import Screen1 from "$lib/components/onboarding/Screen1.svelte";
  import Screen2 from "$lib/components/onboarding/Screen2.svelte";
  import Screen3 from "$lib/components/onboarding/Screen3.svelte";
  import Screen4 from "$lib/components/onboarding/Screen4.svelte";
  import Screen5 from "$lib/components/onboarding/Screen5.svelte";

  let screen = $state(1);

  function finish() {
    localStorage.setItem("kursal_onboarded", "done");
    goto("/chat");
  }

  function goToLast() {
    screen = 5;
  }
</script>

<div class="onboarding" class:bright={screen === 5}>
  <div class="grain"></div>
  <div class="glow-bg"></div>

  <div class="stage">
    {#if screen === 1}
      <div class="screen-wrap" data-key="1">
        <Screen1 onNext={() => (screen = 2)} onSkip={goToLast} />
      </div>
    {:else if screen === 2}
      <div class="screen-wrap" data-key="2">
        <Screen2 onNext={() => (screen = 3)} />
      </div>
    {:else if screen === 3}
      <div class="screen-wrap" data-key="3">
        <Screen3 onNext={() => (screen = 4)} />
      </div>
    {:else if screen === 4}
      <div class="screen-wrap" data-key="4">
        <Screen4 onNext={() => (screen = 5)} />
      </div>
    {:else if screen === 5}
      <div class="screen-wrap" data-key="5">
        <Screen5 onFinish={finish} />
      </div>
    {/if}
  </div>
</div>

<style>
  .onboarding {
    position: fixed;
    inset: 0;
    background: radial-gradient(
        ellipse at 20% 15%,
        rgba(46, 91, 215, 0.18) 0%,
        transparent 45%
      ),
      radial-gradient(
        ellipse at 85% 85%,
        rgba(30, 80, 229, 0.12) 0%,
        transparent 50%
      ),
      #05070f;
    overflow-y: auto;
    overflow-x: hidden;
    display: flex;
    align-items: stretch;
    justify-content: center;
    transition: background 1200ms ease;
    -webkit-overflow-scrolling: touch;
    padding-top: env(safe-area-inset-top, 0);
    padding-bottom: env(safe-area-inset-bottom, 0);
  }

  .onboarding.bright {
    background: radial-gradient(
        ellipse at 50% 30%,
        rgba(123, 163, 247, 0.28) 0%,
        transparent 55%
      ),
      radial-gradient(
        ellipse at 50% 90%,
        rgba(46, 91, 215, 0.2) 0%,
        transparent 60%
      ),
      #0a1026;
  }

  .grain {
    position: absolute;
    inset: 0;
    pointer-events: none;
    opacity: 0.05;
    mix-blend-mode: overlay;
    background-image: url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='200' height='200'><filter id='n'><feTurbulence type='fractalNoise' baseFrequency='0.9' numOctaves='2' stitchTiles='stitch'/><feColorMatrix values='0 0 0 0 1  0 0 0 0 1  0 0 0 0 1  0 0 0 1 0'/></filter><rect width='100%' height='100%' filter='url(%23n)'/></svg>");
  }

  .glow-bg {
    position: absolute;
    inset: 0;
    pointer-events: none;
    background: radial-gradient(
      circle at 50% 50%,
      rgba(46, 91, 215, 0.08) 0%,
      transparent 60%
    );
  }

  .stage {
    position: relative;
    z-index: 1;
    width: 100%;
    min-height: 100%;
    display: flex;
  }

  .screen-wrap {
    width: 100%;
    min-height: 100%;
    display: flex;
    flex-direction: column;
    animation: screenFade 500ms ease-out;
  }

  .screen-wrap > :global(*) {
    flex: 1;
    min-height: 0;
  }

  @keyframes screenFade {
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }
</style>
