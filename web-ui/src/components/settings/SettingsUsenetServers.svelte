<script lang="ts">
  import { onMount } from "svelte";
  import { usenetServers, features, type UsenetServer } from "../../lib/stores";
  import { getUsenetServers, addUsenetServer, deleteUsenetServer } from "../../lib/api";
  import { addToast } from "../../lib/toast";
  import { locale, tr } from "../../lib/i18n";

  let showAddForm = $state(false);
  let saving = $state(false);

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
      const servers = await getUsenetServers();
      usenetServers.set(servers);
    } catch { /* offline */ }
  });

  function resetForm() {
    name = ""; host = ""; port = 563; ssl = true;
    username = ""; password = ""; connections = 10; priority = 0;
  }

  async function handleAdd() {
    if (!name.trim() || !host.trim()) return;
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
    } catch (e: any) {
      addToast("error", e.message || tr($locale, "usenet.add_failed"));
    } finally {
      saving = false;
    }
  }

  async function handleDelete(id: string) {
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
  <div class="flex items-center justify-between mb-4">
    <h3 class="text-lg font-bold" style="color: var(--text-primary)">{tr($locale, "usenet.title")}</h3>
    <button
      onclick={() => (showAddForm = !showAddForm)}
      class="px-3 py-1.5 rounded-lg text-xs font-semibold"
      style="background: var(--neon-primary); color: var(--bg-deep)"
    >
      {showAddForm ? tr($locale, "common.cancel") : tr($locale, "usenet.add")}
    </button>
  </div>

  {#if showAddForm}
    <div class="rounded-xl p-5 space-y-3 mb-4" style="background: var(--bg-surface); border: 1px solid var(--border-color)">
      <div class="grid grid-cols-2 gap-3">
        <div>
          <label class="block text-xs font-medium mb-1" style="color: var(--text-secondary)">{tr($locale, "usenet.name")}</label>
          <input type="text" bind:value={name} placeholder="My Usenet Server"
            class="w-full rounded-lg px-3 py-2 text-sm"
            style="background: var(--bg-surface-2); border: 1px solid var(--border-color); color: var(--text-primary)" />
        </div>
        <div>
          <label class="block text-xs font-medium mb-1" style="color: var(--text-secondary)">{tr($locale, "usenet.host")}</label>
          <input type="text" bind:value={host} placeholder="news.example.com"
            class="w-full rounded-lg px-3 py-2 text-sm"
            style="font-family: var(--font-mono);background: var(--bg-surface-2); border: 1px solid var(--border-color); color: var(--text-primary)" />
        </div>
      </div>
      <div class="grid grid-cols-3 gap-3">
        <div>
          <label class="block text-xs font-medium mb-1" style="color: var(--text-secondary)">{tr($locale, "usenet.port")}</label>
          <input type="number" bind:value={port}
            class="w-full rounded-lg px-3 py-2 text-sm"
            style="font-family: var(--font-mono);background: var(--bg-surface-2); border: 1px solid var(--border-color); color: var(--text-primary)" />
        </div>
        <div>
          <label class="block text-xs font-medium mb-1" style="color: var(--text-secondary)">{tr($locale, "usenet.connections")}</label>
          <input type="number" bind:value={connections} min="1" max="50"
            class="w-full rounded-lg px-3 py-2 text-sm"
            style="font-family: var(--font-mono);background: var(--bg-surface-2); border: 1px solid var(--border-color); color: var(--text-primary)" />
        </div>
        <div>
          <label class="block text-xs font-medium mb-1" style="color: var(--text-secondary)">{tr($locale, "usenet.priority")}</label>
          <input type="number" bind:value={priority} min="0"
            class="w-full rounded-lg px-3 py-2 text-sm"
            style="font-family: var(--font-mono);background: var(--bg-surface-2); border: 1px solid var(--border-color); color: var(--text-primary)" />
        </div>
      </div>
      <div class="grid grid-cols-2 gap-3">
        <div>
          <label class="block text-xs font-medium mb-1" style="color: var(--text-secondary)">{tr($locale, "usenet.username")}</label>
          <input type="text" bind:value={username}
            class="w-full rounded-lg px-3 py-2 text-sm"
            style="background: var(--bg-surface-2); border: 1px solid var(--border-color); color: var(--text-primary)" />
        </div>
        <div>
          <label class="block text-xs font-medium mb-1" style="color: var(--text-secondary)">{tr($locale, "usenet.password")}</label>
          <input type="password" bind:value={password}
            class="w-full rounded-lg px-3 py-2 text-sm"
            style="background: var(--bg-surface-2); border: 1px solid var(--border-color); color: var(--text-primary)" />
        </div>
      </div>
      <div class="flex items-center gap-3">
        <label class="flex items-center gap-2 text-sm cursor-pointer" style="color: var(--text-primary)">
          <input type="checkbox" bind:checked={ssl} class="rounded" />
          <span>SSL/TLS</span>
        </label>
      </div>
      <div class="flex gap-2 pt-2">
        <button onclick={handleAdd} disabled={saving || !name.trim() || !host.trim()}
          class="px-4 py-2 rounded-lg text-sm font-semibold disabled:opacity-50"
          style="background: var(--neon-primary); color: var(--bg-deep)">{saving ? tr($locale, "common.saving") : tr($locale, "usenet.save")}</button>
        <button onclick={() => { resetForm(); showAddForm = false; }}
          class="px-4 py-2 rounded-lg text-sm"
          style="color: var(--text-secondary)">{tr($locale, "common.cancel")}</button>
      </div>
    </div>
  {/if}

  {#if $usenetServers.length === 0 && !showAddForm}
    <div class="rounded-xl p-8 text-center" style="background: var(--bg-surface); border: 1px solid var(--border-color)">
      <p class="text-sm" style="color: var(--text-secondary)">
        {tr($locale, "usenet.empty")}
      </p>
    </div>
  {/if}

  <div class="space-y-3">
    {#each $usenetServers as server (server.id)}
      <div class="rounded-xl p-4" style="background: var(--bg-surface); border: 1px solid var(--border-color)">
        <div class="flex items-center justify-between gap-3">
          <div class="min-w-0 flex-1">
            <p class="font-semibold text-sm truncate" style="color: var(--text-primary)">{server.name}</p>
            <p class="text-xs truncate" style="font-family: var(--font-mono);color: var(--text-secondary)">
              {server.host}:{server.port} {server.ssl ? "(SSL)" : ""}
            </p>
            <p class="text-[10px] mt-0.5" style="color: var(--text-secondary)">
              {tr($locale, "usenet.meta", { count: server.connections, priority: server.priority })}
            </p>
          </div>
          <button
            onclick={() => handleDelete(server.id)}
            class="px-2.5 py-1.5 rounded-lg text-xs shrink-0"
            style="color: var(--neon-accent); border: 1px solid color-mix(in srgb, var(--neon-accent) 20%, transparent)"
          >{tr($locale, "common.delete")}</button>
        </div>
        {#if $features.server_stats}
          <div class="mt-3 pt-3 grid grid-cols-4 gap-2 text-center" style="border-top: 1px solid var(--border-color)">
            <div>
              <p class="text-[10px] uppercase tracking-wide" style="color: var(--text-secondary)">{tr($locale, "usenet.stat_status")}</p>
              <p class="text-xs font-semibold" style="color: var(--neon-success)">{tr($locale, "usenet.idle")}</p>
            </div>
            <div>
              <p class="text-[10px] uppercase tracking-wide" style="color: var(--text-secondary)">{tr($locale, "usenet.stat_active")}</p>
              <p class="text-xs" style="font-family: var(--font-mono)">0/{server.connections}</p>
            </div>
            <div>
              <p class="text-[10px] uppercase tracking-wide" style="color: var(--text-secondary)">{tr($locale, "usenet.stat_articles")}</p>
              <p class="text-xs" style="font-family: var(--font-mono)">&mdash;</p>
            </div>
            <div>
              <p class="text-[10px] uppercase tracking-wide" style="color: var(--text-secondary)">{tr($locale, "usenet.stat_speed")}</p>
              <p class="text-xs" style="font-family: var(--font-mono)">&mdash;</p>
            </div>
          </div>
        {/if}
      </div>
    {/each}
  </div>
</section>
