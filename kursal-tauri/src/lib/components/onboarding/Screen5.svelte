<script lang="ts">
  const LINE_1 = "This is what private actually looks like.";
  const LINE_2 = "Let's make it yours.";
  const BUTTON_LABEL = "Let's go";

  import Winston from "./Winston.svelte";
  import Avatar from "$lib/components/Avatar.svelte";
  import AvatarPicker from "$lib/components/AvatarPicker.svelte";
  import { Upload, X } from "lucide-svelte";
  import { broadcastProfile } from "$lib/api/identity";
  import { profileState } from "$lib/state/profile.svelte";
  import { notifications } from "$lib/state/notifications.svelte";

  const EXIT_MS = 2000;
  const SPARK_COUNT = 26;

  const T = {
    scene: 150,
    winston: 550,
    line1: 1300,
    line2: 2600,
    form: 3500,
  };

  type Props = {
    onFinish: () => void;
    onBack?: () => void;
  };

  let { onFinish }: Props = $props();

  let sceneIn = $state(false);
  let winstonIn = $state(false);
  let showLine1 = $state(false);
  let showLine2 = $state(false);
  let showForm = $state(false);
  let exiting = $state(false);
  let saving = $state(false);

  let displayName = $state("");
  let avatarBase64 = $state<string | null>(null);
  let avatarBytes = $state<number[] | null>(null);
  let nameInput = $state<HTMLInputElement>();

  $effect(() => {
    const timers = [
      setTimeout(() => (sceneIn = true), T.scene),
      setTimeout(() => (winstonIn = true), T.winston),
      setTimeout(() => (showLine1 = true), T.line1),
      setTimeout(() => (showLine2 = true), T.line2),
      setTimeout(() => {
        showForm = true;
        setTimeout(() => nameInput?.focus(), 500);
      }, T.form),
    ];
    return () => timers.forEach(clearTimeout);
  });

  function chars(text: string, step = 28) {
    return text.split("").map((ch, i) => ({ ch, delay: i * step }));
  }

  function handleAvatarChange(b64: string, bytes: number[]) {
    avatarBase64 = b64;
    avatarBytes = bytes;
  }

  function removeAvatar() {
    avatarBase64 = null;
    avatarBytes = null;
  }

  async function handleFinish() {
    if (exiting || saving) return;
    const name = displayName.trim();
    if (!name) {
      notifications.push("Pick a name first", "error");
      nameInput?.focus();
      return;
    }
    saving = true;
    try {
      await broadcastProfile(name, avatarBytes);
      profileState.update(name, avatarBase64, avatarBytes);
    } catch (e) {
      console.error("Profile save failed", e);
      notifications.push("Saved locally but broadcast failed", "error");
    }
    saving = false;
    exiting = true;
    setTimeout(onFinish, EXIT_MS);
  }

  function handleKey(e: KeyboardEvent) {
    if (e.key === "Enter") handleFinish();
  }

  const sparkles = Array.from({ length: SPARK_COUNT }, (_, i) => ({
    id: i,
    left: Math.random() * 100,
    top: Math.random() * 100,
    delay: Math.random() * 6,
    size: 2 + Math.random() * 3,
    duration: 5 + Math.random() * 5,
  }));
</script>

<div class="screen" class:in={sceneIn} class:exiting>
  <div class="aurora"></div>

  <div class="sparkles" aria-hidden="true">
    {#each sparkles as s (s.id)}
      <span
        class="spark"
        style="left: {s.left}%; top: {s.top}%; width: {s.size}px; height: {s.size}px; animation-delay: {s.delay}s; animation-duration: {s.duration}s;"
      ></span>
    {/each}
  </div>

  <div class="stage">
    <div class="winston-wrap" class:show={winstonIn}>
      <div class="aura aura-1"></div>
      <div class="aura aura-2"></div>
      <div class="aura aura-3"></div>
      {#if winstonIn}
        <Winston size={180} src="/winston-warm.png" />
      {/if}
    </div>

    <div class="text-block">
      <div class="line line-1" class:show={showLine1}>
        {#if showLine1}
          {#each chars(LINE_1) as { ch, delay }}
            <span class="char" style="animation-delay: {delay}ms"
              >{ch === " " ? "\u00A0" : ch}</span
            >
          {/each}
        {/if}
      </div>

      <div class="line line-2" class:show={showLine2}>
        {#if showLine2}
          {#each chars(LINE_2) as { ch, delay }}
            <span class="char" style="animation-delay: {delay}ms"
              >{ch === " " ? "\u00A0" : ch}</span
            >
          {/each}
        {/if}
      </div>
    </div>

    <div class="form" class:show={showForm} aria-hidden={!showForm}>
      <AvatarPicker onChange={handleAvatarChange}>
        {#snippet children(open)}
          <div class="avatar-slot-wrap">
            <button
              type="button"
              class="avatar-slot"
              title="Upload a photo"
              aria-label="Upload a photo"
              tabindex={showForm ? 0 : -1}
              onclick={open}
            >
              <Avatar
                name={displayName.trim() || "?"}
                src={avatarBase64}
                size={88}
              />
              <div class="avatar-overlay" class:filled={!!avatarBase64}>
                <Upload size={18} strokeWidth={2.2} />
              </div>
            </button>
            {#if avatarBase64}
              <button
                type="button"
                class="avatar-remove"
                aria-label="Remove photo"
                tabindex={showForm ? 0 : -1}
                onclick={removeAvatar}
              >
                <X size={12} strokeWidth={2.6} />
              </button>
            {/if}
          </div>
        {/snippet}
      </AvatarPicker>

      <input
        bind:this={nameInput}
        class="name-input"
        type="text"
        placeholder="Your name"
        maxlength="40"
        bind:value={displayName}
        onkeydown={handleKey}
        disabled={exiting || saving || !showForm}
        tabindex={showForm ? 0 : -1}
      />

      <span class="hint">You can change this later in Settings.</span>
    </div>
  </div>

  <div class="actions" class:show={showForm}>
    <button
      class="next"
      onclick={handleFinish}
      disabled={exiting || saving || !showForm || !displayName.trim()}
      tabindex={showForm ? 0 : -1}
    >
      <span>{saving ? "Saving…" : BUTTON_LABEL}</span>
      {#if !saving}
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
      {/if}
    </button>
  </div>

  <div class="exit-flash"></div>
  <div class="dark-overlay"></div>
</div>

<style>
  .screen {
    width: 100%;
    min-height: 100%;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 32px;
    padding: 40px;
    position: relative;
    opacity: 0;
    transition: opacity 600ms ease;
    overflow: hidden;
  }
  .screen.in {
    opacity: 1;
  }

  .aurora {
    position: absolute;
    inset: -10%;
    background: radial-gradient(
        circle at 25% 20%,
        rgba(123, 163, 247, 0.24),
        transparent 45%
      ),
      radial-gradient(
        circle at 75% 75%,
        rgba(46, 91, 215, 0.22),
        transparent 50%
      ),
      radial-gradient(
        circle at 50% 50%,
        rgba(255, 213, 106, 0.07),
        transparent 55%
      );
    filter: blur(30px);
    animation: auroraShift 18s ease-in-out infinite;
    pointer-events: none;
    z-index: 0;
  }
  @keyframes auroraShift {
    0%,
    100% {
      transform: translate(0, 0) scale(1);
    }
    33% {
      transform: translate(2%, -1%) scale(1.05);
    }
    66% {
      transform: translate(-1%, 2%) scale(1.02);
    }
  }

  .sparkles {
    position: absolute;
    inset: 0;
    pointer-events: none;
    z-index: 0;
  }
  .spark {
    position: absolute;
    border-radius: 50%;
    background: #fff;
    box-shadow: 0 0 8px rgba(180, 200, 255, 0.7);
    opacity: 0;
    animation: sparkTwinkle linear infinite;
  }
  @keyframes sparkTwinkle {
    0%,
    100% {
      opacity: 0;
      transform: translateY(0) scale(0.6);
    }
    50% {
      opacity: 0.85;
      transform: translateY(-12px) scale(1);
    }
  }

  .stage {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 20px;
    z-index: 2;
    transition:
      transform 900ms cubic-bezier(0.4, 0, 0.2, 1),
      opacity 700ms ease;
  }
  .screen.exiting .stage {
    transform: scale(1.12);
    opacity: 0;
  }

  .winston-wrap {
    position: relative;
    width: 180px;
    height: 180px;
    display: flex;
    align-items: center;
    justify-content: center;
    opacity: 0;
    transform: translateY(18px) scale(0.92);
    transition:
      opacity 800ms ease,
      transform 900ms cubic-bezier(0.22, 1, 0.36, 1);
  }
  .winston-wrap.show {
    opacity: 1;
    transform: translateY(0) scale(1);
  }

  .aura {
    position: absolute;
    inset: 0;
    border-radius: 50%;
    border: 1px solid rgba(123, 163, 247, 0.3);
    pointer-events: none;
    opacity: 0;
  }
  .winston-wrap.show .aura {
    animation: auraPulse 4.2s ease-out infinite;
  }
  .aura-1 {
    animation-delay: 0s;
  }
  .aura-2 {
    animation-delay: 1.4s;
  }
  .aura-3 {
    animation-delay: 2.8s;
  }
  @keyframes auraPulse {
    0% {
      transform: scale(0.85);
      opacity: 0;
      border-color: rgba(123, 163, 247, 0.5);
    }
    40% {
      opacity: 0.9;
    }
    100% {
      transform: scale(1.6);
      opacity: 0;
      border-color: rgba(123, 163, 247, 0);
    }
  }

  .text-block {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    text-align: center;
    min-height: 72px;
    max-width: 620px;
  }
  .line {
    min-height: 24px;
    font-weight: 500;
    color: rgba(220, 230, 255, 0.78);
    font-size: 15px;
  }
  .line.line-1 {
    font-size: 20px;
    font-weight: 600;
    color: #f0f4ff;
    letter-spacing: -0.005em;
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

  .form {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 14px;
    opacity: 0;
    transform: translateY(12px);
    pointer-events: none;
    transition:
      opacity 600ms ease,
      transform 700ms cubic-bezier(0.22, 1, 0.36, 1);
  }
  .form.show {
    opacity: 1;
    transform: translateY(0);
    pointer-events: auto;
  }

  .avatar-slot-wrap {
    position: relative;
    width: 88px;
    height: 88px;
  }
  .avatar-slot {
    position: relative;
    width: 88px;
    height: 88px;
    border-radius: 50%;
    cursor: pointer;
    display: block;
    padding: 0;
    background: transparent;
    border: none;
    box-shadow:
      0 0 0 1px rgba(123, 163, 247, 0.35),
      0 0 26px rgba(46, 91, 215, 0.3);
    transition:
      box-shadow 200ms ease,
      transform 200ms ease;
  }
  .avatar-slot:hover {
    transform: translateY(-1px);
    box-shadow:
      0 0 0 1px rgba(123, 163, 247, 0.6),
      0 0 34px rgba(46, 91, 215, 0.5);
  }

  .avatar-overlay {
    position: absolute;
    inset: 0;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(8, 12, 26, 0.55);
    color: #f0f4ff;
    opacity: 0;
    transition: opacity 180ms ease;
    pointer-events: none;
  }
  .avatar-slot:hover .avatar-overlay {
    opacity: 1;
  }
  .avatar-overlay.filled {
    background: rgba(8, 12, 26, 0.72);
  }

  .avatar-remove {
    position: absolute;
    top: -2px;
    right: -2px;
    width: 22px;
    height: 22px;
    border-radius: 50%;
    background: #1a1f36;
    color: #f0f4ff;
    display: flex;
    align-items: center;
    justify-content: center;
    border: 1px solid rgba(123, 163, 247, 0.4);
    cursor: pointer;
    padding: 0;
    z-index: 2;
    transition:
      background 150ms ease,
      transform 150ms ease;
  }
  .avatar-remove:hover {
    background: #2a3152;
    transform: scale(1.08);
  }

  .name-input {
    width: 260px;
    padding: 10px 14px;
    border-radius: 10px;
    border: 1px solid rgba(123, 163, 247, 0.3);
    background: rgba(10, 16, 38, 0.6);
    color: #f0f4ff;
    font-size: 15px;
    font-weight: 500;
    text-align: center;
    outline: none;
    transition:
      border-color 180ms ease,
      box-shadow 180ms ease,
      background 180ms ease;
    backdrop-filter: blur(8px);
  }
  .name-input::placeholder {
    color: rgba(220, 230, 255, 0.4);
    font-weight: 400;
  }
  .name-input:focus {
    border-color: rgba(123, 163, 247, 0.75);
    background: rgba(14, 22, 50, 0.75);
    box-shadow: 0 0 0 3px rgba(46, 91, 215, 0.25);
  }

  .hint {
    font-size: 12px;
    color: rgba(220, 230, 255, 0.5);
  }

  .actions {
    z-index: 2;
    opacity: 0;
    transform: translateY(10px);
    pointer-events: none;
    transition:
      opacity 600ms ease,
      transform 500ms cubic-bezier(0.4, 0, 0.2, 1);
  }
  .actions.show {
    opacity: 1;
    transform: translateY(0);
    pointer-events: auto;
  }
  .screen.exiting .actions {
    opacity: 0;
    transform: scale(0.85);
  }

  .next {
    display: inline-flex;
    align-items: center;
    gap: 10px;
    padding: 14px 30px;
    background: linear-gradient(135deg, #2e5bd7 0%, #1e50e5 100%);
    color: #fff;
    border-radius: 999px;
    font-size: 16px;
    font-weight: 600;
    letter-spacing: 0.01em;
    box-shadow:
      0 0 0 1px rgba(123, 163, 247, 0.4),
      0 0 30px rgba(46, 91, 215, 0.65),
      0 10px 28px rgba(0, 0, 0, 0.5);
    transition:
      transform 150ms ease,
      box-shadow 150ms ease,
      opacity 150ms ease;
    animation: ctaPulse 3s ease-in-out 800ms infinite;
  }
  .next:hover:not(:disabled) {
    transform: translateY(-1px);
    animation: none;
    box-shadow:
      0 0 0 1px rgba(123, 163, 247, 0.7),
      0 0 42px rgba(46, 91, 215, 0.9),
      0 14px 34px rgba(0, 0, 0, 0.55);
  }
  .next:active:not(:disabled) {
    transform: translateY(0);
  }
  .next:disabled {
    cursor: default;
    opacity: 0.55;
    animation: none;
  }

  @keyframes ctaPulse {
    0%,
    100% {
      box-shadow:
        0 0 0 1px rgba(123, 163, 247, 0.4),
        0 0 30px rgba(46, 91, 215, 0.65),
        0 10px 28px rgba(0, 0, 0, 0.5);
    }
    50% {
      box-shadow:
        0 0 0 1px rgba(123, 163, 247, 0.65),
        0 0 46px rgba(46, 91, 215, 0.95),
        0 10px 28px rgba(0, 0, 0, 0.5);
    }
  }
  .exit-flash {
    position: absolute;
    inset: 0;
    pointer-events: none;
    background: radial-gradient(
      circle at 50% 55%,
      rgba(220, 232, 255, 0.98) 0%,
      rgba(123, 163, 247, 0.55) 35%,
      rgba(46, 91, 215, 0.15) 60%,
      transparent 75%
    );
    opacity: 0;
    transform: scale(0.15);
    z-index: 5;
  }
  .screen.exiting .exit-flash {
    animation: exitFlash 2000ms cubic-bezier(0.22, 1, 0.36, 1) forwards;
  }
  @keyframes exitFlash {
    0% {
      opacity: 0;
      transform: scale(0.15);
    }
    25% {
      opacity: 1;
      transform: scale(1.4);
    }
    50% {
      opacity: 1;
      transform: scale(3.5);
    }
    100% {
      opacity: 0;
      transform: scale(4);
    }
  }

  .dark-overlay {
    position: absolute;
    inset: 0;
    background: #080c1a;
    opacity: 0;
    pointer-events: none;
    z-index: 6;
  }
  .screen.exiting .dark-overlay {
    animation: fadeToDark 2000ms ease forwards;
  }
  @keyframes fadeToDark {
    0%,
    40% {
      opacity: 0;
    }
    100% {
      opacity: 1;
    }
  }

  @media (max-width: 640px) {
    .screen {
      gap: 22px;
      padding: 24px 20px;
    }
    .winston-wrap {
      width: 140px;
      height: 140px;
    }
    .line {
      font-size: 14px;
    }
    .line.line-1 {
      font-size: 17px;
    }
    .name-input {
      width: 220px;
    }
  }
</style>
