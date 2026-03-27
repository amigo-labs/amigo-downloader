import { writable } from "svelte/store";

export interface Toast {
  id: number;
  type: "success" | "error" | "info";
  title: string;
  message?: string;
}

let nextId = 0;

export const toasts = writable<Toast[]>([]);

export function addToast(type: Toast["type"], title: string, message?: string) {
  const id = nextId++;
  toasts.update((t) => [...t, { id, type, title, message }]);

  // Auto-remove after 5s
  setTimeout(() => removeToast(id), 5000);
}

export function removeToast(id: number) {
  toasts.update((t) => t.filter((toast) => toast.id !== id));
}
