<script lang="ts">
  import { onMount } from "svelte";
  import { getRssFeeds, addRssFeed, deleteRssFeed } from "../lib/api";
  import { addToast } from "../lib/toast";

  let feeds = $state<any[]>([]);
  let showAddForm = $state(false);
  let saving = $state(false);

  let name = $state("");
  let url = $state("");
  let category = $state("");
  let intervalMinutes = $state(15);

  onMount(async () => {
    try {
      feeds = await getRssFeeds();
    } catch {
      // offline
    }
  });

  function resetForm() {
    name = "";
    url = "";
    category = "";
    intervalMinutes = 15;
  }

  async function handleAdd() {
    if (!name.trim() || !url.trim()) return;
    saving = true;
    try {
      const feed = await addRssFeed({
        name: name.trim(),
        url: url.trim(),
        category: category.trim(),
        interval_minutes: intervalMinutes,
      });
      feeds = [...feeds, feed];
      addToast("RSS feed added", "success");
      resetForm();
      showAddForm = false;
    } catch (e: any) {
      addToast(e.message || "Failed to add feed", "error");
    } finally {
      saving = false;
    }
  }

  async function handleDelete(id: string) {
    try {
      await deleteRssFeed(id);
      feeds = feeds.filter((f) => f.id !== id);
      addToast("Feed removed", "success");
    } catch {
      addToast("Failed to remove feed", "error");
    }
  }
</script>

<div class="max-w-2xl space-y-6">
  <div class="flex items-center justify-between">
    <div>
      <h3 class="text-lg font-bold">RSS Feeds</h3>
      <p class="text-xs mt-0.5" style="color: var(--text-secondary-color)">
        Monitor RSS/Atom feeds for new NZB links. New items are automatically imported.
      </p>
    </div>
    <button
      onclick={() => (showAddForm = !showAddForm)}
      class="px-3 py-1.5 rounded-lg text-xs font-semibold text-white"
      style="background: var(--accent-color)"
    >
      {showAddForm ? "Cancel" : "+ Add Feed"}
    </button>
  </div>

  {#if showAddForm}
    <div
      class="rounded-xl p-5 space-y-3"
      style="background: var(--surface-2-color); border: 1px solid var(--border-color)"
    >
      <div>
        <label class="block text-xs font-medium mb-1" style="color: var(--text-secondary-color)">Feed Name</label>
        <input
          type="text"
          bind:value={name}
          placeholder="My NZB Feed"
          class="w-full rounded-lg px-3 py-2 text-sm outline-none"
          style="background: var(--surface-3-color); border: 1px solid var(--border-color); color: var(--text-color)"
        />
      </div>
      <div>
        <label class="block text-xs font-medium mb-1" style="color: var(--text-secondary-color)">Feed URL</label>
        <input
          type="url"
          bind:value={url}
          placeholder="https://example.com/feed.xml"
          class="w-full rounded-lg px-3 py-2 text-sm font-mono outline-none"
          style="background: var(--surface-3-color); border: 1px solid var(--border-color); color: var(--text-color)"
        />
      </div>
      <div class="grid grid-cols-2 gap-3">
        <div>
          <label class="block text-xs font-medium mb-1" style="color: var(--text-secondary-color)">Category</label>
          <input
            type="text"
            bind:value={category}
            placeholder="tv-shows (optional)"
            class="w-full rounded-lg px-3 py-2 text-sm outline-none"
            style="background: var(--surface-3-color); border: 1px solid var(--border-color); color: var(--text-color)"
          />
        </div>
        <div>
          <label class="block text-xs font-medium mb-1" style="color: var(--text-secondary-color)">Check Interval (min)</label>
          <input
            type="number"
            bind:value={intervalMinutes}
            min="5"
            class="w-full rounded-lg px-3 py-2 text-sm font-mono outline-none"
            style="background: var(--surface-3-color); border: 1px solid var(--border-color); color: var(--text-color)"
          />
        </div>
      </div>
      <div class="flex gap-2 pt-2">
        <button
          onclick={handleAdd}
          disabled={saving || !name.trim() || !url.trim()}
          class="px-4 py-2 rounded-lg text-sm font-semibold text-white disabled:opacity-50"
          style="background: var(--accent-color)"
        >
          {saving ? "Saving..." : "Add Feed"}
        </button>
        <button
          onclick={() => { resetForm(); showAddForm = false; }}
          class="px-4 py-2 rounded-lg text-sm"
          style="background: var(--surface-3-color); color: var(--text-secondary-color)"
        >
          Cancel
        </button>
      </div>
    </div>
  {/if}

  {#if feeds.length === 0 && !showAddForm}
    <div
      class="rounded-xl p-8 text-center"
      style="background: var(--surface-2-color); border: 1px solid var(--border-color)"
    >
      <p class="text-sm" style="color: var(--text-secondary-color)">
        No RSS feeds configured. Add a feed to automatically import NZBs.
      </p>
    </div>
  {/if}

  {#each feeds as feed (feed.id)}
    <div
      class="rounded-xl p-4 flex items-center justify-between gap-3"
      style="background: var(--surface-2-color); border: 1px solid var(--border-color)"
    >
      <div class="min-w-0 flex-1">
        <p class="font-semibold text-sm truncate">{feed.name}</p>
        <p class="text-xs font-mono truncate" style="color: var(--text-secondary-color)">
          {feed.url}
        </p>
        <div class="flex gap-3 text-[10px] mt-0.5" style="color: var(--text-secondary-color)">
          {#if feed.category}
            <span>Category: {feed.category}</span>
          {/if}
          <span>Every {feed.interval_minutes}m</span>
          {#if feed.last_check}
            <span>Last check: {feed.last_check}</span>
          {/if}
          {#if feed.last_error}
            <span class="text-red-400">{feed.last_error}</span>
          {/if}
        </div>
      </div>
      <button
        onclick={() => handleDelete(feed.id)}
        class="px-2.5 py-1.5 rounded-lg text-xs text-red-400 hover:bg-red-400/10 shrink-0"
      >
        Delete
      </button>
    </div>
  {/each}
</div>
