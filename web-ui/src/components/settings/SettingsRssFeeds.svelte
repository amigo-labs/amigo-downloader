<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { getRssFeeds, addRssFeed, deleteRssFeed } from "../../lib/api";
  import { addToast } from "../../lib/toast";
  import { locale, tr } from "../../lib/i18n";
  import Card from "@amigo/ui/components/Card.svelte";
  import Button from "@amigo/ui/components/Button.svelte";
  import Field from "@amigo/ui/components/Field.svelte";
  import Banner from "@amigo/ui/components/Banner.svelte";
  import EmptyState from "@amigo/ui/components/EmptyState.svelte";

  interface RssFeed {
    id: string;
    name: string;
    url: string;
    category?: string | null;
    interval_minutes: number;
    last_error?: string | null;
  }

  let feeds = $state<RssFeed[]>([]);
  let showAddForm = $state(false);
  let saving = $state(false);
  let loading = $state(true);
  // Was `catch { /* offline */ }`, which showed the "no feeds" empty state
  // when the server was simply unreachable.
  let loadError = $state(false);
  let confirmingDelete = $state<string | null>(null);
  let confirmTimer: ReturnType<typeof setTimeout> | undefined;

  let name = $state("");
  let url = $state("");
  let category = $state("");
  let intervalMinutes = $state(15);

  onMount(async () => {
    try {
      feeds = await getRssFeeds();
    } catch {
      loadError = true;
    } finally {
      loading = false;
    }
  });

  onDestroy(() => clearTimeout(confirmTimer));

  function resetForm() {
    name = ""; url = ""; category = ""; intervalMinutes = 15;
  }

  async function handleAdd() {
    if (!name.trim() || !url.trim() || saving) return;
    saving = true;
    try {
      const feed = await addRssFeed({
        name: name.trim(), url: url.trim(),
        category: category.trim(), interval_minutes: intervalMinutes,
      });
      feeds = [...feeds, feed as RssFeed];
      addToast("success", tr($locale, "rss.added"));
      resetForm();
      showAddForm = false;
    } catch (e) {
      addToast("error", e instanceof Error ? e.message : tr($locale, "rss.add_failed"));
    } finally {
      saving = false;
    }
  }

  async function handleDelete(id: string) {
    if (confirmingDelete !== id) {
      confirmingDelete = id;
      clearTimeout(confirmTimer);
      confirmTimer = setTimeout(() => (confirmingDelete = null), 2500);
      return;
    }
    clearTimeout(confirmTimer);
    confirmingDelete = null;
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
  <div class="flex items-start justify-between gap-3 mb-4">
    <div>
      <h3 class="text-lg font-bold" style="color: var(--text-primary)">{tr($locale, "rss.title")}</h3>
      <p class="text-xs mt-0.5" style="color: var(--text-secondary)">{tr($locale, "rss.hint")}</p>
    </div>
    <Button
      variant={showAddForm ? "ghost" : "solid"}
      size="sm"
      iconLeft={showAddForm ? undefined : "plus"}
      onclick={() => (showAddForm = !showAddForm)}
    >
      {showAddForm ? tr($locale, "common.cancel") : tr($locale, "rss.add")}
    </Button>
  </div>

  {#if loadError}
    <div class="mb-4">
      <Banner tone="danger" role="alert">{tr($locale, "rss.load_failed")}</Banner>
    </div>
  {/if}

  {#if showAddForm}
    <Card padding="lg" class="mb-4">
      <div class="grid gap-3">
        <Field label={tr($locale, "rss.name")} bind:value={name} required placeholder="My NZB Feed" />
        <Field label={tr($locale, "rss.url")} type="url" bind:value={url} required mono placeholder="https://example.com/feed.xml" />
        <div class="grid grid-cols-1 sm:grid-cols-2 gap-3">
          <Field label={tr($locale, "rss.category")} bind:value={category} placeholder="tv-shows" hint={tr($locale, "common.optional")} />
          <Field label={tr($locale, "rss.interval")} type="number" bind:value={intervalMinutes} min={5} mono />
        </div>
        <div class="flex gap-2 pt-1">
          <Button variant="solid" onclick={handleAdd} loading={saving} disabled={!name.trim() || !url.trim()}>
            {tr($locale, "rss.save")}
          </Button>
          <Button variant="ghost" onclick={() => { resetForm(); showAddForm = false; }}>
            {tr($locale, "common.cancel")}
          </Button>
        </div>
      </div>
    </Card>
  {/if}

  {#if !loading && !loadError && feeds.length === 0 && !showAddForm}
    <Card padding="none">
      <EmptyState size="sm" icon="rss" title={tr($locale, "rss.empty")} />
    </Card>
  {/if}

  <div class="space-y-2">
    {#each feeds as feed (feed.id)}
      <Card>
        <div class="flex items-center justify-between gap-3">
          <div class="min-w-0 flex-1">
            <p class="font-semibold text-sm truncate" style="color: var(--text-primary)">{feed.name}</p>
            <p class="text-xs truncate" style="font-family: var(--font-mono); color: var(--text-secondary)">{feed.url}</p>
            <div class="flex flex-wrap gap-3 text-xs mt-0.5" style="color: var(--text-secondary)">
              {#if feed.category}<span>{tr($locale, "rss.category_prefix", { name: feed.category })}</span>{/if}
              <span>{tr($locale, "rss.every", { minutes: feed.interval_minutes })}</span>
              {#if feed.last_error}<span style="color: var(--danger-ink)">{feed.last_error}</span>{/if}
            </div>
          </div>
          <Button variant="outline" tone="danger" size="sm" onclick={() => handleDelete(feed.id)}>
            {confirmingDelete === feed.id ? tr($locale, "action.sure") : tr($locale, "common.delete")}
          </Button>
        </div>
      </Card>
    {/each}
  </div>
</section>
