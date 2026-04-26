export type ConfirmTone = "default" | "warning" | "danger";

export interface ConfirmOptions {
  title: string;
  message?: string;
  detail?: string;
  confirmLabel?: string;
  cancelLabel?: string;
  tone?: ConfirmTone;
  // If set, confirm button is locked for this many ms with a fill animation.
  holdMs?: number;
}

function createConfirmState() {
  let open = $state(false);
  let options = $state<ConfirmOptions | null>(null);
  let resolver: ((value: boolean) => void) | null = null;

  function ask(opts: ConfirmOptions): Promise<boolean> {
    options = opts;
    open = true;
    return new Promise((resolve) => {
      resolver = resolve;
    });
  }

  function close(result: boolean) {
    open = false;
    const r = resolver;
    resolver = null;
    const prev = options;
    // keep options during exit animation if you add one later
    queueMicrotask(() => {
      if (!open) options = null;
    });
    void prev;
    r?.(result);
  }

  return {
    get open() { return open; },
    get options() { return options; },
    ask,
    confirm: () => close(true),
    cancel: () => close(false),
  };
}

export const confirmState = createConfirmState();
export const confirmDialog = (opts: ConfirmOptions) => confirmState.ask(opts);
