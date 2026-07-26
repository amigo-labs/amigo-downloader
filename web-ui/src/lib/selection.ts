// Multi-selection model for the download list.
//
// Before this, selection had exactly two entry points: a "select all" chip in
// the toolbar and a bare `<div onclick>` acting as a checkbox -- no role, no
// tabindex, no aria-checked -- which made batch selection completely
// unreachable by keyboard. It was also a *mode*: while anything was selected,
// Space/Enter on a card opened the detail panel instead of toggling, so there
// was no way to deselect an item without a mouse.
//
// Here selection is not a mode. The checkbox is a real form control that is
// always present, and the row's title button always opens the detail panel.
// Shift/Ctrl click, Ctrl+A and Escape follow file-manager convention.

import { get, writable, derived, type Readable } from "svelte/store";
import { selectedIds } from "./stores";

/**
 * Anchor for Shift-click range selection, tracked by id rather than index:
 * the list re-sorts on every WebSocket progress tick, so an index captured
 * one frame ago can point at a different row by the time it is used.
 */
const anchorId = writable<string | null>(null);

export const selectionCount: Readable<number> = derived(selectedIds, ($s) => $s.size);
export const hasSelection: Readable<boolean> = derived(selectedIds, ($s) => $s.size > 0);

export function isSelected(id: string): boolean {
  return get(selectedIds).has(id);
}

export function toggle(id: string): void {
  selectedIds.update((s) => {
    const next = new Set(s);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    return next;
  });
  anchorId.set(id);
}

export function setSelection(ids: string[]): void {
  selectedIds.set(new Set(ids));
  anchorId.set(ids[ids.length - 1] ?? null);
}

export function clear(): void {
  selectedIds.set(new Set());
  anchorId.set(null);
}

/** Escape handler: returns true only if there was something to clear. */
export function clearIfAny(): boolean {
  if (get(selectedIds).size === 0) return false;
  clear();
  return true;
}

/**
 * Select the inclusive range between the anchor and `id`, in the order
 * currently displayed. Falls back to a plain toggle when the anchor is gone
 * (deleted elsewhere, or filtered out of the current view).
 */
export function extendTo(id: string, orderedIds: string[]): void {
  const from = orderedIds.indexOf(get(anchorId) ?? "");
  const to = orderedIds.indexOf(id);
  if (from === -1 || to === -1) {
    toggle(id);
    return;
  }
  const [lo, hi] = from <= to ? [from, to] : [to, from];
  selectedIds.update((s) => {
    const next = new Set(s);
    for (const rangeId of orderedIds.slice(lo, hi + 1)) next.add(rangeId);
    return next;
  });
}

/** Ctrl+A: select everything visible, or clear if it is already all selected. */
export function toggleAll(orderedIds: string[]): void {
  const current = get(selectedIds);
  const allSelected =
    orderedIds.length > 0 && orderedIds.every((id) => current.has(id));
  if (allSelected) clear();
  else setSelection(orderedIds);
}

/** Dispatch a click on a row checkbox, honouring Shift / Ctrl / Cmd. */
export function handleCheckboxClick(
  id: string,
  event: MouseEvent | KeyboardEvent,
  orderedIds: string[],
): void {
  if (event.shiftKey) extendTo(id, orderedIds);
  else toggle(id);
}

/**
 * Drop ids that no longer exist. Without this, deleting a download in another
 * tab leaves a phantom count in the batch bar and batch actions fire against
 * ids the server has already forgotten.
 */
export function pruneSelection(existingIds: Iterable<string>): void {
  const alive = new Set(existingIds);
  selectedIds.update((s) => {
    if ([...s].every((id) => alive.has(id))) return s;
    return new Set([...s].filter((id) => alive.has(id)));
  });
  const a = get(anchorId);
  if (a && !alive.has(a)) anchorId.set(null);
}
