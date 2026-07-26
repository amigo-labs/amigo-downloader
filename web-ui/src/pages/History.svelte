<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { getHistory, clearHistory, addDownload, formatBytes, formatRelativeTime } from "../lib/api";
  import { addToast } from "../lib/toast";
  import { locale, tr } from "../lib/i18n";
  import { fileIcon, statusInk, statusFill, statusLabelKey, statusCounts, displayName } from "../lib/download";
  import type { Download } from "../lib/stores";
  import SkeletonCard from "../components/SkeletonCard.svelte";
  import Card from "@amigo/ui/components/Card.svelte";
  import Button from "@amigo/ui/components/Button.svelte";
  import IconButton from "@amigo/ui/components/IconButton.svelte";
  import Chip from "@amigo/ui/components/Chip.svelte";
  import Icon from "@amigo/ui/components/Icon.svelte";
  import Banner from "@amigo/ui/components/Banner.svelte";
  import EmptyState from "@amigo/ui/components/EmptyState.svelte";

  // This page used to be a dead end: no search, no filter, every entry
  // labelled "Completed" regardless of its real status, no way to re-download
  // and no way to clear the log even though DELETE /history exists.
  let history = $state<Download[]>([]);
  let loading = $state(true);
  let error = $state(false);
  let query = $state("");
  let filter = $state<"all" | "completed" | "failed">("all");
  let confirmingClear = $state(false);
  let clearing = $state(false);
  let requeueing = $state<string | null>(null);
  let clearTimer: ReturnType<typeof setTimeout> | undefined;

  onMount(async () => {
    try {
      history = await getHistory();
    } catch (e) {
      console.error("Failed to load history:", e);
      error = true;
    }
    loading = false;
  });

  // An async onMount cannot supply a teardown.
  onDestroy(() => clearTimeout(clearTimer));

  let counts = $derived(statusCounts(history));

  let filtered = $derived.by(() =>
    history
      .filter((d) => filter === "all" || d.status === filter)
      .filter((d) => {
        if (!query) return true;
        const q = query.toLowerCase();
        return d.filename?.toLowerCase().includes(q) || d.url.toLowerCase().includes(q);
      }),
  );

  async function requeue(item: Download) {
    if (requeueing) return;
    requeueing = item.id;
    try {
      await addDownload(item.url);
      addToast("success", tr($locale, "history.requeued"), displayName(item));
    } catch {
      addToast("error", tr($locale, "history.requeue_failed"), displayName(item));
    } finally {
      requeueing = null;
    }
  }

  async function handleClear() {
    if (!confirmingClear) {
      confirmingClear = true;
      clearTimer = setTimeout(() => (confirmingClear = false), 2500);
      return;
    }
    clearTimeout(clearTimer);
    confirmingClear = false;
    clearing = true;
    try {
      await clearHistory();
      history = [];
      addToast("info", tr($locale, "history.cleared"));
    } catch {
      addToast("error", tr($locale, "history.clear_failed"));
    } finally {
      clearing = false;
    }
  }
</script>

<div class="space-y-4">
  {#if !loading && !error && history.length > 0}
    <div class="flex gap-2 items-center flex-wrap">
      <div
        class="flex-1 min-w-[12rem] flex items-center gap-2 rounded-lg px-3"
        style="background: var(--bg-surface); border: 1px solid var(--border-color); min-height: 40px"
      >
        <Icon name="search" size={16} />
        <input
          type="search"
          placeholder={tr($locale, "history.search")}
          bind:value={query}
          class="flex-1 bg-transparent text-sm outline-none min-w-0"
          style="color: var(--text-primary)"
          aria-label={tr($locale, "history.search")}
        />
      </div>

      <div class="flex gap-2" role="group" aria-label={tr($locale, "downloads.filter_by_status")}>
        {#each (["all", "completed", "failed"] as const) as f}
          <Chip
            size="sm"
            selected={filter === f}
            count={f === "all" ? history.length : (counts[f] ?? 0)}
            aria-pressed={filter === f}
            onclick={() => (filter = f)}
          >
            {tr($locale, `filter.${f}`)}
          </Chip>
        {/each}
      </div>

      <Button
        variant="outline"
        tone="danger"
        size="sm"
        iconLeft="trash"
        loading={clearing}
        onclick={handleClear}
      >
        {confirmingClear ? tr($locale, "action.sure") : tr($locale, "history.clear")}
      </Button>
    </div>
  {/if}

  {#if loading}
    <SkeletonCard count={3} />
  {:else if error}
    <Banner tone="danger" role="alert">{tr($locale, "history.load_failed")}</Banner>
  {:else if history.length === 0}
    <EmptyState
      image="/amigo-logo.png"
      title={tr($locale, "history.empty")}
      description={tr($locale, "history.empty_hint")}
    />
  {:else if filtered.length === 0}
    <EmptyState size="sm" icon="search" title={tr($locale, "empty.search")} />
  {:else}
    <ul class="grid gap-2 m-0 p-0" style="list-style: none">
      {#each filtered as item (item.id)}
        <li>
          <Card padding="sm">
            <div class="flex items-center gap-3">
              <span style="color: var(--text-muted)"><Icon name={fileIcon(item.filename)} size={16} /></span>
              <div class="flex-1 min-w-0">
                <p class="font-medium truncate text-sm m-0" style="color: var(--text-primary)">
                  {displayName(item)}
                </p>
                <p class="text-xs m-0" style="color: var(--text-secondary)">
                  {item.filesize ? formatBytes(item.filesize) : "—"} &middot; {formatRelativeTime(item.created_at)}
                </p>
                {#if item.error}
                  <p class="text-xs m-0 truncate" style="color: var(--danger-ink); font-family: var(--font-mono)">
                    {item.error}
                  </p>
                {/if}
              </div>

              <!-- Every row used to read "Completed", including failures. -->
              <span
                class="text-xs font-semibold shrink-0 px-2 py-0.5 rounded-full"
                style="color: {statusInk(item.status)};
                       background: color-mix(in srgb, {statusFill(item.status)} 12%, transparent)"
              >
                {tr($locale, statusLabelKey(item.status))}
              </span>

              <IconButton
                icon="refresh"
                size="sm"
                label={tr($locale, "history.requeue_named", { name: displayName(item) })}
                disabled={requeueing !== null}
                onclick={() => requeue(item)}
              />
            </div>
          </Card>
        </li>
      {/each}
    </ul>
  {/if}
</div>
