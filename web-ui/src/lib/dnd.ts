// Drag-and-drop source discrimination.
//
// The app has two unrelated drag interactions: dropping files/URLs from
// outside the window, and dragging a download within the queue to reorder it.
// They collided badly. DropZone listened on `<svelte:window>` with no source
// check, so starting a card drag raised its full-viewport overlay, which then
// swallowed the dragover/drop events meant for the target row. Reordering
// silently did nothing and the "Drop files here" overlay flashed instead.
//
// Discrimination is threefold and each layer is independently sufficient:
//   1. internal drags carry a private MIME type,
//   2. a module-level flag covers browsers that hide custom types during
//      `dragover` for security reasons,
//   3. file drags are additionally required to advertise the "Files" type.

import { writable, get } from "svelte/store";

export const DND_MIME = "application/x-amigo-download-id";

/** Set for the lifetime of an internal drag; null otherwise. */
export const internalDragId = writable<string | null>(null);

export function isInternalDrag(e: DragEvent): boolean {
  const types = e.dataTransfer?.types;
  if (types && Array.prototype.includes.call(types, DND_MIME)) return true;
  return get(internalDragId) !== null;
}

export function isFileDrag(e: DragEvent): boolean {
  const types = e.dataTransfer?.types;
  const hasFiles = !!types && Array.prototype.includes.call(types, "Files");
  return hasFiles && !isInternalDrag(e);
}

export interface ReorderableOptions {
  id: string;
  enabled: boolean;
  onReorder: (draggedId: string, targetId: string) => void;
}

/**
 * Svelte action for a drag handle. Applied to the grip button, not the whole
 * row: making the entire card draggable broke text selection and contradicted
 * the `cursor-grab` affordance drawn on the grip.
 */
export function dragHandle(node: HTMLElement, options: ReorderableOptions) {
  let opts = options;

  function onDragStart(e: DragEvent) {
    if (!opts.enabled || !e.dataTransfer) return;
    e.dataTransfer.effectAllowed = "move";
    e.dataTransfer.setData(DND_MIME, opts.id);
    // text/plain is a fallback for browsers that drop unknown custom types;
    // DropZone ignores it because isInternalDrag() short-circuits first.
    e.dataTransfer.setData("text/plain", opts.id);
    internalDragId.set(opts.id);
  }

  function onDragEnd() {
    internalDragId.set(null);
  }

  node.setAttribute("draggable", String(opts.enabled));
  node.addEventListener("dragstart", onDragStart);
  node.addEventListener("dragend", onDragEnd);

  return {
    update(next: ReorderableOptions) {
      opts = next;
      node.setAttribute("draggable", String(opts.enabled));
    },
    destroy() {
      node.removeEventListener("dragstart", onDragStart);
      node.removeEventListener("dragend", onDragEnd);
    },
  };
}

/** Svelte action for a row that can receive an internal reorder drop. */
export function dropTarget(node: HTMLElement, options: ReorderableOptions) {
  let opts = options;

  function onDragOver(e: DragEvent) {
    if (!opts.enabled || !isInternalDrag(e)) return;
    e.preventDefault();
    if (e.dataTransfer) e.dataTransfer.dropEffect = "move";
    node.classList.add("drop-target-active");
  }

  function onDragLeave() {
    node.classList.remove("drop-target-active");
  }

  function onDrop(e: DragEvent) {
    node.classList.remove("drop-target-active");
    if (!opts.enabled || !isInternalDrag(e)) return;
    e.preventDefault();
    e.stopPropagation();
    const dragged = e.dataTransfer?.getData(DND_MIME) || get(internalDragId);
    internalDragId.set(null);
    if (!dragged || dragged === opts.id) return;
    opts.onReorder(dragged, opts.id);
  }

  node.addEventListener("dragover", onDragOver);
  node.addEventListener("dragleave", onDragLeave);
  node.addEventListener("drop", onDrop);

  return {
    update(next: ReorderableOptions) {
      opts = next;
    },
    destroy() {
      node.removeEventListener("dragover", onDragOver);
      node.removeEventListener("dragleave", onDragLeave);
      node.removeEventListener("drop", onDrop);
    },
  };
}

/** Move an item within an array by id, returning a new array. */
export function moveById<T extends { id: string }>(
  list: T[],
  fromId: string,
  toId: string,
): T[] {
  const items = [...list];
  const from = items.findIndex((d) => d.id === fromId);
  const to = items.findIndex((d) => d.id === toId);
  if (from === -1 || to === -1 || from === to) return list;
  const [item] = items.splice(from, 1);
  items.splice(to, 0, item);
  return items;
}
