interface Toast {
  id: string;
  message: string;
  kind: 'info' | 'success' | 'error';
}

function createNotifications() {
  let toasts = $state<Toast[]>([]);

  function push(message: string, kind: Toast['kind'] = 'info') {
    const id = crypto.randomUUID();
    toasts.push({ id, message, kind });
    setTimeout(() => dismiss(id), 4000);
  }

  function dismiss(id: string) {
    toasts = toasts.filter(t => t.id !== id);
  }

  return {
    get toasts() { return toasts; },
    push, dismiss,
  };
}

export const notifications = createNotifications();
