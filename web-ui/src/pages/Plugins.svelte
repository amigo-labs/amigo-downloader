<script lang="ts">
  import { onMount } from "svelte";
  import {
    getPlugins, setPluginEnabled, checkUpdates, applyCoreUpdate,
    listAvailablePlugins, installPlugin, updatePlugin,
    type Plugin, type MarketplaceEntry, type UpdateCheck, type PluginUpdate,
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

  // Updates for installed plugins, keyed by id (new plugins live in the
  // marketplace section instead).
  let updatesById = $derived(
    new Map(
      (updateInfo?.plugins ?? [])
        .filter((u) => !u.is_new)
        .map((u) => [u.plugin_id, u] as [string, PluginUpdate]),
    ),
  );

  // Install / update waiting for the user to confirm the hosts it may reach.
  // Installing always asks; an update asks only when it widens access.
  let reviewId = $state<string | null>(null);

  function isUnscoped(p: Plugin): boolean {
    return p.permissions?.domains == null;
  }

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
    reviewId = null;
    busyId = entry.id;
    try {
      await installPlugin(entry.id, true);
      addToast("success", tr($locale, "plugins.installed_toast"), entry.name);
      plugins = await getPlugins();
      market = market.map((p) => (p.id === entry.id ? { ...p, installed: true } : p));
    } catch (e) {
      addToast("error", tr($locale, "plugins.install_failed"), e instanceof Error ? e.message : entry.name);
    } finally {
      busyId = null;
    }
  }

  async function handleUpdate(plugin: Plugin, approve: boolean) {
    if (busyId) return;
    const update = updatesById.get(plugin.id);
    if (update?.requires_approval && !approve) {
      reviewId = plugin.id;
      return;
    }
    reviewId = null;
    busyId = plugin.id;
    try {
      await updatePlugin(plugin.id, approve);
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
              {#if isUnscoped(plugin)}
                <p
                  class="text-xs mt-2 m-0 font-semibold perm-warn"
                  title={tr($locale, "plugins.unscoped_hint")}
                >
                  {tr($locale, "plugins.unscoped")}
                </p>
              {/if}
              {@const update = updatesById.get(plugin.id)}
              {#if update}
                {#if reviewId === plugin.id}
                  <div class="mt-3" role="group" aria-label={tr($locale, "plugins.domains")}>
                    <p class="text-xs m-0 perm-prompt">
                      {update.added_domains?.includes("*")
                        ? tr($locale, "plugins.review_update_unscoped")
                        : tr($locale, "plugins.review_update")}
                    </p>
                    {#if update.added_domains && !update.added_domains.includes("*")}
                      <ul class="text-xs mt-1 mb-0 pl-4 perm-list">
                        {#each update.added_domains as domain}<li>{domain}</li>{/each}
                      </ul>
                    {/if}
                    <div class="flex gap-2 mt-2">
                      <Button variant="solid" size="sm" onclick={() => handleUpdate(plugin, true)}>
                        {tr($locale, "plugins.confirm_update")}
                      </Button>
                      <Button variant="ghost" size="sm" onclick={() => (reviewId = null)}>
                        {tr($locale, "plugins.cancel")}
                      </Button>
                    </div>
                  </div>
                {:else}
                  <div class="mt-3">
                    <Button
                      variant="soft"
                      size="sm"
                      iconLeft="refresh"
                      loading={busyId === plugin.id}
                      onclick={() => handleUpdate(plugin, false)}
                    >
                      {tr($locale, "plugins.update_available")}
                    </Button>
                  </div>
                {/if}
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
                    aria-expanded={reviewId === entry.id}
                    onclick={() => (reviewId = reviewId === entry.id ? null : entry.id)}
                  >
                    {tr($locale, "plugins.install")}
                  </Button>
                {/if}
              </div>
              <p class="text-xs mt-2 m-0" style="color: var(--text-secondary)">{entry.description}</p>
              <p class="text-xs mt-2 m-0 perm-line">
                {tr($locale, "plugins.domains")}:
                {#if entry.domains === null}
                  <span class="font-semibold perm-warn" title={tr($locale, "plugins.unscoped_hint")}>
                    {tr($locale, "plugins.unscoped")}
                  </span>
                {:else if entry.domains.length === 0}
                  {tr($locale, "plugins.no_hosts")}
                {:else}
                  <span class="perm-mono">{entry.domains.join(", ")}</span>
                {/if}
              </p>
              {#if reviewId === entry.id && !entry.installed}
                <div class="mt-3" role="group" aria-label={tr($locale, "plugins.domains")}>
                  <p class="text-xs m-0 perm-prompt">
                    {entry.domains === null
                      ? tr($locale, "plugins.review_install_unscoped")
                      : tr($locale, "plugins.review_install")}
                  </p>
                  {#if entry.domains !== null}
                    <ul class="text-xs mt-1 mb-0 pl-4 perm-list">
                      {#each entry.domains as domain}<li>{domain}</li>{:else}<li>{tr($locale, "plugins.no_hosts")}</li>{/each}
                    </ul>
                  {/if}
                  <div class="flex gap-2 mt-2">
                    <Button variant="solid" size="sm" loading={busyId === entry.id} onclick={() => handleInstall(entry)}>
                      {tr($locale, "plugins.confirm_install")}
                    </Button>
                    <Button variant="ghost" size="sm" onclick={() => (reviewId = null)}>
                      {tr($locale, "plugins.cancel")}
                    </Button>
                  </div>
                </div>
              {/if}
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

<style>
  .perm-warn {
    color: var(--warning-ink);
  }
  .perm-prompt {
    color: var(--text-primary);
  }
  .perm-line {
    color: var(--text-secondary);
  }
  .perm-list {
    font-family: var(--font-mono);
    color: var(--text-secondary);
  }
  .perm-mono {
    font-family: var(--font-mono);
  }
</style>
