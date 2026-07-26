<script lang="ts">
  import { onMount } from "svelte";
  import {
    getPlugins, setPluginEnabled, checkUpdates, applyCoreUpdate,
    listAvailablePlugins, installPlugin, updatePlugin,
    type Plugin, type MarketplaceEntry, type UpdateCheck,
  } from "../lib/api";
  import { addToast } from "../lib/toast";
  import { locale, tr } from "../lib/i18n";
  import SkeletonCard from "../components/SkeletonCard.svelte";
  import Card from "@amigo/ui/components/Card.svelte";
  import Button from "@amigo/ui/components/Button.svelte";
  import Chip from "@amigo/ui/components/Chip.svelte";
  import Banner from "@amigo/ui/components/Banner.svelte";
  import EmptyState from "@amigo/ui/components/EmptyState.svelte";
  import Icon from "@amigo/ui/components/Icon.svelte";

  let plugins = $state<Plugin[]>([]);
  let updateInfo = $state<UpdateCheck | null>(null);
  let loading = $state(true);
  let error = $state(false);
  let updating = $state(false);
  let busyId = $state<string | null>(null);

  // The marketplace said "coming soon" while the backend had shipped the
  // whole thing: GET /updates/plugins/available and
  // POST /updates/plugins/{id}/install have existed all along.
  let market = $state<MarketplaceEntry[]>([]);
  let marketState = $state<"loading" | "ready" | "unavailable">("loading");
  let marketQuery = $state("");

  let updatableIds = $derived(new Set((updateInfo?.plugins ?? []).map((p) => p.id)));

  let filteredMarket = $derived.by(() => {
    if (!marketQuery) return market;
    const q = marketQuery.toLowerCase();
    return market.filter(
      (p) =>
        p.name.toLowerCase().includes(q) ||
        p.id.toLowerCase().includes(q) ||
        p.description.toLowerCase().includes(q) ||
        p.tags.some((t) => t.toLowerCase().includes(q)),
    );
  });

  onMount(async () => {
    try {
      plugins = await getPlugins();
    } catch {
      error = true;
    }
    loading = false;

    try {
      updateInfo = await checkUpdates();
    } catch {
      // Update check is optional — never block the page on it.
    }

    try {
      market = await listAvailablePlugins();
      marketState = "ready";
    } catch {
      // A registry that is unreachable or not configured is normal for an
      // offline install; say so rather than pretending the feature is absent.
      marketState = "unavailable";
    }
  });

  async function handleCoreUpdate() {
    updating = true;
    try {
      await applyCoreUpdate();
      addToast("success", tr($locale, "plugins.update_started"));
    } catch (e) {
      addToast("error", tr($locale, "plugins.update_failed"), e instanceof Error ? e.message : undefined);
    } finally {
      updating = false;
    }
  }

  async function handleToggle(plugin: Plugin) {
    if (busyId) return;
    busyId = plugin.id;
    const enabled = !plugin.enabled;
    try {
      await setPluginEnabled(plugin.id, enabled);
      plugins = plugins.map((p) => (p.id === plugin.id ? { ...p, enabled } : p));
      addToast("info", tr($locale, enabled ? "plugins.enabled_toast" : "plugins.disabled_toast"), plugin.name);
    } catch {
      addToast("error", tr($locale, "plugins.toggle_failed"), plugin.name);
    } finally {
      busyId = null;
    }
  }

  async function handleInstall(entry: MarketplaceEntry) {
    if (busyId) return;
    busyId = entry.id;
    try {
      await installPlugin(entry.id);
      addToast("success", tr($locale, "plugins.installed_toast"), entry.name);
      plugins = await getPlugins();
      market = market.map((p) => (p.id === entry.id ? { ...p, installed: true } : p));
    } catch (e) {
      addToast("error", tr($locale, "plugins.install_failed"), e instanceof Error ? e.message : entry.name);
    } finally {
      busyId = null;
    }
  }

  async function handleUpdate(plugin: Plugin) {
    if (busyId) return;
    busyId = plugin.id;
    try {
      await updatePlugin(plugin.id);
      addToast("success", tr($locale, "plugins.plugin_updated"), plugin.name);
      plugins = await getPlugins();
      updateInfo = await checkUpdates();
    } catch (e) {
      addToast("error", tr($locale, "plugins.update_failed"), e instanceof Error ? e.message : plugin.name);
    } finally {
      busyId = null;
    }
  }
</script>

<div class="space-y-6">
  {#if updateInfo?.core?.update_available}
    <Banner
      tone="info"
      title={tr($locale, "plugins.core_update")}
    >
      <span style="font-family: var(--font-mono)">
        v{updateInfo.core.current_version} &rarr; v{updateInfo.core.latest_version}
      </span>
      {#snippet actions()}
        <Button variant="solid" size="sm" loading={updating} onclick={handleCoreUpdate}>
          {tr($locale, "plugins.update")}
        </Button>
      {/snippet}
    </Banner>
  {/if}

  <section>
    <h3 class="text-lg font-bold mb-4" style="color: var(--text-primary)">
      {tr($locale, "plugins.installed")}
    </h3>

    {#if loading}
      <div class="grid gap-3 sm:grid-cols-2"><SkeletonCard count={2} /></div>
    {:else if error}
      <Banner tone="danger" role="alert">{tr($locale, "plugins.load_failed")}</Banner>
    {:else if plugins.length === 0}
      <Card padding="none">
        <EmptyState size="sm" icon="puzzle" title={tr($locale, "plugins.none")} />
      </Card>
    {:else}
      <ul class="grid gap-3 sm:grid-cols-2 m-0 p-0" style="list-style: none">
        {#each plugins as plugin (plugin.id)}
          <li>
            <Card>
              <div class="flex items-start justify-between gap-3">
                <div class="min-w-0">
                  <h4 class="font-semibold text-sm m-0" style="color: var(--text-primary)">{plugin.name}</h4>
                  <p class="text-xs m-0" style="font-family: var(--font-mono); color: var(--text-secondary)">
                    v{plugin.version}
                  </p>
                </div>
                <Chip
                  size="sm"
                  selected={plugin.enabled}
                  disabled={busyId === plugin.id}
                  aria-pressed={plugin.enabled}
                  aria-label="{plugin.name}: {plugin.enabled ? tr($locale, 'plugins.active') : tr($locale, 'plugins.disabled')}"
                  onclick={() => handleToggle(plugin)}
                >
                  {plugin.enabled ? tr($locale, "plugins.active") : tr($locale, "plugins.disabled")}
                </Chip>
              </div>
              <p class="text-xs mt-2 truncate m-0" style="font-family: var(--font-mono); color: var(--text-secondary)">
                {plugin.url_pattern}
              </p>
              {#if updatableIds.has(plugin.id)}
                <div class="mt-3">
                  <Button
                    variant="soft"
                    size="sm"
                    iconLeft="refresh"
                    loading={busyId === plugin.id}
                    onclick={() => handleUpdate(plugin)}
                  >
                    {tr($locale, "plugins.update_available")}
                  </Button>
                </div>
              {/if}
            </Card>
          </li>
        {/each}
      </ul>
    {/if}
  </section>

  <section>
    <div class="flex items-center justify-between gap-3 mb-4 flex-wrap">
      <h3 class="text-lg font-bold m-0" style="color: var(--text-primary)">
        {tr($locale, "plugins.marketplace")}
      </h3>
      {#if marketState === "ready" && market.length > 0}
        <div
          class="flex items-center gap-2 rounded-lg px-3 min-w-[12rem]"
          style="background: var(--bg-surface); border: 1px solid var(--border-color); min-height: 36px"
        >
          <Icon name="search" size={14} />
          <input
            type="search"
            bind:value={marketQuery}
            placeholder={tr($locale, "plugins.search")}
            aria-label={tr($locale, "plugins.search")}
            class="flex-1 bg-transparent text-sm outline-none min-w-0"
            style="color: var(--text-primary)"
          />
        </div>
      {/if}
    </div>

    {#if marketState === "loading"}
      <div class="grid gap-3 sm:grid-cols-2"><SkeletonCard count={2} /></div>
    {:else if marketState === "unavailable"}
      <Card padding="none">
        <EmptyState
          size="sm"
          icon="globe"
          title={tr($locale, "plugins.registry_unavailable")}
          description={tr($locale, "plugins.registry_unavailable_hint")}
        />
      </Card>
    {:else if filteredMarket.length === 0}
      <Card padding="none">
        <EmptyState size="sm" icon="search" title={tr($locale, "plugins.no_results")} />
      </Card>
    {:else}
      <ul class="grid gap-3 sm:grid-cols-2 m-0 p-0" style="list-style: none">
        {#each filteredMarket as entry (entry.id)}
          <li>
            <Card>
              <div class="flex items-start justify-between gap-3">
                <div class="min-w-0">
                  <h4 class="font-semibold text-sm m-0" style="color: var(--text-primary)">{entry.name}</h4>
                  <p class="text-xs m-0" style="font-family: var(--font-mono); color: var(--text-secondary)">
                    v{entry.version} &middot; {entry.author}
                  </p>
                </div>
                {#if entry.installed}
                  <span class="text-xs font-semibold shrink-0" style="color: var(--success-ink)">
                    {tr($locale, "plugins.installed_badge")}
                  </span>
                {:else}
                  <Button
                    variant="soft"
                    size="sm"
                    loading={busyId === entry.id}
                    disabled={busyId !== null}
                    onclick={() => handleInstall(entry)}
                  >
                    {tr($locale, "plugins.install")}
                  </Button>
                {/if}
              </div>
              <p class="text-xs mt-2 m-0" style="color: var(--text-secondary)">{entry.description}</p>
              {#if entry.tags.length}
                <div class="flex gap-1 mt-2 flex-wrap">
                  {#each entry.tags as tag}
                    <span
                      class="text-xs px-1.5 py-0.5 rounded"
                      style="background: var(--bg-surface-2); color: var(--text-secondary)"
                    >{tag}</span>
                  {/each}
                </div>
              {/if}
            </Card>
          </li>
        {/each}
      </ul>
    {/if}
  </section>
</div>
