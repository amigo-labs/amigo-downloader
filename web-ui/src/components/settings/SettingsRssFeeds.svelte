<script lang="ts">
  import { onMount } from "svelte";
  import { getRssFeeds, addRssFeed, deleteRssFeed } from "../../lib/api";
  import { addToast } from "../../lib/toast";
  import { locale, tr } from "../../lib/i18n";

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
      addToast("success", tr($locale, "rss.added"));
      resetForm();
      showAddForm = false;
    } catch (e: any) {
      addToast("error", e.message || tr($locale, "rss.add_failed"));
    } finally {
      saving = false;
    }
  }

  async function handleDelete(id: string) {
    try {
      await deleteRssFeed(id);
      feeds = feeds.filter((f) => f.id !== id);
      addToast("success", tr($locale, "rss.removed"));
    } catch {
      addToast("error", tr($locale, "rss.remove_failed"));
    }
  }
</script>

<section>
  <div class="flex items-center justify-between mb-4">
    <div>
      <h3 class="text-lg font-bold" style="color: var(--text-primary)">{tr($locale, "rss.title")}</h3>
      <p class="text-xs mt-0.5" style="color: var(--text-secondary)">
        {tr($locale, "rss.hint")}
      </p>
    </div>
    <button
      onclick={() => (showAddForm = !showAddForm)}
      class="px-3 py-1.5 rounded-lg text-xs font-semibold"
      style="background: var(--neon-primary); color: var(--bg-deep)"
    >
      {showAddForm ? tr($locale, "common.cancel") : tr($locale, "rss.add")}
    </button>
  </div>

  {#if showAddForm}
    <div class="rounded-xl p-5 space-y-3 mb-4" style="background: var(--bg-surface); border: 1px solid var(--border-color)">
      <div>
        <label class="block text-xs font-medium mb-1" style="color: var(--text-secondary)">{tr($locale, "rss.name")}</label>
        <input type="text" bind:value={name} placeholder="My NZB Feed"
          class="w-full rounded-lg px-3 py-2 text-sm"
          style="background: var(--bg-surface-2); border: 1px solid var(--border-color); color: var(--text-primary)" />
      </div>
      <div>
        <label class="block text-xs font-medium mb-1" style="color: var(--text-secondary)">{tr($locale, "rss.url")}</label>
        <input type="url" bind:value={url} placeholder="https://example.com/feed.xml"
          class="w-full rounded-lg px-3 py-2 text-sm"
          style="font-family: var(--font-mono);background: var(--bg-surface-2); border: 1px solid var(--border-color); color: var(--text-primary)" />
      </div>
      <div class="grid grid-cols-2 gap-3">
        <div>
          <label class="block text-xs font-medium mb-1" style="color: var(--text-secondary)">{tr($locale, "rss.category")}</label>
          <input type="text" bind:value={category} placeholder="tv-shows (optional)"
            class="w-full rounded-lg px-3 py-2 text-sm"
            style="background: var(--bg-surface-2); border: 1px solid var(--border-color); color: var(--text-primary)" />
        </div>
        <div>
          <label class="block text-xs font-medium mb-1" style="color: var(--text-secondary)">{tr($locale, "rss.interval")}</label>
          <input type="number" bind:value={intervalMinutes} min="5"
            class="w-full rounded-lg px-3 py-2 text-sm"
            style="font-family: var(--font-mono);background: var(--bg-surface-2); border: 1px solid var(--border-color); color: var(--text-primary)" />
        </div>
      </div>
      <div class="flex gap-2 pt-2">
        <button onclick={handleAdd} disabled={saving || !name.trim() || !url.trim()}
          class="px-4 py-2 rounded-lg text-sm font-semibold disabled:opacity-50"
          style="background: var(--neon-primary); color: var(--bg-deep)">{saving ? tr($locale, "common.saving") : tr($locale, "rss.save")}</button>
        <button onclick={() => { resetForm(); showAddForm = false; }}
          class="px-4 py-2 rounded-lg text-sm"
          style="color: var(--text-secondary)">{tr($locale, "common.cancel")}</button>
      </div>
    </div>
  {/if}

  {#if feeds.length === 0 && !showAddForm}
    <div class="rounded-xl p-8 text-center" style="background: var(--bg-surface); border: 1px solid var(--border-color)">
      <p class="text-sm" style="color: var(--text-secondary)">
        {tr($locale, "rss.empty")}
      </p>
    </div>
  {/if}

  {#each feeds as feed (feed.id)}
    <div class="rounded-xl p-4 flex items-center justify-between gap-3 mb-2" style="background: var(--bg-surface); border: 1px solid var(--border-color)">
      <div class="min-w-0 flex-1">
        <p class="font-semibold text-sm truncate" style="color: var(--text-primary)">{feed.name}</p>
        <p class="text-xs truncate" style="font-family: var(--font-mono);color: var(--text-secondary)">{feed.url}</p>
        <div class="flex gap-3 text-[10px] mt-0.5" style="color: var(--text-secondary)">
          {#if feed.category}<span>{tr($locale, "rss.category_prefix", { name: feed.category })}</span>{/if}
          <span>{tr($locale, "rss.every", { minutes: feed.interval_minutes })}</span>
          {#if feed.last_error}<span style="color: var(--neon-accent)">{feed.last_error}</span>{/if}
        </div>
      </div>
      <button onclick={() => handleDelete(feed.id)}
        class="px-2.5 py-1.5 rounded-lg text-xs shrink-0"
        style="color: var(--neon-accent); border: 1px solid color-mix(in srgb, var(--neon-accent) 20%, transparent)">{tr($locale, "common.delete")}</button>
    </div>
  {/each}
</section>
