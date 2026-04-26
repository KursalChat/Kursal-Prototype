import { browser } from '$app/environment';

export type NotificationPreview = 'content' | 'sender' | 'generic' | 'none';

export interface DndSchedule {
  enabled: boolean;
  start: string; // "HH:MM"
  end: string;   // "HH:MM"
}

const KEYS = {
  preview: 'kursal_notif_preview',
  dnd: 'kursal_notif_dnd',
};

function readJson<T>(key: string, fallback: T): T {
  if (!browser) return fallback;
  const raw = localStorage.getItem(key);
  if (!raw) return fallback;
  try { return JSON.parse(raw) as T; } catch { return fallback; }
}

function writeJson<T>(key: string, value: T) {
  if (!browser) return;
  localStorage.setItem(key, JSON.stringify(value));
}

function createPrefsState() {
  let notificationPreview = $state<NotificationPreview>('content');
  let dnd = $state<DndSchedule>({ enabled: false, start: '22:00', end: '06:00' });
  let initialized = $state(false);

  function init() {
    if (!browser || initialized) return;
    notificationPreview = readJson<NotificationPreview>(KEYS.preview, 'content');
    dnd = readJson<DndSchedule>(KEYS.dnd, dnd);
    initialized = true;
  }

  function setPreview(value: NotificationPreview) {
    notificationPreview = value;
    writeJson(KEYS.preview, value);
  }

  function setDnd(value: DndSchedule) {
    dnd = value;
    writeJson(KEYS.dnd, value);
  }

  return {
    get notificationPreview() { return notificationPreview; },
    get dnd() { return dnd; },
    init,
    setPreview,
    setDnd,
  };
}

export const prefsState = createPrefsState();
