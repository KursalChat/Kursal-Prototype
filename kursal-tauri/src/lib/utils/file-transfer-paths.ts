import { appCacheDir, join } from "@tauri-apps/api/path";
import { copyFile, readFile, remove, writeFile } from "@tauri-apps/plugin-fs";

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

  // Handle file:///C:/... on Windows.
  if (/^\/[A-Za-z]:/.test(pathname)) {
    pathname = pathname.slice(1);
  }

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

export async function prepareOfferSourcePath(rawSelection: string): Promise<{
  backendPath: string;
  filename: string;
}> {
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
): Promise<{
  backendPath: string;
  deferredTargetUri: string | null;
}> {
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
    return {
      backendPath,
      deferredTargetUri: rawSelection,
    };
  }

  return {
    backendPath: rawSelection,
    deferredTargetUri: null,
  };
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
    return {
      filename: filenameFromPath(savePath),
      moved: false,
    };
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

    return {
      filename: deferred.filename,
      moved: true,
    };
  } catch {
    deferredDownloadTargets.delete(savePath);
    throw new Error("Could not finalize downloaded file to selected destination");
  }
}