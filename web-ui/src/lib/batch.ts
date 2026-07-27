// Batch execution with progress and cancellation.
//
// Batch actions used to be a bare `for (const id of selected) await fn(id)`
// with no progress indicator and no disabled state: pausing 50 downloads
// froze the batch bar for tens of seconds while the buttons stayed clickable,
// so a second run could be queued on top of the first.
//
// The server now has bulk endpoints, so the normal path is one request. The
// per-id runner below stays as the fallback for an older server, chosen once
// per session by feature-detecting a 404/405 -- a freshly built web UI is
// often served against a daemon that has not been updated yet.

import { writable } from "svelte/store";
import { ApiError, batchUpdateDownloads, batchDeleteDownloads } from "./api";

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

/** Whether the server understands /downloads/batch. null = not yet probed. */
let bulkSupported: boolean | null = null;

function isMissingEndpoint(e: unknown): boolean {
  return e instanceof ApiError && (e.status === 404 || e.status === 405);
}

export interface BulkResult {
  ok: string[];
  failed: string[];
  cancelled: boolean;
}

/**
 * Run a bulk action, preferring the single-request endpoint.
 *
 * `perId` is the fallback used against a server that predates the bulk
 * routes; it is also what makes the operation cancellable, which a single
 * request is not.
 */
export async function runBulk(
  ids: string[],
  bulk: (ids: string[]) => Promise<{ ok: string[]; failed: { id: string }[] }>,
  perId: (id: string) => Promise<unknown>,
  signal?: AbortSignal,
): Promise<BulkResult> {
  if (bulkSupported !== false) {
    batchProgress.set({ total: ids.length, done: 0, failed: 0, running: true });
    try {
      const res = await bulk(ids);
      bulkSupported = true;
      batchProgress.set(null);
      return { ok: res.ok, failed: res.failed.map((f) => f.id), cancelled: false };
    } catch (e) {
      batchProgress.set(null);
      if (!isMissingEndpoint(e)) throw e;
      bulkSupported = false;
      // Fall through to the per-id path below.
    }
  }

  const res = await runBatch(ids, perId, { concurrency: 4, signal });
  return { ok: res.ok, failed: res.failed, cancelled: res.cancelled };
}

export const bulkUpdate = (ids: string[], action: "pause" | "resume" | "retry",
                           perId: (id: string) => Promise<unknown>, signal?: AbortSignal) =>
  runBulk(ids, (batch) => batchUpdateDownloads(batch, action), perId, signal);

export const bulkDelete = (ids: string[], perId: (id: string) => Promise<unknown>,
                           signal?: AbortSignal) =>
  runBulk(ids, batchDeleteDownloads, perId, signal);

/** Test-only: reset the feature-detection cache. */
export function resetBulkSupport(): void {
  bulkSupported = null;
}
