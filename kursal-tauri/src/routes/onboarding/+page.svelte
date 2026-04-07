<script lang="ts">
  import { goto } from "$app/navigation";
  import Button from "$lib/components/Button.svelte";

  function handleStart() {
    localStorage.setItem("kursal_onboarded", "true");
    goto("/chat");
  }
</script>

<div class="container">
  <div class="content">
    <div class="hero">
      <p class="eyebrow">Kursal [PROTOTYPE!]</p>
      <h1>Private messaging without servers.</h1>
      <p class="subheading">
        DISCLAIMER: The interface is made by AI. Nothing else.<br />
        Exchange messages via a transparent and resilient network, protected by encryption
        :p
      </p>
      <Button variant="primary" onclick={handleStart}>Enter Kursal</Button>
    </div>

    <div class="feature-panel">
      <div class="feature">
        <h3>End-to-end encrypted</h3>
        <p>Built with the Signal Protocol for strong forward secrecy.</p>
      </div>
      <div class="feature">
        <h3>No central message server</h3>
        <p>
          Messages go directly between peers on the network (sometimes via
          relays).
        </p>
      </div>
      <div class="feature">
        <h3>will change this onboarding</h3>
        <p>
          with our friendly mascott that'll explain how kursal works under the
          hood
        </p>
      </div>
    </div>
  </div>
</div>

<style>
  .container {
    height: 100dvh;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 16px;
  }

  .content {
    width: min(980px, 100%);
    display: grid;
    grid-template-columns: 1.1fr 1fr;
    gap: 14px;
  }

  .hero,
  .feature-panel {
    border-radius: 18px;
    border: 1px solid var(--border);
    background: linear-gradient(
      160deg,
      rgba(30, 41, 59, 0.7),
      rgba(15, 23, 42, 0.78)
    );
    backdrop-filter: blur(12px);
    box-shadow: var(--glow);
    padding: 24px;
  }

  .hero {
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 14px;
  }

  .eyebrow {
    margin: 0;
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.14em;
    color: var(--accent-hover);
  }

  h1 {
    font-size: clamp(34px, 4vw, 48px);
    font-weight: 700;
    margin: 0;
    letter-spacing: -0.03em;
    line-height: 1.05;
  }

  .subheading {
    font-size: 16px;
    color: var(--text-secondary);
    margin: 0;
    max-width: 46ch;
  }

  .feature-panel {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .feature {
    display: grid;
    gap: 6px;
    text-align: left;
    background: rgba(15, 23, 42, 0.62);
    border: 1px solid rgba(148, 163, 184, 0.22);
    padding: 12px;
    border-radius: 10px;
  }

  .feature h3 {
    margin: 0;
    font-size: 14px;
    color: var(--text-primary);
  }

  .feature p {
    margin: 0;
    font-size: 13px;
    color: var(--text-secondary);
    line-height: 1.5;
  }

  :global(.hero .button) {
    width: fit-content;
    min-width: 160px;
    margin-top: 4px;
  }

  @media (max-width: 860px) {
    .content {
      grid-template-columns: 1fr;
      gap: 12px;
    }

    .hero,
    .feature-panel {
      padding: 20px;
    }

    :global(.hero .button) {
      width: 100%;
    }
  }
</style>
