<script lang="ts">
  import { onMount } from "svelte";
  import { getHistory, formatBytes } from "../lib/api";

  let history = $state<any[]>([]);

  onMount(async () => {
    try {
      history = await getHistory();
    } catch { /* offline */ }
  });
</script>

<div class="space-y-4">
  {#if history.length === 0}
    <div class="flex flex-col items-center justify-center py-20 opacity-50">
      <p style="color: var(--text-secondary-color)">No download history yet.</p>
    </div>
  {:else}
    <div class="space-y-2">
      {#each history as item}
        <div
          class="flex items-center gap-4 rounded-xl px-4 py-3"
          style="background: var(--surface-2-color); border: 1px solid var(--border-color)"
        >
          <div class="flex-1 min-w-0">
            <p class="font-medium truncate">{item.filename || item.url}</p>
            <p class="text-xs" style="color: var(--text-secondary-color)">
              {item.filesize ? formatBytes(item.filesize) : "—"} &middot; {item.created_at}
            </p>
          </div>
          <span class="text-xs font-semibold text-green-500">Completed</span>
        </div>
      {/each}
    </div>
  {/if}
</div>
