// Layer manager for every overlay in the app.
//
// Previously each overlay picked its own z-index by hand (z-50, z-[60],
// z-[61], z-[100] twice, z-[110], z-[200], a raw 1000) and the single global
// Escape handler hard-coded an order that was simply wrong: the side panel
// was checked before the dialogs, so Escape closed the panel *behind* an open
// modal. CaptchaDialog was not in the chain at all, which combined with its
// focus trap to make it an inescapable keyboard trap.
//
// Here z-index is *derived* from stack position, so two layers can no longer
// collide, and "topmost wins" is the only dismissal rule there is.

import { writable, derived, get, type Readable } from "svelte/store";

export type LayerKind = "panel" | "modal" | "critical" | "transient";

export interface LayerOptions {
  id: string;
  kind: LayerKind;
  /** Human-readable name, used in debugging and by the shortcut list. */
  label?: string;
  /** false => Escape and backdrop clicks are ignored. Default true. */
  dismissible?: boolean;
  onDismiss?: () => void;
  /** Keymap scope active while this layer is on top (e.g. "palette"). */
  scope?: string;
}

interface Layer extends Required<Pick<LayerOptions, "id" | "kind">> {
  label: string;
  dismissible: boolean;
  onDismiss?: () => void;
  scope?: string;
}

/** Base z-index per kind. Position within the kind adds 2 per layer. */
const BASE: Record<LayerKind, number> = {
  panel: 50,
  modal: 100,
  critical: 150,
  transient: 300,
};

/** Kinds that block the page behind them. */
const BLOCKING: LayerKind[] = ["modal", "critical"];

const stack = writable<Layer[]>([]);

/** The live stack, bottom-first. Exposed for the overlay host and tests. */
export const layerStack: Readable<Layer[]> = { subscribe: stack.subscribe };

/** True while any modal/critical layer is open — used to gate shortcuts. */
export const blocking: Readable<boolean> = derived(stack, ($s) =>
  $s.some((l) => BLOCKING.includes(l.kind)),
);

/** The scope of the topmost layer, or undefined. */
export const topScope: Readable<string | undefined> = derived(
  stack,
  ($s) => $s[$s.length - 1]?.scope,
);

/**
 * Register a layer. Returns a close function; calling it twice is a no-op.
 *
 * Re-registering an id that is already open removes the old entry and appends
 * the new one, so the layer moves to the *top* of the stack rather than
 * keeping its position. That is intentional — a re-open should take focus and
 * Escape priority — and it also keeps hot-reload from stacking duplicates.
 * Anything that depends on stable ordering must not re-register.
 */
export function openLayer(options: LayerOptions): () => void {
  const layer: Layer = {
    id: options.id,
    kind: options.kind,
    label: options.label ?? options.id,
    dismissible: options.dismissible ?? true,
    onDismiss: options.onDismiss,
    scope: options.scope,
  };

  stack.update((s) => [...s.filter((l) => l.id !== layer.id), layer]);

  let closed = false;
  return () => {
    if (closed) return;
    closed = true;
    closeLayer(layer.id);
  };
}

export function closeLayer(id: string): void {
  stack.update((s) => s.filter((l) => l.id !== id));
}

/**
 * Dismiss the topmost layer. Returns false when there is nothing to dismiss
 * or the top layer refuses (dismissible: false) — the caller then falls
 * through to the next Escape behaviour, e.g. clearing a selection.
 */
export function dismissTop(): boolean {
  const s = get(stack);
  const top = s[s.length - 1];
  if (!top) return false;
  if (!top.dismissible) return true; // consumed, but nothing happens
  top.onDismiss?.();
  closeLayer(top.id);
  return true;
}

export function isTop(id: string): boolean {
  const s = get(stack);
  return s.length > 0 && s[s.length - 1].id === id;
}

/** z-index for a layer, derived from its position within its kind. */
export function zOf(id: string): number {
  const s = get(stack);
  const layer = s.find((l) => l.id === id);
  if (!layer) return BASE.panel;
  const within = s.filter((l) => l.kind === layer.kind);
  return BASE[layer.kind] + within.findIndex((l) => l.id === id) * 2;
}

/** Reactive view of a single layer's ordering, for use inside components. */
export function layerState(id: string): Readable<{ z: number; isTop: boolean }> {
  return derived(stack, ($s) => {
    const layer = $s.find((l) => l.id === id);
    if (!layer) return { z: BASE.panel, isTop: false };
    const within = $s.filter((l) => l.kind === layer.kind);
    return {
      z: BASE[layer.kind] + within.findIndex((l) => l.id === id) * 2,
      isTop: $s[$s.length - 1]?.id === id,
    };
  });
}

/** Test-only reset. */
export function resetLayers(): void {
  stack.set([]);
}
