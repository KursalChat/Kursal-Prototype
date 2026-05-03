export type ConfirmTone = "default" | "warning" | "danger";

export interface ConfirmCheckbox {
  label: string;
  defaultChecked?: boolean;
}

export interface ConfirmOptions {
  title: string;
  message?: string;
  detail?: string;
  confirmLabel?: string;
  cancelLabel?: string;
  tone?: ConfirmTone;
  // If set, confirm button is locked for this many ms with a fill animation.
  holdMs?: number;
  // Optional checkbox shown above the buttons.
  checkbox?: ConfirmCheckbox;
}

export interface ConfirmResult {
  confirmed: boolean;
  checked: boolean;
}

function createConfirmState() {
  let open = $state(false);
  let options = $state<ConfirmOptions | null>(null);
  let checkboxChecked = $state(false);
  let resolver: ((value: ConfirmResult) => void) | null = null;

  function ask(opts: ConfirmOptions): Promise<boolean> {
    return askFull(opts).then((r) => r.confirmed);
  }

  function askFull(opts: ConfirmOptions): Promise<ConfirmResult> {
    options = opts;
    checkboxChecked = opts.checkbox?.defaultChecked ?? false;
    open = true;
    return new Promise((resolve) => {
      resolver = resolve;
    });
  }

  function close(confirmed: boolean) {
    const checked = checkboxChecked;
    open = false;
    const r = resolver;
    resolver = null;
    queueMicrotask(() => {
      if (!open) {
        options = null;
        checkboxChecked = false;
      }
    });
    r?.({ confirmed, checked });
  }

  function setChecked(v: boolean) {
    checkboxChecked = v;
  }

  return {
    get open() { return open; },
    get options() { return options; },
    get checkboxChecked() { return checkboxChecked; },
    setChecked,
    ask,
    askFull,
    confirm: () => close(true),
    cancel: () => close(false),
  };
}

export const confirmState = createConfirmState();
export const confirmDialog = (opts: ConfirmOptions) => confirmState.ask(opts);
export const confirmDialogWithCheckbox = (opts: ConfirmOptions & { checkbox: ConfirmCheckbox }) =>
  confirmState.askFull(opts);
