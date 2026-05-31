import { writable, get } from "svelte/store";

export interface ToastAction {
  label: string;
  onAction: () => void;
}

export interface Toast {
  id: number;
  type: "success" | "error" | "info";
  title: string;
  message?: string;
  action?: ToastAction;
  /** Auto-dismiss delay in ms. 0 keeps the toast until dismissed. */
  duration: number;
}

export interface ToastOptions {
  action?: ToastAction;
  /** Override the auto-dismiss delay (ms). 0 = sticky. */
  duration?: number;
}

// Errors linger longer than successes — they usually carry something to read.
const DEFAULT_DURATIONS: Record<Toast["type"], number> = {
  success: 5000,
  info: 5000,
  error: 9000,
};

let nextId = 0;
const timers = new Map<number, ReturnType<typeof setTimeout>>();

export const toasts = writable<Toast[]>([]);

export function addToast(
  type: Toast["type"],
  title: string,
  message?: string,
  opts?: ToastOptions,
): number {
  const id = nextId++;
  const duration = opts?.duration ?? DEFAULT_DURATIONS[type];
  toasts.update((t) => [...t, { id, type, title, message, action: opts?.action, duration }]);
  if (duration > 0) scheduleRemoval(id, duration);
  return id;
}

function scheduleRemoval(id: number, ms: number) {
  clearTimeout(timers.get(id));
  timers.set(
    id,
    setTimeout(() => removeToast(id), ms),
  );
}

/** Pause auto-dismiss (e.g. while the pointer hovers the toast). */
export function pauseToast(id: number) {
  const timer = timers.get(id);
  if (timer) {
    clearTimeout(timer);
    timers.delete(id);
  }
}

/** Resume auto-dismiss using the toast's configured duration. */
export function resumeToast(id: number) {
  const toast = get(toasts).find((t) => t.id === id);
  if (toast && toast.duration > 0) scheduleRemoval(id, toast.duration);
}

export function removeToast(id: number) {
  clearTimeout(timers.get(id));
  timers.delete(id);
  toasts.update((t) => t.filter((toast) => toast.id !== id));
}
