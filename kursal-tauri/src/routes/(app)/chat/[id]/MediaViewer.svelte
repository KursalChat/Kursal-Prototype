<script lang="ts">
  import { onMount } from "svelte";
  import { X, Download } from "lucide-svelte";
  import { revealItemInDir } from "@tauri-apps/plugin-opener";
  import { t } from "$lib/i18n";
  import { notifications } from "$lib/state/notifications.svelte";

  interface Props {
    src: string;
    path: string;
    kind: "image" | "video";
    filename: string;
    onClose: () => void;
  }

  let { src, path, kind, filename, onClose }: Props = $props();

  function handleKey(e: KeyboardEvent) {
    if (e.key === "Escape") onClose();
  }

  async function reveal() {
    try {
      await revealItemInDir(path);
    } catch (e) {
      notifications.push(`${e}`, "error");
    }
  }

  onMount(() => {
    window.addEventListener("keydown", handleKey);
    return () => window.removeEventListener("keydown", handleKey);
  });
</script>

<div
  class="viewer"
  role="dialog"
  aria-modal="true"
  aria-label={filename}
  onclick={onClose}
  onkeydown={(e) => e.key === "Enter" && onClose()}
  tabindex="-1"
>
  <div class="topbar" onclick={(e) => e.stopPropagation()} role="presentation">
    <span class="title" title={filename}>{filename}</span>
    <div class="actions">
      <button class="iconbtn" onclick={reveal} title={t("chat.bubble.showInFolder")} aria-label={t("chat.bubble.showInFolder")}>
        <Download size={16} />
      </button>
      <button class="iconbtn" onclick={onClose} title={t("common.close")} aria-label={t("common.close")}>
        <X size={18} />
      </button>
    </div>
  </div>

  <div class="stage" onclick={(e) => e.stopPropagation()} role="presentation">
    {#if kind === "image"}
      <img class="media" {src} alt={filename} />
    {:else}
      <!-- svelte-ignore a11y_media_has_caption -->
      <video class="media" {src} controls autoplay></video>
    {/if}
  </div>
</div>

<style>
  .viewer {
    position: fixed;
    inset: 0;
    z-index: 400;
    background: rgba(0, 0, 0, 0.82);
    backdrop-filter: blur(6px);
    -webkit-backdrop-filter: blur(6px);
    display: flex;
    flex-direction: column;
    animation: fadeIn 0.15s ease;
  }
  @keyframes fadeIn {
    from { opacity: 0; }
    to { opacity: 1; }
  }
  .topbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 10px 14px;
    color: #fff;
    flex-shrink: 0;
  }
  .title {
    font-size: 13px;
    color: rgba(255, 255, 255, 0.85);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 60vw;
  }
  .actions { display: flex; gap: 6px; }
  .iconbtn {
    width: 34px;
    height: 34px;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.12);
    color: #fff;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: background 0.12s ease;
  }
  .iconbtn:hover { background: rgba(255, 255, 255, 0.22); }

  .stage {
    flex: 1;
    min-height: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 8px 16px 24px;
  }
  .media {
    max-width: 100%;
    max-height: 100%;
    object-fit: contain;
    border-radius: 8px;
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.6);
    background: #000;
  }
</style>
