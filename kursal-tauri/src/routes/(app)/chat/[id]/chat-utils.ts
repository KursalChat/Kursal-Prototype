import { marked, type Tokens } from "marked";
import DOMPurify from "dompurify";
import { openUrl } from "@tauri-apps/plugin-opener";
import { notifications } from "$lib/state/notifications.svelte";
import { confirmDialogWithCheckbox } from "$lib/state/confirm.svelte";
import { trustedDomainsState } from "$lib/state/trustedDomains.svelte";
import { t } from "$lib/i18n";

marked.use({
  extensions: [
    {
      name: "spoiler",
      level: "inline",
      start(src: string) {
        const i = src.indexOf("||");
        return i < 0 ? undefined : i;
      },
      tokenizer(
        this: { lexer: { inlineTokens: (s: string) => Tokens.Generic[] } },
        src: string,
      ) {
        const match = /^\|\|([\s\S]+?)\|\|/.exec(src);
        if (match) {
          return {
            type: "spoiler",
            raw: match[0],
            tokens: this.lexer.inlineTokens(match[1]),
          };
        }
      },
      renderer(this: any, token: any) {
        return `<span class="spoiler" tabindex="0">${this.parser.parseInline(token.tokens)}</span>`;
      },
    },
  ],
});

export type MediaKind = "image" | "audio" | "video" | "other";

const IMAGE_EXT = new Set([
  "png",
  "jpg",
  "jpeg",
  "gif",
  "webp",
  "bmp",
  "svg",
  "avif",
  "heic",
  "heif",
]);
const AUDIO_EXT = new Set([
  "mp3",
  "wav",
  "ogg",
  "oga",
  "m4a",
  "aac",
  "flac",
  "opus",
  "weba",
]);
const VIDEO_EXT = new Set(["mp4", "webm", "mov", "m4v", "ogv", "mkv", "avi"]);

export function mediaKindFromFilename(filename: string): MediaKind {
  const dot = filename.lastIndexOf(".");
  if (dot < 0) return "other";
  const ext = filename.slice(dot + 1).toLowerCase();
  if (IMAGE_EXT.has(ext)) return "image";
  if (AUDIO_EXT.has(ext)) return "audio";
  if (VIDEO_EXT.has(ext)) return "video";
  return "other";
}

export function midTruncate(name: string, maxLen = 30): string {
  if (name.length <= maxLen) return name;
  const dot = name.lastIndexOf(".");
  const ext = dot > 0 && name.length - dot < 8 ? name.slice(dot) : "";
  const stem = ext ? name.slice(0, name.length - ext.length) : name;
  const room = maxLen - ext.length - 1;
  if (room < 6) return name.slice(0, maxLen - 1) + "…";
  const head = Math.ceil(room * 0.6);
  const tail = room - head;
  return stem.slice(0, head) + "…" + stem.slice(stem.length - tail) + ext;
}

export function fileTypeColor(filename: string): string {
  const dot = filename.lastIndexOf(".");
  if (dot < 0) return "var(--text-muted)";
  const ext = filename.slice(dot + 1).toLowerCase();
  if (ext === "pdf") return "#ef4444";
  if (["doc", "docx", "odt", "rtf"].includes(ext)) return "#3b82f6";
  if (["xls", "xlsx", "csv", "ods"].includes(ext)) return "#22c55e";
  if (["ppt", "pptx", "odp", "key"].includes(ext)) return "#f97316";
  if (["zip", "tar", "gz", "7z", "rar", "bz2", "xz"].includes(ext))
    return "#a855f7";
  if (AUDIO_EXT.has(ext)) return "#ec4899";
  if (VIDEO_EXT.has(ext)) return "#14b8a6";
  if (IMAGE_EXT.has(ext)) return "#6366f1";
  if (["txt", "md", "log"].includes(ext)) return "#94a3b8";
  if (["json", "xml", "yaml", "yml", "toml"].includes(ext)) return "#eab308";
  if (
    [
      "js",
      "ts",
      "tsx",
      "jsx",
      "rs",
      "py",
      "go",
      "rb",
      "java",
      "c",
      "cpp",
      "h",
      "hpp",
      "cs",
      "swift",
      "kt",
      "php",
      "html",
      "css",
      "scss",
    ].includes(ext)
  )
    return "#0ea5e9";
  return "var(--text-muted)";
}

export function formatFileSize(bytes: number): string {
  if (bytes <= 0) return "";
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024)
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}

export function getMessagePreview(content: string): string {
  const clean = content.replace(/\s+/g, " ").trim();
  if (!clean) return "(empty)";
  return clean.length > 80 ? clean.slice(0, 77) + "..." : clean;
}

export function formatTime(ts: number): string {
  return new Date(ts).toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
  });
}

export function formatGroupTime(ts: number): string {
  const d = new Date(ts);
  const now = new Date();
  const isToday = d.toDateString() === now.toDateString();
  const yesterday = new Date(now);
  yesterday.setDate(yesterday.getDate() - 1);
  const isYesterday = d.toDateString() === yesterday.toDateString();

  if (isToday) return `Today · ${formatTime(ts)}`;
  if (isYesterday) return `Yesterday · ${formatTime(ts)}`;
  if (now.getTime() - ts < 7 * 24 * 3600 * 1000) {
    return (
      d.toLocaleDateString([], { weekday: "long" }) + ` · ${formatTime(ts)}`
    );
  }
  return (
    d.toLocaleDateString([], {
      month: "short",
      day: "numeric",
      year: d.getFullYear() !== now.getFullYear() ? "numeric" : undefined,
    }) + ` · ${formatTime(ts)}`
  );
}

const markdownCache = new Map<string, string>();

export function renderMarkdown(
  content: string,
  isEdited: boolean = false,
): string {
  const cacheKey = content + (isEdited ? "|e" : "|n");
  const cached = markdownCache.get(cacheKey);
  if (cached) return cached;

  const escaped = content.replace(/</g, "&lt;").replace(/>/g, "&gt;");
  let html = marked.parse(escaped, {
    async: false,
    gfm: true,
    breaks: true,
  }) as string;

  if (isEdited) {
    const badge = ' <span class="edited-tag">(edited)</span>';
    if (html.endsWith("</p>\n"))
      html = html.replace(/<\/p>\n$/, `${badge}</p>\n`);
    else if (html.endsWith("</p>"))
      html = html.replace(/<\/p>$/, `${badge}</p>`);
    else html += badge;
  }

  const sanitized = DOMPurify.sanitize(html, {
    ALLOWED_TAGS: [
      "p",
      "br",
      "b",
      "i",
      "em",
      "strong",
      "a",
      "pre",
      "code",
      "blockquote",
      "ul",
      "ol",
      "li",
      "del",
      "h1",
      "h2",
      "h3",
      "h4",
      "h5",
      "h6",
      "hr",
      "span",
    ],
    ALLOWED_ATTR: ["href", "class", "target", "rel", "style", "tabindex"],
  });
  markdownCache.set(cacheKey, sanitized);
  if (markdownCache.size > 600) {
    const firstKey = markdownCache.keys().next().value;
    if (firstKey) markdownCache.delete(firstKey);
  }
  return sanitized;
}

export async function handleMarkdownClick(e: MouseEvent) {
  const target = e.target as HTMLElement | null;
  const spoiler = target?.closest(".spoiler");
  if (spoiler) {
    e.preventDefault();
    e.stopPropagation();
    spoiler.classList.toggle("revealed");
    return;
  }
  const anchor = target?.closest("a") as HTMLAnchorElement | null;
  if (!anchor) return;
  e.preventDefault();
  e.stopPropagation();
  const href = anchor.getAttribute("href");
  if (!href) return;
  try {
    const url = new URL(href, window.location.origin);
    if (!["http:", "https:", "mailto:", "tel:"].includes(url.protocol)) {
      notifications.push(t("chat.bubble.linkConfirm.unsupported"), "error");
      return;
    }
    const isWeb = url.protocol === "http:" || url.protocol === "https:";
    if (isWeb && !trustedDomainsState.isTrusted(url.hostname)) {
      const result = await confirmDialogWithCheckbox({
        title: t("chat.bubble.linkConfirm.title"),
        message: t("chat.bubble.linkConfirm.message"),
        detail: url.toString(),
        confirmLabel: t("chat.bubble.linkConfirm.open"),
        cancelLabel: t("chat.bubble.linkConfirm.cancel"),
        tone: "warning",
        checkbox: {
          label: t("chat.bubble.linkConfirm.trustDomain", { host: url.hostname }),
        },
      });
      if (!result.confirmed) return;
      if (result.checked) trustedDomainsState.trust(url.hostname);
    }
    await openUrl(url.toString());
  } catch (err) {
    notifications.push(
      t("chat.bubble.linkConfirm.error", {
        error: err instanceof Error ? err.message : String(err),
      }),
      "error",
    );
  }
}
