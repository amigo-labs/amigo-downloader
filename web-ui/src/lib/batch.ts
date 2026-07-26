// Bounded-concurrency batch runner with progress and cancellation.
//
// Batch actions used to be a bare `for (const id of selected) await fn(id)`
// with no progress indicator and no disabled state: pausing 50 downloads
// froze the batch bar for tens of seconds while the buttons stayed clickable,
// so a second run could be queued on top of the first.
//
// The server has no batch endpoints (only per-id PATCH/DELETE), so the work
// still happens client-side -- but four at a time, reported, and cancellable.

import { writable } from "svelte/store";

export interface BatchProgress {
  total: number;
  done: number;
  failed: number;
  running: boolean;
}

export const batchProgress = writable<BatchProgress | null>(null);

export interface BatchResult<T> {
  ok: T[];
  failed: T[];
  cancelled: boolean;
}

export async function runBatch<T>(
  items: T[],
  fn: (item: T) => Promise<unknown>,
  opts: { concurrency?: number; signal?: AbortSignal } = {},
): Promise<BatchResult<T>> {
  const concurrency = Math.max(1, opts.concurrency ?? 4);
  const ok: T[] = [];
  const failed: T[] = [];
  let cursor = 0;
  let done = 0;

  batchProgress.set({ total: items.length, done: 0, failed: 0, running: true });

  async function worker() {
    while (cursor < items.length) {
      if (opts.signal?.aborted) return;
      const item = items[cursor++];
      try {
        await fn(item);
        ok.push(item);
      } catch {
        failed.push(item);
      }
      done++;
      batchProgress.set({
        total: items.length,
        done,
        failed: failed.length,
        running: true,
      });
    }
  }

  await Promise.all(Array.from({ length: Math.min(concurrency, items.length) }, worker));

  const cancelled = !!opts.signal?.aborted;
  batchProgress.set(null);
  return { ok, failed, cancelled };
}
