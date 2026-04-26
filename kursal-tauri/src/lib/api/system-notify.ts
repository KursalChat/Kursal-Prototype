import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from '@tauri-apps/plugin-notification';
import { prefsState } from '$lib/state/prefs.svelte';

let cached: boolean | null = null;

export async function getPermission(force = false): Promise<boolean> {
  if (cached !== null && !force) return cached;
  cached = await isPermissionGranted();
  return cached;
}

export async function ensurePermission(): Promise<boolean> {
  if (await getPermission()) return true;
  const r = await requestPermission();
  cached = r === 'granted';
  return cached;
}

function parseTime(s: string): number | null {
  const [h, m] = s.split(':').map((x) => parseInt(x, 10));
  if (isNaN(h) || isNaN(m)) return null;
  return h * 60 + m;
}

export function isInDndWindow(now: Date = new Date()): boolean {
  const { enabled, start, end } = prefsState.dnd;
  if (!enabled) return false;
  const s = parseTime(start);
  const e = parseTime(end);
  if (s === null || e === null) return false;
  if (s === e) return false;
  const cur = now.getHours() * 60 + now.getMinutes();
  if (s < e) return cur >= s && cur < e;
  return cur >= s || cur < e;
}

export interface MessageNotifyOptions {
  senderName: string;
  body: string;
}

export async function notifyMessage({ senderName, body }: MessageNotifyOptions) {
  const preview = prefsState.notificationPreview;
  if (preview === 'none') return;
  if (isInDndWindow()) return;
  if (!(await getPermission())) return;

  let title: string;
  let text: string | undefined;
  switch (preview) {
    case 'content':
      title = senderName;
      text = body;
      break;
    case 'sender':
      title = `New message from ${senderName}`;
      break;
    case 'generic':
    default:
      title = 'Kursal';
      text = 'New message';
      break;
  }

  sendNotification(text ? { title, body: text } : { title });
}

export async function sendTestNotification(): Promise<boolean> {
  if (!(await ensurePermission())) return false;

  const senderName = 'Test User';
  const body = 'This is a test message from Kursal.';
  const preview = prefsState.notificationPreview;

  let title: string;
  let text: string | undefined;
  switch (preview) {
    case 'sender':
      title = `New message from ${senderName}`;
      break;
    case 'generic':
      title = 'Kursal';
      text = 'New message';
      break;
    case 'none':
      title = 'Kursal';
      text = 'Notifications are set to Off — real messages will not show up.';
      break;
    case 'content':
    default:
      title = senderName;
      text = body;
      break;
  }

  sendNotification(text ? { title, body: text } : { title });
  return true;
}
