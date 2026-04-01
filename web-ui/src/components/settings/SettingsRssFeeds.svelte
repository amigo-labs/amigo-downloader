<script lang="ts">
  import { onMount } from "svelte";
  import { getRssFeeds, addRssFeed, deleteRssFeed } from "../../lib/api";
  import { addToast } from "../../lib/toast";

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
    } catch { /* offline */ }
  });

  function resetForm() {
    name = ""; url = ""; category = ""; intervalMinutes = 15;
  }

  async function handleAdd() {
    if (!name.trim() || !url.trim()) return;
    saving = true;
    try {
      const feed = await addRssFeed({
        name: name.trim(), url: url.trim(),
        category: category.trim(), interval_minutes: intervalMinutes,
      });
      feeds = [...feeds, feed];
      addToast("success", "RSS feed added");
      resetForm();
      showAddForm = false;
    } catch (e: any) {
      addToast("error", e.message || "Failed to add feed");
    } finally {
      saving = false;
    }
  }

  async function handleDelete(id: string) {
    try {
      await deleteRssFeed(id);
      feeds = feeds.filter((f) => f.id !== id);
      addToast("success", "Feed removed");
    } catch {
      addToast("error", "Failed to remove feed");
    }
  }
</script>

<section>
  <div class="flex items-center justify-between mb-4">
    <div>
      <h3 class="text-lg font-bold" style="color: var(--text-primary)">RSS Feeds</h3>
      <p class="text-xs mt-0.5" style="color: var(--text-secondary)">
        Monitor RSS/Atom feeds for new NZB links. New items are automatically imported.
      </p>
    </div>
    <button
      onclick={() => (showAddForm = !showAddForm)}
      class="px-3 py-1.5 rounded-lg text-xs font-semibold"
      style="background: var(--neon-primary); color: var(--bg-deep)"
    >
      {showAddForm ? "Cancel" : "+ Add Feed"}
    </button>
  </div>

  {#if showAddForm}
    <div class="rounded-xl p-5 space-y-3 mb-4" style="background: var(--bg-surface); border: 1px solid var(--border-color)">
      <div>
        <label class="block text-xs font-medium mb-1" style="color: var(--text-secondary)">Feed Name</label>
        <input type="text" bind:value={name} placeholder="My NZB Feed"
          class="w-full rounded-lg px-3 py-2 text-sm"
          style="background: var(--bg-surface-2); border: 1px solid var(--border-color); color: var(--text-primary)" />
      </div>
      <div>
        <label class="block text-xs font-medium mb-1" style="color: var(--text-secondary)">Feed URL</label>
        <input type="url" bind:value={url} placeholder="https://example.com/feed.xml"
          class="w-full rounded-lg px-3 py-2 text-sm"
          style="font-family: var(--font-mono);background: var(--bg-surface-2); border: 1px solid var(--border-color); color: var(--text-primary)" />
      </div>
      <div class="grid grid-cols-2 gap-3">
        <div>
          <label class="block text-xs font-medium mb-1" style="color: var(--text-secondary)">Category</label>
          <input type="text" bind:value={category} placeholder="tv-shows (optional)"
            class="w-full rounded-lg px-3 py-2 text-sm"
            style="background: var(--bg-surface-2); border: 1px solid var(--border-color); color: var(--text-primary)" />
        </div>
        <div>
          <label class="block text-xs font-medium mb-1" style="color: var(--text-secondary)">Check Interval (min)</label>
          <input type="number" bind:value={intervalMinutes} min="5"
            class="w-full rounded-lg px-3 py-2 text-sm"
            style="font-family: var(--font-mono);background: var(--bg-surface-2); border: 1px solid var(--border-color); color: var(--text-primary)" />
        </div>
      </div>
      <div class="flex gap-2 pt-2">
        <button onclick={handleAdd} disabled={saving || !name.trim() || !url.trim()}
          class="px-4 py-2 rounded-lg text-sm font-semibold disabled:opacity-50"
          style="background: var(--neon-primary); color: var(--bg-deep)">{saving ? "Saving..." : "Add Feed"}</button>
        <button onclick={() => { resetForm(); showAddForm = false; }}
          class="px-4 py-2 rounded-lg text-sm"
          style="color: var(--text-secondary)">Cancel</button>
      </div>
    </div>
  {/if}

  {#if feeds.length === 0 && !showAddForm}
    <div class="rounded-xl p-8 text-center" style="background: var(--bg-surface); border: 1px solid var(--border-color)">
      <p class="text-sm" style="color: var(--text-secondary)">
        No RSS feeds configured. Add a feed to automatically import NZBs.
      </p>
    </div>
  {/if}

  {#each feeds as feed (feed.id)}
    <div class="rounded-xl p-4 flex items-center justify-between gap-3 mb-2" style="background: var(--bg-surface); border: 1px solid var(--border-color)">
      <div class="min-w-0 flex-1">
        <p class="font-semibold text-sm truncate" style="color: var(--text-primary)">{feed.name}</p>
        <p class="text-xs truncate" style="font-family: var(--font-mono);color: var(--text-secondary)">{feed.url}</p>
        <div class="flex gap-3 text-[10px] mt-0.5" style="color: var(--text-secondary)">
          {#if feed.category}<span>Category: {feed.category}</span>{/if}
          <span>Every {feed.interval_minutes}m</span>
          {#if feed.last_error}<span style="color: var(--neon-accent)">{feed.last_error}</span>{/if}
        </div>
      </div>
      <button onclick={() => handleDelete(feed.id)}
        class="px-2.5 py-1.5 rounded-lg text-xs shrink-0"
        style="color: var(--neon-accent); border: 1px solid color-mix(in srgb, var(--neon-accent) 20%, transparent)">Delete</button>
    </div>
  {/each}
</section>
