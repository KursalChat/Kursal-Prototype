import { marked } from "marked";
import DOMPurify from "dompurify";
import { openUrl } from "@tauri-apps/plugin-opener";
import { confirm } from "@tauri-apps/plugin-dialog";
import { notifications } from "$lib/state/notifications.svelte";

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

export function getStatusLabel(s: string | undefined): string {
  if (!s || s === "disconnected") return "Offline";
  if (s === "direct") return "Connected";
  if (s === "holepunch") return "Connected · p2p";
  if (s === "relay") return "Connected · relay";
  return s.charAt(0).toUpperCase() + s.slice(1);
}

const markdownCache = new Map<string, string>();

export function renderMarkdown(content: string, isEdited: boolean = false): string {
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
      "p", "br", "b", "i", "em", "strong", "a", "pre", "code", "blockquote",
      "ul", "ol", "li", "del", "h1", "h2", "h3", "h4", "h5", "h6", "hr", "span",
    ],
    ALLOWED_ATTR: ["href", "class", "target", "rel", "style"],
  });
  markdownCache.set(cacheKey, sanitized);
  if (markdownCache.size > 600) {
    const firstKey = markdownCache.keys().next().value;
    if (firstKey) markdownCache.delete(firstKey);
  }
  return sanitized;
}

const trustedHosts = new Set([
  "github.com",
  "docs.rs",
  "crates.io",
  "kursal.chat",
]);

export async function handleMarkdownClick(e: MouseEvent) {
  const anchor = (e.target as HTMLElement | null)?.closest(
    "a",
  ) as HTMLAnchorElement | null;
  if (!anchor) return;
  e.preventDefault();
  e.stopPropagation();
  const href = anchor.getAttribute("href");
  if (!href) return;
  try {
    const url = new URL(href, window.location.origin);
    if (!["http:", "https:", "mailto:", "tel:"].includes(url.protocol)) {
      notifications.push("Unsupported link type", "error");
      return;
    }
    if (
      ["http:", "https:"].includes(url.protocol) &&
      !trustedHosts.has(url.hostname)
    ) {
      const approved = await confirm(
        `Open external link?\n\n${url.toString()}`,
        { title: "Open Link", kind: "warning" },
      );
      if (!approved) return;
    }
    await openUrl(url.toString());
  } catch (err) {
    notifications.push(
      `Failed to open link: ${err instanceof Error ? err.message : err}`,
      "error",
    );
  }
}
