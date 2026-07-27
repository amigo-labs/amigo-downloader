<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { usenetServers, features, type UsenetServer } from "../../lib/stores";
  import { getUsenetServers, addUsenetServer, deleteUsenetServer } from "../../lib/api";
  import { addToast } from "../../lib/toast";
  import { locale, tr } from "../../lib/i18n";
  import Card from "@amigo/ui/components/Card.svelte";
  import Button from "@amigo/ui/components/Button.svelte";
  import Field from "@amigo/ui/components/Field.svelte";
  import Toggle from "@amigo/ui/components/Toggle.svelte";
  import Banner from "@amigo/ui/components/Banner.svelte";
  import EmptyState from "@amigo/ui/components/EmptyState.svelte";
  import StatusDot from "@amigo/ui/components/StatusDot.svelte";

  let showAddForm = $state(false);
  let saving = $state(false);
  let loading = $state(true);
  // The load error used to be swallowed by `catch { /* offline */ }`, which
  // rendered the "no servers configured" empty state — telling the user their
  // configuration was gone when the server was merely unreachable.
  let loadError = $state(false);
  let confirmingDelete = $state<string | null>(null);
  let confirmTimer: ReturnType<typeof setTimeout> | undefined;

  let name = $state("");
  let host = $state("");
  let port = $state(563);
  let ssl = $state(true);
  let username = $state("");
  let password = $state("");
  let connections = $state(10);
  let priority = $state(0);

  onMount(async () => {
    try {
      usenetServers.set(await getUsenetServers());
    } catch {
      loadError = true;
    } finally {
      loading = false;
    }
  });

  // Not returned from onMount: an async onMount cannot supply a teardown.
  onDestroy(() => clearTimeout(confirmTimer));

  function resetForm() {
    name = ""; host = ""; port = 563; ssl = true;
    username = ""; password = ""; connections = 10; priority = 0;
  }

  async function handleAdd() {
    if (!name.trim() || !host.trim() || saving) return;
    saving = true;
    try {
      const server = await addUsenetServer({
        name: name.trim(), host: host.trim(), port, ssl,
        username: username.trim(), password, connections, priority,
      });
      usenetServers.update((s) => [...s, server as UsenetServer]);
      addToast("success", tr($locale, "usenet.added"));
      resetForm();
      showAddForm = false;
    } catch (e) {
      addToast("error", e instanceof Error ? e.message : tr($locale, "usenet.add_failed"));
    } finally {
      saving = false;
    }
  }

  // Deleting a configured server used to be a single unconfirmed click, while
  // deleting a download needed two. Same two-step confirmation everywhere now.
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
      await deleteUsenetServer(id);
      usenetServers.update((s) => s.filter((srv) => srv.id !== id));
      addToast("success", tr($locale, "usenet.removed"));
    } catch {
      addToast("error", tr($locale, "usenet.remove_failed"));
    }
  }
</script>

<section>
  <div class="flex items-center justify-between gap-3 mb-4">
    <h3 class="text-lg font-bold" style="color: var(--text-primary)">{tr($locale, "usenet.title")}</h3>
    <Button
      variant={showAddForm ? "ghost" : "solid"}
      size="sm"
      iconLeft={showAddForm ? undefined : "plus"}
      onclick={() => (showAddForm = !showAddForm)}
    >
      {showAddForm ? tr($locale, "common.cancel") : tr($locale, "usenet.add")}
    </Button>
  </div>

  {#if loadError}
    <div class="mb-4">
      <Banner tone="danger" role="alert">{tr($locale, "usenet.load_failed")}</Banner>
    </div>
  {/if}

  {#if showAddForm}
    <Card padding="lg" class="mb-4">
      <div class="grid gap-3">
        <!-- grid-cols-2/3 with no breakpoint left each number input about
             87px wide on a 360px screen. -->
        <div class="grid grid-cols-1 sm:grid-cols-2 gap-3">
          <Field label={tr($locale, "usenet.name")} bind:value={name} required placeholder="My Usenet Server" />
          <Field label={tr($locale, "usenet.host")} bind:value={host} required mono placeholder="news.example.com" />
        </div>
        <div class="grid grid-cols-2 sm:grid-cols-3 gap-3">
          <Field label={tr($locale, "usenet.port")} type="number" bind:value={port} mono />
          <Field label={tr($locale, "usenet.connections")} type="number" bind:value={connections} min={1} max={50} mono />
          <Field label={tr($locale, "usenet.priority")} type="number" bind:value={priority} min={0} mono />
        </div>
        <div class="grid grid-cols-1 sm:grid-cols-2 gap-3">
          <Field label={tr($locale, "usenet.username")} bind:value={username} autocomplete="off" />
          <Field label={tr($locale, "usenet.password")} type="password" bind:value={password} autocomplete="new-password" />
        </div>
        <Toggle bind:checked={ssl} label="SSL/TLS" description={tr($locale, "usenet.ssl_hint")} />
        <div class="flex gap-2 pt-1">
          <Button
            variant="solid"
            onclick={handleAdd}
            loading={saving}
            disabled={!name.trim() || !host.trim()}
          >
            {tr($locale, "usenet.save")}
          </Button>
          <Button variant="ghost" onclick={() => { resetForm(); showAddForm = false; }}>
            {tr($locale, "common.cancel")}
          </Button>
        </div>
      </div>
    </Card>
  {/if}

  {#if !loading && !loadError && $usenetServers.length === 0 && !showAddForm}
    <Card padding="none">
      <EmptyState size="sm" icon="globe" title={tr($locale, "usenet.empty")} />
    </Card>
  {/if}

  <div class="space-y-3">
    {#each $usenetServers as server (server.id)}
      <Card>
        <div class="flex items-center justify-between gap-3">
          <div class="min-w-0 flex-1">
            <p class="font-semibold text-sm truncate" style="color: var(--text-primary)">{server.name}</p>
            <p class="text-xs truncate" style="font-family: var(--font-mono); color: var(--text-secondary)">
              {server.host}:{server.port}{server.ssl ? " (SSL)" : ""}
            </p>
            <p class="text-xs mt-0.5" style="color: var(--text-secondary)">
              {tr($locale, "usenet.meta", { count: server.connections, priority: server.priority })}
            </p>
          </div>
          <Button
            variant="outline"
            tone="danger"
            size="sm"
            onclick={() => handleDelete(server.id)}
          >
            {confirmingDelete === server.id ? tr($locale, "action.sure") : tr($locale, "common.delete")}
          </Button>
        </div>
        {#if $features.server_stats}
          <div class="mt-3 pt-3 grid grid-cols-2 sm:grid-cols-4 gap-2" style="border-top: 1px solid var(--border-color)">
            <div class="text-center">
              <p class="stat-label">{tr($locale, "usenet.stat_status")}</p>
              <StatusDot status="idle" label={tr($locale, "usenet.idle")} showLabel />
            </div>
            <div class="text-center">
              <p class="stat-label">{tr($locale, "usenet.stat_active")}</p>
              <p class="text-xs" style="font-family: var(--font-mono); color: var(--text-primary)">0/{server.connections}</p>
            </div>
            <div class="text-center">
              <p class="stat-label">{tr($locale, "usenet.stat_articles")}</p>
              <p class="text-xs" style="font-family: var(--font-mono); color: var(--text-secondary)">&mdash;</p>
            </div>
            <div class="text-center">
              <p class="stat-label">{tr($locale, "usenet.stat_speed")}</p>
              <p class="text-xs" style="font-family: var(--font-mono); color: var(--text-secondary)">&mdash;</p>
            </div>
          </div>
        {/if}
      </Card>
    {/each}
  </div>
</section>
