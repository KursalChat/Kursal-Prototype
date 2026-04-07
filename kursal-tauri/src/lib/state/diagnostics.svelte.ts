export type DiagnosticLevel = 'info' | 'warn' | 'error';

export interface DiagnosticEntry {
  id: number;
  ts: number;
  level: DiagnosticLevel;
  category: string;
  message: string;
}

function createDiagnosticsState() {
  let entries = $state<DiagnosticEntry[]>([]);
  let nextId = 1;
  const MAX_ENTRIES = 400;

  function push(level: DiagnosticLevel, category: string, message: string) {
    const entry: DiagnosticEntry = {
      id: nextId++,
      ts: Date.now(),
      level,
      category,
      message,
    };
    entries = [...entries, entry];
    if (entries.length > MAX_ENTRIES) {
      entries = entries.slice(entries.length - MAX_ENTRIES);
    }
  }

  function clear() {
    entries = [];
  }

  return {
    get entries() {
      return entries;
    },
    push,
    clear,
    info: (category: string, message: string) => push('info', category, message),
    warn: (category: string, message: string) => push('warn', category, message),
    error: (category: string, message: string) => push('error', category, message),
  };
}

export const diagnosticsState = createDiagnosticsState();
