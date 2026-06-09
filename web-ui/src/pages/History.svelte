<script lang="ts">
  import { onMount } from "svelte";
  import { getHistory, formatBytes, formatRelativeTime } from "../lib/api";
  import { locale, tr } from "../lib/i18n";
  import SkeletonCard from "../components/SkeletonCard.svelte";

  let history = $state<any[]>([]);
  let loading = $state(true);
  let error = $state(false);

  onMount(async () => {
    try {
      history = await getHistory();
    } catch (e) {
      console.error("Failed to load history:", e);
      error = true;
    }
    loading = false;
  });
</script>

<div class="space-y-4">
  {#if loading}
    <SkeletonCard count={3} />
  {:else if error}
    <div class="rounded-xl p-4" style="background: color-mix(in srgb, var(--neon-accent) 8%, transparent); border: 1px solid color-mix(in srgb, var(--neon-accent) 20%, transparent)">
      <p class="text-sm" style="color: var(--neon-accent)">{tr($locale, "history.load_failed")}</p>
    </div>
  {:else if history.length === 0}
    <div class="flex flex-col items-center justify-center py-20">
      <img src="/amigo-logo.png" alt="" width="48" height="48" class="rounded-lg opacity-30" />
      <p class="mt-4 text-sm" style="color: var(--text-secondary)">{tr($locale, "history.empty")}</p>
      <p class="text-xs mt-1" style="color: var(--text-secondary); opacity: 0.5">{tr($locale, "history.empty_hint")}</p>
    </div>
  {:else}
    <div class="space-y-2">
      {#each history as item}
        <div
          class="download-card flex items-center gap-4 rounded-xl px-4 py-3"
          style="border: 1px solid var(--border-color)"
        >
          <div class="flex-1 min-w-0">
            <p class="font-medium truncate text-sm" style="color: var(--text-primary)">{item.filename || item.url}</p>
            <p class="text-xs" style="color: var(--text-secondary)">
              {item.filesize ? formatBytes(item.filesize) : "—"} &middot; {formatRelativeTime(item.created_at)}
            </p>
          </div>
          <!-- Fix M9: use neon-success instead of hardcoded text-green-500 -->
          <span class="text-xs font-semibold" style="color: var(--neon-success)">{tr($locale, "history.completed")}</span>
        </div>
      {/each}
    </div>
  {/if}
</div>
