import { appCacheDir, join } from "@tauri-apps/api/path";
import { copyFile, readFile, remove, writeFile } from "@tauri-apps/plugin-fs";
import { open, save } from "@tauri-apps/plugin-dialog";
import { isMobile, OS } from "$lib/api/window";

const deferredDownloadTargets = new Map<
  string,
  { targetUri: string; filename: string }
>();

function isUriPath(path: string): boolean {
  return /^[a-z][a-z0-9+.-]*:\/\//i.test(path);
}

function pathFromFileUri(uri: string): string {
  const parsed = new URL(uri);
  let pathname = decodeURIComponent(parsed.pathname);
  if (/^\/[A-Za-z]:/.test(pathname)) pathname = pathname.slice(1);
  return pathname;
}

function sanitizeFilename(name: string): string {
  const trimmed = name.trim();
  const safe = trimmed.replace(/[^a-zA-Z0-9._-]/g, "_");
  return safe.length > 0 ? safe : "file";
}

export function filenameFromPath(value: string): string {
  if (!value) return "file";
  if (value.startsWith("file://")) {
    const localPath = pathFromFileUri(value);
    const part = localPath.split(/[\\/]/).pop();
    return part ? sanitizeFilename(part) : "file";
  }
  if (isUriPath(value)) {
    try {
      const parsed = new URL(value);
      const part = decodeURIComponent(parsed.pathname).split("/").pop();
      return part ? sanitizeFilename(part) : "file";
    } catch {
      return "file";
    }
  }
  const part = value.split(/[\\/]/).pop();
  return part ? sanitizeFilename(part) : "file";
}

async function writeBytesToAppCache(
  bytes: Uint8Array,
  suggestedFilename: string,
  prefix: string,
): Promise<string> {
  const cacheDir = await appCacheDir();
  const timestamp = Date.now();
  const random = Math.floor(Math.random() * 1_000_000_000)
    .toString()
    .padStart(9, "0");
  const tempName = `${prefix}-${timestamp}-${random}-${sanitizeFilename(suggestedFilename)}`;
  const tempPath = await join(cacheDir, tempName);
  await writeFile(tempPath, bytes);
  return tempPath;
}

function extractPath(raw: unknown): string {
  if (raw === null || raw === undefined) return "";
  if (typeof raw === "string") return raw;
  if (Array.isArray(raw)) return extractPath(raw[0]);
  if (typeof raw === "object") {
    const r = raw as { path?: unknown; toString?: () => string };
    if (typeof r.path === "string") return r.path;
    if (typeof r.toString === "function") return r.toString();
  }
  return String(raw);
}

export interface PreparedFile {
  backendPath: string;
  filename: string;
}

export async function pickFileForSend(
  suggestedName?: string,
): Promise<PreparedFile | null> {
  try {
    const selected = await open({
      multiple: false,
      directory: false,
      ...(suggestedName ? { defaultPath: suggestedName } : {}),
      pickerMode: "document",
      fileAccessMode: "copy",
    });
    const raw = extractPath(selected);
    if (!raw) return null;
    return await prepareOfferSourcePath(raw);
  } catch (e) {
    // Android: some builds reject unknown options — retry without them.
    if (isMobile) {
      const selected = await open({ multiple: false, directory: false });
      const raw = extractPath(selected);
      if (!raw) return null;
      return await prepareOfferSourcePath(raw);
    }
    throw e;
  }
}

export interface ResolvedReceivePath {
  backendPath: string;
  deferredTargetUri: string | null;
}

export async function pickFileForReceive(
  filename: string,
): Promise<ResolvedReceivePath | null> {
  if (OS == "android") {
    // Android SAF: we can't always use save() reliably. Write to app cache, then
    // let the user share out once done (via platform-level share) — see notes below.
    // For now, write directly to app cache; backend fills in.
    const backendPath = await writeBytesToAppCache(
      new Uint8Array(0),
      filename,
      "recv",
    );
    return { backendPath, deferredTargetUri: null };
  }

  const savePath = await save({ defaultPath: filename });
  const raw = extractPath(savePath);
  if (!raw) return null;
  return await resolveReceiveSavePath(raw, filename);
}

export async function prepareOfferSourcePath(
  rawSelection: string,
): Promise<PreparedFile> {
  const filename = filenameFromPath(rawSelection);

  if (rawSelection.startsWith("file://")) {
    return { backendPath: pathFromFileUri(rawSelection), filename };
  }

  if (rawSelection.startsWith("content://") || isUriPath(rawSelection)) {
    const bytes = await readFile(rawSelection);
    const tempPath = await writeBytesToAppCache(bytes, filename, "send");
    return { backendPath: tempPath, filename };
  }

  return { backendPath: rawSelection, filename };
}

export async function resolveReceiveSavePath(
  rawSelection: string,
  defaultFilename: string,
): Promise<ResolvedReceivePath> {
  if (rawSelection.startsWith("file://")) {
    return {
      backendPath: pathFromFileUri(rawSelection),
      deferredTargetUri: null,
    };
  }

  if (rawSelection.startsWith("content://")) {
    const backendPath = await writeBytesToAppCache(
      new Uint8Array(0),
      defaultFilename,
      "recv",
    );
    return { backendPath, deferredTargetUri: rawSelection };
  }

  return { backendPath: rawSelection, deferredTargetUri: null };
}

export function registerDeferredReceiveTarget(
  backendPath: string,
  targetUri: string,
  filename: string,
) {
  deferredDownloadTargets.set(backendPath, { targetUri, filename });
}

export async function finalizeDeferredReceiveTarget(savePath: string): Promise<{
  filename: string;
  moved: boolean;
}> {
  const deferred = deferredDownloadTargets.get(savePath);
  if (!deferred) {
    return { filename: filenameFromPath(savePath), moved: false };
  }

  try {
    try {
      await copyFile(savePath, deferred.targetUri);
    } catch {
      const bytes = await readFile(savePath);
      await writeFile(deferred.targetUri, bytes);
    }
    await remove(savePath);
    deferredDownloadTargets.delete(savePath);
    return { filename: deferred.filename, moved: true };
  } catch {
    deferredDownloadTargets.delete(savePath);
    throw new Error(
      "Could not finalize downloaded file to selected destination",
    );
  }
}
