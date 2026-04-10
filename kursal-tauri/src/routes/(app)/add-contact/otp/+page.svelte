<script lang="ts">
  import { browser } from "$app/environment";
  import { onMount } from "svelte";
  import { generateOtp, publishOtp, fetchOtp } from "$lib/api/otp";
  import { contactsState } from "$lib/state/contacts.svelte";
  import { goto } from "$app/navigation";
  import { notifications } from "$lib/state/notifications.svelte";
  import Button from "$lib/components/Button.svelte";
  import QRCode from "qrcode";
  import { Copy, QrCode, Type, RefreshCw, ClipboardPaste } from "lucide-svelte";

  let mode = $state<"share" | "receive">("share");
  let shareView = $state<"words" | "qr">("words");
  let otp = $state<string | null>(null);
  let qrDataUrl = $state<string | null>(null);
  let status = $state<"idle" | "generating" | "waiting" | "done">("idle");
  let inputOtp = $state("");
  let fetchStatus = $state<"idle" | "loading" | "error">("idle");
  let fetchError = $state("");
  let expiresAt = $state<number | null>(null);
  let countdown = $state(600); // 10 min in seconds
  let countdownInterval: ReturnType<typeof setInterval> | null = null;
  const storageKey = "kursal_add_contact_otp";

  type PersistedOtpState = {
    otp: string;
    shareView: "words" | "qr";
    expiresAt: number;
  };

  function persistOtpState() {
    if (!browser) return;

    if (!otp || !expiresAt) {
      sessionStorage.removeItem(storageKey);
      return;
    }

    const payload: PersistedOtpState = {
      otp,
      shareView,
      expiresAt,
    };

    sessionStorage.setItem(storageKey, JSON.stringify(payload));
  }

  function clearPersistedOtpState() {
    if (browser) {
      sessionStorage.removeItem(storageKey);
    }
  }

  function stopCountdown(clearState = false) {
    if (countdownInterval) clearInterval(countdownInterval);
    countdownInterval = null;

    if (clearState) {
      status = "idle";
      otp = null;
      qrDataUrl = null;
      expiresAt = null;
      countdown = 600;
      clearPersistedOtpState();
    }
  }

  function updateCountdown() {
    if (!expiresAt) return;

    const remainingSeconds = Math.max(
      0,
      Math.ceil((expiresAt - Date.now()) / 1000),
    );
    countdown = remainingSeconds;

    if (remainingSeconds <= 0) {
      stopCountdown(true);
    }
  }

  function startCountdown(nextExpiresAt: number) {
    expiresAt = nextExpiresAt;
    updateCountdown();

    if (countdownInterval) clearInterval(countdownInterval);
    countdownInterval = setInterval(updateCountdown, 1000);
  }

  async function handleGenerateCode() {
    status = "generating";
    shareView = "words";
    try {
      const result = await generateOtp();
      otp = result.otp;
      await publishOtp(result.otp);
      await renderQr(result.otp);
      startCountdown(Date.now() + 10 * 60 * 1000);
      status = "waiting";
      persistOtpState();
    } catch (e) {
      notifications.push("Failed to generate code", "error");
      console.error("Generate OTP failed:", e);
      status = "idle";
    }
  }

  async function renderQr(code: string) {
    try {
      qrDataUrl = await QRCode.toDataURL(code, {
        errorCorrectionLevel: "M",
        margin: 1,
        width: 360,
        color: {
          dark: "#0f172a",
          light: "#f8fafc",
        },
      });
    } catch (e) {
      qrDataUrl = null;
      console.error("Failed to render QR:", e);
    }
  }

  async function handleFetchOtp() {
    fetchStatus = "loading";
    fetchError = "";
    try {
      const contact = await fetchOtp(inputOtp.trim());
      contactsState.upsert(contact);
      notifications.push("Contact added!", "success");
      fetchStatus = "idle";
      goto("/chat/" + contact.userId);
    } catch (e) {
      fetchStatus = "error";
      fetchError = String(e);
      console.error("Fetch OTP failed:", e);
    }
  }

  async function copyCode() {
    if (otp) {
      try {
        await navigator.clipboard.writeText(otp);
        notifications.push("Code copied!", "success");
      } catch (e) {
        console.error("Failed to copy:", e);
      }
    }
  }

  async function pasteCode() {
    try {
      const text = await navigator.clipboard.readText();
      if (text.trim()) {
        inputOtp = text.trim();
      }
    } catch (e) {
      console.error("Failed to paste from clipboard:", e);
    }
  }

  onMount(() => {
    if (browser) {
      const raw = sessionStorage.getItem(storageKey);

      if (raw) {
        try {
          const parsed = JSON.parse(raw) as PersistedOtpState;
          if (parsed.otp && parsed.expiresAt > Date.now()) {
            otp = parsed.otp;
            shareView = parsed.shareView;
            startCountdown(parsed.expiresAt);
            void renderQr(parsed.otp);
            status = "waiting";
          } else {
            clearPersistedOtpState();
          }
        } catch (e) {
          console.error("Failed to restore OTP state:", e);
          clearPersistedOtpState();
        }
      }
    }

    return () => {
      stopCountdown();
    };
  });

  const minutes = $derived(Math.floor(countdown / 60));
  const seconds = $derived(countdown % 60);
  const formattedTime = $derived(
    `${minutes}:${seconds.toString().padStart(2, "0")}`,
  );
</script>

<div class="otp-flow">
  <div class="mode-toggle" role="tablist" aria-label="OTP workflow mode">
    <button
      class="toggle-pill"
      class:active={mode === "share"}
      onclick={() => (mode = "share")}
      role="tab"
      aria-selected={mode === "share"}>Share</button
    >
    <button
      class="toggle-pill"
      class:active={mode === "receive"}
      onclick={() => (mode = "receive")}
      role="tab"
      aria-selected={mode === "receive"}>Receive</button
    >
  </div>

  {#if mode === "share"}
    <section class="mode-content">
      <div class="heading-row">
        <div>
          <h3>Share code</h3>
          <p class="subtle">
            This code expires automatically after 10 minutes.
          </p>
        </div>

        {#if otp && status === "waiting"}
          <span class="timer-pill">Expiring in {formattedTime}</span>
        {/if}
      </div>

      {#if otp && status === "waiting"}
        <p class="explanation">
          This code expires automatically after 10 minutes. Send this to your
          contact through a secure channel.
        </p>

        <div class="sub-toggle" role="tablist" aria-label="OTP share view">
          <button
            class="sub-toggle-button"
            class:active={shareView === "words"}
            onclick={() => (shareView = "words")}
          >
            <Type size={14} />
            Words
          </button>
          <button
            class="sub-toggle-button"
            class:active={shareView === "qr"}
            onclick={() => (shareView = "qr")}
          >
            <QrCode size={14} />
            QR
          </button>
        </div>

        {#if shareView === "words"}
          <div
            class="code-display"
            role="group"
            aria-label="One-time passphrase words"
          >
            <div class="code-grid">
              {#each otp.split(/\s+/).filter((s) => s.length > 0) as word}
                <div class="code-word">
                  <span class="word-value">{word}</span>
                </div>
              {/each}
            </div>
          </div>
        {:else}
          <div class="qr-card">
            {#if qrDataUrl}
              <img
                class="qr-image"
                src={qrDataUrl}
                alt="QR code containing the one-time contact phrase"
              />
            {:else}
              <p class="qr-fallback">
                QR preview unavailable, use the words view.
              </p>
            {/if}
          </div>
        {/if}

        <div class="action-row">
          <Button variant="secondary" onclick={copyCode}>
            <Copy size={14} />
            Copy Code
          </Button>
          <Button variant="secondary" onclick={handleGenerateCode}>
            <RefreshCw size={14} />
            Regenerate
          </Button>
        </div>
      {:else}
        <p class="explanation">
          Generate a short-lived 8-word phrase and share it with someone you
          trust.
        </p>
        <Button
          variant="primary"
          loading={status === "generating"}
          onclick={handleGenerateCode}
        >
          Generate Code
        </Button>
        <p class="help-text">
          After generation, you can switch between words and QR.
        </p>
      {/if}
    </section>
  {:else}
    <section class="mode-content">
      <div class="heading-row">
        <div>
          <h3>Enter code</h3>
          <p class="subtle">Paste or type the 8 words from your contact.</p>
        </div>
      </div>

      <div class="input-wrap">
        <textarea
          value={inputOtp}
          onchange={(e) => (inputOtp = (e.target as HTMLTextAreaElement).value)}
          placeholder="example: maple echo ribbon solar ..."
          rows="3"
          disabled={fetchStatus === "loading"}
          autocapitalize="off"
          spellcheck="false"
        ></textarea>
        <button class="paste-button" type="button" onclick={pasteCode}>
          <ClipboardPaste size={14} />
          Paste
        </button>
      </div>

      {#if fetchError}
        <div class="error-message">
          {fetchError.includes("expired")
            ? "This contact code has expired. Ask for a new one."
            : "Invalid code. Make sure you copied it correctly."}
        </div>
      {/if}

      <Button
        variant="primary"
        loading={fetchStatus === "loading"}
        disabled={!inputOtp.trim()}
        onclick={handleFetchOtp}
      >
        Connect Contact
      </Button>
    </section>
  {/if}
</div>

<style>
  .otp-flow {
    max-width: 760px;
    display: grid;
    gap: 16px;
  }

  .mode-toggle {
    display: flex;
    gap: 6px;
    background: rgba(15, 23, 42, 0.45);
    border-radius: 12px;
    padding: 4px;
    border: 1px solid rgba(148, 163, 184, 0.22);
    width: fit-content;
  }

  .toggle-pill {
    min-width: 92px;
    padding: 8px 12px;
    border: 1px solid transparent;
    background: transparent;
    color: var(--text-secondary);
    cursor: pointer;
    border-radius: 10px;
    transition: all var(--transition);
    font-weight: 700;
    font-size: 12px;
    letter-spacing: 0.06em;
  }

  .toggle-pill.active {
    background: rgba(51, 65, 85, 0.9);
    color: var(--text-primary);
    border-color: rgba(148, 163, 184, 0.35);
  }

  .mode-content {
    display: flex;
    flex-direction: column;
    gap: 16px;
    border: 1px solid rgba(148, 163, 184, 0.22);
    border-radius: 14px;
    background: rgba(15, 23, 42, 0.5);
    backdrop-filter: blur(14px);
    padding: 18px;
  }

  .heading-row {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 14px;
  }

  h3 {
    margin: 0;
    font-size: 20px;
    line-height: 1.2;
  }

  .subtle {
    margin: 4px 0 0;
    color: var(--text-muted);
    font-size: 13px;
  }

  .timer-pill {
    font-size: 12px;
    font-weight: 700;
    color: var(--text-secondary);
    border: 1px solid rgba(148, 163, 184, 0.28);
    background: rgba(30, 41, 59, 0.55);
    border-radius: 999px;
    padding: 7px 11px;
    white-space: nowrap;
  }

  .explanation {
    margin: 0;
    color: var(--text-secondary);
    font-size: 14px;
    line-height: 1.5;
  }

  .help-text {
    margin: 0;
    color: var(--text-muted);
    font-size: 13px;
    line-height: 1.5;
  }

  .sub-toggle {
    display: flex;
    gap: 8px;
  }

  .sub-toggle-button {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    border-radius: 10px;
    border: 1px solid rgba(148, 163, 184, 0.26);
    background: rgba(15, 23, 42, 0.42);
    color: var(--text-secondary);
    font-size: 12px;
    font-weight: 700;
    padding: 8px 10px;
    transition: all var(--transition);
  }

  .sub-toggle-button.active {
    background: rgba(51, 65, 85, 0.9);
    border-color: rgba(148, 163, 184, 0.38);
    color: var(--text-primary);
  }

  .code-display {
    padding: 0;
  }

  .code-grid {
    display: flex;
    flex-wrap: wrap;
    gap: 10px 16px;
  }

  .code-word {
    padding: 8px 10px;
    min-width: 0;
    border-radius: 10px;
    border: 1px solid rgba(148, 163, 184, 0.25);
    background: rgba(30, 41, 59, 0.62);
  }

  .word-value {
    font-family: "SF Mono", "Menlo", "Monaco", "Courier New", monospace;
    font-size: 14px;
    font-weight: 700;
    line-height: 1.35;
    overflow-wrap: anywhere;
    word-break: normal;
    color: var(--text-primary);
  }

  .qr-card {
    padding: 0;
    display: flex;
    align-items: center;
    justify-content: flex-start;
  }

  .qr-image {
    width: min(320px, 100%);
    border-radius: 10px;
  }

  .action-row {
    display: flex;
    flex-wrap: wrap;
    gap: 14px;
    padding: 8px 0;
  }

  .qr-fallback {
    margin: 0;
    color: var(--text-muted);
    font-size: 13px;
  }

  :global(.action-row .button) {
    width: auto;
    min-width: 180px;
    flex: 1 1 220px;
  }

  .input-wrap {
    position: relative;
  }

  textarea {
    background: rgba(15, 23, 42, 0.35);
    border: 1px solid rgba(148, 163, 184, 0.24);
    border-radius: 12px;
    color: var(--text-primary);
    padding: 14px 92px 14px 14px;
    font-size: 14px;
    resize: vertical;
    min-height: 92px;
    width: 100%;
    font-family: "SF Mono", "Menlo", "Monaco", "Courier New", monospace;
    transition: all var(--transition);
  }

  textarea:focus {
    outline: none;
    border-color: rgba(129, 140, 248, 0.55);
    box-shadow: 0 0 0 2px rgba(129, 140, 248, 0.16);
  }

  textarea:disabled {
    opacity: 0.5;
  }

  .paste-button {
    position: absolute;
    right: 10px;
    top: 10px;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    border-radius: 8px;
    border: 1px solid rgba(148, 163, 184, 0.26);
    background: rgba(30, 41, 59, 0.72);
    color: var(--text-secondary);
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.02em;
    padding: 7px 8px;
    transition: all var(--transition);
  }

  .paste-button:hover {
    border-color: rgba(148, 163, 184, 0.42);
    color: var(--text-primary);
  }

  .error-message {
    background: rgba(237, 66, 69, 0.1);
    color: var(--danger);
    padding: 12px;
    border-radius: 10px;
    font-size: 13px;
    line-height: 1.5;
    border: 1px solid rgba(237, 66, 69, 0.35);
  }

  :global(.mode-content .button) {
    width: 100%;
  }

  @media (max-width: 740px) {
    .mode-content {
      padding: 16px;
      border-radius: 16px;
    }

    h3 {
      font-size: 19px;
    }

    .heading-row {
      flex-direction: column;
      align-items: flex-start;
    }

    .action-row {
      flex-direction: column;
      padding: 0;
    }

    :global(.action-row .button) {
      min-width: 0;
      width: 100%;
      flex: 1 1 auto;
    }

    .toggle-pill {
      min-width: 80px;
      padding-inline: 10px;
    }
  }
</style>
