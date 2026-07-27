// Central keyboard-shortcut registry.
//
// This replaces a single 55-line `handleKeydown` in App.svelte where each
// shortcut carried its own ad-hoc guard list. Two bugs came directly from
// that: Ctrl+N was unguarded and opened the Add panel *behind* whatever modal
// was up, and the digit shortcuts enumerated four overlay flags but missed
// the captcha dialog.
//
// Here the guard is structural. Escape is not a binding at all -- it runs a
// fixed chain -- and every other binding is suppressed while a blocking layer
// is open unless it opts into that layer's scope.

import { get, writable, type Readable } from "svelte/store";
import { blocking, dismissTop, topScope } from "./overlays.svelte";

export interface Chord {
  /** Compared case-insensitively against KeyboardEvent.key. */
  key: string;
  /** Ctrl on Windows/Linux, Cmd on macOS — matched interchangeably. */
  mod?: boolean;
  shift?: boolean;
  alt?: boolean;
}

export interface Binding {
  chord: Chord;
  run: (e: KeyboardEvent) => void;
  /** i18n key describing the shortcut; ShortcutsDialog renders from these. */
  descKey: string;
  /** Extra runtime condition. */
  when?: () => boolean;
  /** Allow firing while focus is in a text field. Default false. */
  allowInEditable?: boolean;
  /**
   * Scope this binding belongs to. "global" bindings are suppressed while a
   * modal/critical layer is open; a binding whose scope matches the topmost
   * layer's scope still fires.
   */
  scope?: string;
}

const registry = writable<Binding[]>([]);

/** All active bindings, for rendering the shortcuts help. */
export const bindings: Readable<Binding[]> = { subscribe: registry.subscribe };

/** Register bindings; returns an unregister function. */
export function registerBindings(items: Binding[]): () => void {
  registry.update((b) => [...b, ...items]);
  return () => {
    registry.update((b) => b.filter((x) => !items.includes(x)));
  };
}

/** Extra Escape handlers, tried after layers. First to return true wins. */
const escapeFallbacks: Array<() => boolean> = [];

export function registerEscapeFallback(fn: () => boolean): () => void {
  escapeFallbacks.push(fn);
  return () => {
    const i = escapeFallbacks.indexOf(fn);
    if (i !== -1) escapeFallbacks.splice(i, 1);
  };
}

function isEditableTarget(e: KeyboardEvent): boolean {
  const el = e.target as HTMLElement | null;
  if (!el) return false;
  const tag = el.tagName;
  return tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT" || el.isContentEditable;
}

function matches(chord: Chord, e: KeyboardEvent): boolean {
  if (e.key.toLowerCase() !== chord.key.toLowerCase()) return false;
  const mod = e.ctrlKey || e.metaKey;
  if (!!chord.mod !== mod) return false;
  if (!!chord.shift !== e.shiftKey) return false;
  if (!!chord.alt !== e.altKey) return false;
  return true;
}

export function formatChord(chord: Chord): string {
  const mac = typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform ?? "");
  const parts: string[] = [];
  if (chord.mod) parts.push(mac ? "⌘" : "Ctrl");
  if (chord.alt) parts.push(mac ? "⌥" : "Alt");
  if (chord.shift) parts.push("Shift");
  parts.push(chord.key.length === 1 ? chord.key.toUpperCase() : chord.key);
  return parts.join(mac ? "" : "+");
}

/** Install the single window listener. Returns a teardown function. */
export function startKeymap(): () => void {
  function onKeydown(e: KeyboardEvent) {
    // Escape is a chain, not a binding: topmost layer first, then whatever
    // fallbacks are registered (clearing a selection, for instance).
    if (e.key === "Escape") {
      if (dismissTop()) {
        e.preventDefault();
        return;
      }
      for (const fn of [...escapeFallbacks].reverse()) {
        if (fn()) {
          e.preventDefault();
          return;
        }
      }
      return;
    }

    const editable = isEditableTarget(e);
    const isBlocked = get(blocking);
    const scope = get(topScope);

    for (const b of get(registry)) {
      if (!matches(b.chord, e)) continue;
      if (editable && !b.allowInEditable) continue;
      // A blocking layer swallows every binding except its own scope.
      if (isBlocked && b.scope !== scope) continue;
      if (b.when && !b.when()) continue;
      e.preventDefault();
      b.run(e);
      return;
    }
  }

  window.addEventListener("keydown", onKeydown);
  return () => window.removeEventListener("keydown", onKeydown);
}
