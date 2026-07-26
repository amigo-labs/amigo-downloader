import { describe, it, expect, beforeEach } from "vitest";
import { get } from "svelte/store";
import { selectedIds } from "../src/lib/stores";
import {
  toggle, setSelection, clear, clearIfAny, extendTo, toggleAll,
  isSelected, pruneSelection, handleCheckboxClick,
} from "../src/lib/selection";

const ORDER = ["a", "b", "c", "d", "e"];

beforeEach(() => clear());

describe("toggle", () => {
  it("adds and removes", () => {
    toggle("a");
    expect(isSelected("a")).toBe(true);
    toggle("a");
    expect(isSelected("a")).toBe(false);
  });
});

describe("extendTo (Shift+click)", () => {
  it("selects the inclusive range from the anchor", () => {
    toggle("b");
    extendTo("d", ORDER);
    expect([...get(selectedIds)].sort()).toEqual(["b", "c", "d"]);
  });

  it("works backwards", () => {
    toggle("d");
    extendTo("b", ORDER);
    expect([...get(selectedIds)].sort()).toEqual(["b", "c", "d"]);
  });

  it("degrades to a plain toggle when the anchor is gone", () => {
    // No anchor was ever set, so there is no range to extend.
    extendTo("c", ORDER);
    expect([...get(selectedIds)]).toEqual(["c"]);
  });

  it("degrades when the anchor has left the current view", () => {
    toggle("a");
    extendTo("d", ["b", "c", "d"]);
    expect([...get(selectedIds)].sort()).toEqual(["a", "d"]);
  });
});

describe("toggleAll (Ctrl+A)", () => {
  it("selects everything, then clears on a second press", () => {
    toggleAll(ORDER);
    expect(get(selectedIds).size).toBe(5);
    toggleAll(ORDER);
    expect(get(selectedIds).size).toBe(0);
  });

  it("selects all when only a subset is currently selected", () => {
    setSelection(["a", "b"]);
    toggleAll(ORDER);
    expect(get(selectedIds).size).toBe(5);
  });

  it("does nothing meaningful on an empty list", () => {
    toggleAll([]);
    expect(get(selectedIds).size).toBe(0);
  });
});

describe("clearIfAny (the Escape fallback)", () => {
  it("reports whether it consumed the keypress", () => {
    expect(clearIfAny()).toBe(false);
    toggle("a");
    expect(clearIfAny()).toBe(true);
    expect(get(selectedIds).size).toBe(0);
  });
});

describe("pruneSelection", () => {
  it("drops ids the server no longer knows about", () => {
    setSelection(["a", "b", "c"]);
    pruneSelection(["a", "c"]);
    expect([...get(selectedIds)].sort()).toEqual(["a", "c"]);
  });

  it("keeps the same Set identity when nothing changed", () => {
    setSelection(["a", "b"]);
    const before = get(selectedIds);
    pruneSelection(["a", "b", "z"]);
    expect(get(selectedIds)).toBe(before);
  });
});

describe("handleCheckboxClick", () => {
  it("extends on shift and toggles otherwise", () => {
    toggle("b");
    handleCheckboxClick("d", { shiftKey: true } as MouseEvent, ORDER);
    expect([...get(selectedIds)].sort()).toEqual(["b", "c", "d"]);

    handleCheckboxClick("a", { shiftKey: false } as MouseEvent, ORDER);
    expect([...get(selectedIds)].sort()).toEqual(["a", "b", "c", "d"]);
  });
});
