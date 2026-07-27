import { describe, it, expect, beforeEach } from "vitest";
import { get } from "svelte/store";
import { DND_MIME, internalDragId, isInternalDrag, isFileDrag, moveById } from "../src/lib/dnd";

function ev(types: string[]): DragEvent {
  return { dataTransfer: { types } } as unknown as DragEvent;
}

beforeEach(() => internalDragId.set(null));

describe("drag source discrimination", () => {
  it("recognises an internal drag by its private MIME type", () => {
    expect(isInternalDrag(ev([DND_MIME, "text/plain"]))).toBe(true);
    expect(isFileDrag(ev([DND_MIME, "text/plain"]))).toBe(false);
  });

  it("recognises a file drag from outside the window", () => {
    expect(isFileDrag(ev(["Files"]))).toBe(true);
    expect(isInternalDrag(ev(["Files"]))).toBe(false);
  });

  it("still knows an internal drag when the browser hides custom types", () => {
    // Chromium withholds custom types during dragover for security; the
    // module flag is what keeps DropZone from hijacking a reorder.
    internalDragId.set("abc");
    expect(isInternalDrag(ev([]))).toBe(true);
    expect(isFileDrag(ev(["Files"]))).toBe(false);
  });

  it("treats a plain text drag as neither", () => {
    expect(isFileDrag(ev(["text/plain"]))).toBe(false);
    expect(isInternalDrag(ev(["text/plain"]))).toBe(false);
  });

  it("tolerates a missing dataTransfer", () => {
    expect(isFileDrag({} as DragEvent)).toBe(false);
    expect(isInternalDrag({} as DragEvent)).toBe(false);
  });
});

describe("moveById", () => {
  const list = [{ id: "a" }, { id: "b" }, { id: "c" }];

  it("moves an item to the target position", () => {
    expect(moveById(list, "a", "c").map((x) => x.id)).toEqual(["b", "c", "a"]);
    expect(moveById(list, "c", "a").map((x) => x.id)).toEqual(["c", "a", "b"]);
  });

  it("returns the original array identity when nothing moves", () => {
    expect(moveById(list, "a", "a")).toBe(list);
    expect(moveById(list, "a", "zzz")).toBe(list);
    expect(moveById(list, "zzz", "a")).toBe(list);
  });
});
