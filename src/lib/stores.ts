import { writable } from "svelte/store";

export type ViewId =
  | "dashboard"
  | "editor"
  | "merge"
  | "split"
  | "rotate"
  | "organize"
  | "compress"
  | "images"
  | "security"
  | "annotate"
  | "compare"
  | "activity";

export const currentView = writable<ViewId>("dashboard");

export interface PendingDropFiles {
  targetView: ViewId;
  paths: string[];
}

export const pendingFiles = writable<PendingDropFiles | null>(null);

// --- Theme -----------------------------------------------------------------

export type Theme = "dark" | "light";

function createTheme() {
  const initial: Theme =
    (localStorage.getItem("pdfkit-theme") as Theme) ?? "dark";
  apply(initial);
  const { subscribe, set } = writable<Theme>(initial);
  return {
    subscribe,
    toggle() {
      const next: Theme = get() === "dark" ? "light" : "dark";
      localStorage.setItem("pdfkit-theme", next);
      apply(next);
      set(next);
    },
  };

  function get(): Theme {
    let value: Theme = "dark";
    subscribe((v) => (value = v))();
    return value;
  }
}

function apply(theme: Theme) {
  document.documentElement.classList.toggle("dark", theme === "dark");
}

export const theme = createTheme();

// --- Toasts ----------------------------------------------------------------

export interface ToastItem {
  id: number;
  message: string;
  kind: "success" | "error" | "info";
  actionLabel?: string;
  actionPath?: string;
}

export const toasts = writable<ToastItem[]>([]);

let toastId = 0;
const toastTimers = new Map<number, any>();
const MAX_VISIBLE_TOASTS = 3;

export function dismissToast(id: number) {
  if (toastTimers.has(id)) {
    clearTimeout(toastTimers.get(id));
    toastTimers.delete(id);
  }
  toasts.update((list) => list.filter((t) => t.id !== id));
}

export function pushToast(
  message: string,
  kind: ToastItem["kind"] = "info",
  action?: { label: string; path: string }
) {
  // Anti-Spam / Deduplication: If an identical message is already visible, don't spam duplicate toasts
  let existingId: number | null = null;
  toasts.update((list) => {
    const existing = list.find((t) => t.message === message && t.kind === kind);
    if (existing) {
      existingId = existing.id;
    }
    return list;
  });

  if (existingId !== null) {
    // Refresh its dismiss timer instead of stacking duplicate bubbles
    if (toastTimers.has(existingId)) {
      clearTimeout(toastTimers.get(existingId));
    }
    const timer = setTimeout(() => {
      dismissToast(existingId!);
    }, 3500);
    toastTimers.set(existingId, timer);
    return;
  }

  const id = ++toastId;
  toasts.update((list) => {
    // Cap visible toasts to MAX_VISIBLE_TOASTS
    const trimmed =
      list.length >= MAX_VISIBLE_TOASTS
        ? list.slice(list.length - (MAX_VISIBLE_TOASTS - 1))
        : list;
    return [
      ...trimmed,
      {
        id,
        message,
        kind,
        actionLabel: action?.label,
        actionPath: action?.path,
      },
    ];
  });

  const timer = setTimeout(() => {
    dismissToast(id);
  }, 4500);
  toastTimers.set(id, timer);
}
