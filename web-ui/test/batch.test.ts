import { describe, it, expect } from "vitest";
import { get } from "svelte/store";
import { runBatch, batchProgress } from "../src/lib/batch";

describe("runBatch", () => {
  it("partitions successes from failures", async () => {
    const res = await runBatch([1, 2, 3, 4], async (n) => {
      if (n % 2 === 0) throw new Error("nope");
    });
    expect(res.ok.sort()).toEqual([1, 3]);
    expect(res.failed.sort()).toEqual([2, 4]);
  });

  it("never exceeds the concurrency limit", async () => {
    let inFlight = 0;
    let peak = 0;
    await runBatch(Array.from({ length: 20 }, (_, i) => i), async () => {
      inFlight++;
      peak = Math.max(peak, inFlight);
      await new Promise((r) => setTimeout(r, 1));
      inFlight--;
    }, { concurrency: 4 });
    expect(peak).toBeLessThanOrEqual(4);
  });

  it("stops early when aborted and reports it", async () => {
    const ctrl = new AbortController();
    let started = 0;
    const res = await runBatch(Array.from({ length: 50 }, (_, i) => i), async () => {
      if (++started === 5) ctrl.abort();
      await new Promise((r) => setTimeout(r, 1));
    }, { concurrency: 2, signal: ctrl.signal });
    expect(res.cancelled).toBe(true);
    expect(started).toBeLessThan(50);
  });

  it("clears the progress store when finished", async () => {
    await runBatch([1, 2], async () => {});
    expect(get(batchProgress)).toBeNull();
  });

  it("handles an empty list without hanging", async () => {
    const res = await runBatch([], async () => {});
    expect(res.ok).toEqual([]);
    expect(get(batchProgress)).toBeNull();
  });
});
