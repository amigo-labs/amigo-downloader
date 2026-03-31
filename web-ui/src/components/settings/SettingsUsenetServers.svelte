<script lang="ts">
  import { onMount } from "svelte";
  import { usenetServers, features, type UsenetServer } from "../../lib/stores";
  import { getUsenetServers, addUsenetServer, deleteUsenetServer } from "../../lib/api";
  import { addToast } from "../../lib/toast";

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
      addToast("success", "Server added");
      resetForm();
      showAddForm = false;
    } catch (e: any) {
      addToast("error", e.message || "Failed to add server");
    } finally {
      saving = false;
    }
  }

  async function handleDelete(id: string) {
    try {
      await deleteUsenetServer(id);
      usenetServers.update((s) => s.filter((srv) => srv.id !== id));
      addToast("success", "Server removed");
    } catch {
      addToast("error", "Failed to remove server");
    }
  }
</script>

<section>
  <div class="flex items-center justify-between mb-4">
    <h3 class="text-lg font-bold" style="color: var(--text-primary)">Usenet Servers</h3>
    <button
      onclick={() => (showAddForm = !showAddForm)}
      class="px-3 py-1.5 rounded-lg text-xs font-semibold"
      style="background: var(--neon-primary); color: var(--bg-deep)"
    >
      {showAddForm ? "Cancel" : "+ Add Server"}
    </button>
  </div>

  {#if showAddForm}
    <div class="rounded-xl p-5 space-y-3 mb-4" style="background: var(--bg-surface); border: 1px solid var(--border-color)">
      <div class="grid grid-cols-2 gap-3">
        <div>
          <label class="block text-xs font-medium mb-1" style="color: var(--text-secondary)">Name</label>
          <input type="text" bind:value={name} placeholder="My Usenet Server"
            class="w-full rounded-lg px-3 py-2 text-sm"
            style="background: var(--bg-surface-2); border: 1px solid var(--border-color); color: var(--text-primary)" />
        </div>
        <div>
          <label class="block text-xs font-medium mb-1" style="color: var(--text-secondary)">Host</label>
          <input type="text" bind:value={host} placeholder="news.example.com"
            class="w-full rounded-lg px-3 py-2 text-sm"
            style="font-family: var(--font-mono);background: var(--bg-surface-2); border: 1px solid var(--border-color); color: var(--text-primary)" />
        </div>
      </div>
      <div class="grid grid-cols-3 gap-3">
        <div>
          <label class="block text-xs font-medium mb-1" style="color: var(--text-secondary)">Port</label>
          <input type="number" bind:value={port}
            class="w-full rounded-lg px-3 py-2 text-sm"
            style="font-family: var(--font-mono);background: var(--bg-surface-2); border: 1px solid var(--border-color); color: var(--text-primary)" />
        </div>
        <div>
          <label class="block text-xs font-medium mb-1" style="color: var(--text-secondary)">Connections</label>
          <input type="number" bind:value={connections} min="1" max="50"
            class="w-full rounded-lg px-3 py-2 text-sm"
            style="font-family: var(--font-mono);background: var(--bg-surface-2); border: 1px solid var(--border-color); color: var(--text-primary)" />
        </div>
        <div>
          <label class="block text-xs font-medium mb-1" style="color: var(--text-secondary)">Priority</label>
          <input type="number" bind:value={priority} min="0"
            class="w-full rounded-lg px-3 py-2 text-sm"
            style="font-family: var(--font-mono);background: var(--bg-surface-2); border: 1px solid var(--border-color); color: var(--text-primary)" />
        </div>
      </div>
      <div class="grid grid-cols-2 gap-3">
        <div>
          <label class="block text-xs font-medium mb-1" style="color: var(--text-secondary)">Username</label>
          <input type="text" bind:value={username}
            class="w-full rounded-lg px-3 py-2 text-sm"
            style="background: var(--bg-surface-2); border: 1px solid var(--border-color); color: var(--text-primary)" />
        </div>
        <div>
          <label class="block text-xs font-medium mb-1" style="color: var(--text-secondary)">Password</label>
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
          style="background: var(--neon-primary); color: var(--bg-deep)">{saving ? "Saving..." : "Save Server"}</button>
        <button onclick={() => { resetForm(); showAddForm = false; }}
          class="px-4 py-2 rounded-lg text-sm"
          style="color: var(--text-secondary)">Cancel</button>
      </div>
    </div>
  {/if}

  {#if $usenetServers.length === 0 && !showAddForm}
    <div class="rounded-xl p-8 text-center" style="background: var(--bg-surface); border: 1px solid var(--border-color)">
      <p class="text-sm" style="color: var(--text-secondary)">
        No Usenet servers configured. Add a server to start downloading from Usenet.
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
              {server.connections} connections &middot; Priority {server.priority}
            </p>
          </div>
          <button
            onclick={() => handleDelete(server.id)}
            class="px-2.5 py-1.5 rounded-lg text-xs shrink-0"
            style="color: var(--neon-accent); border: 1px solid color-mix(in srgb, var(--neon-accent) 20%, transparent)"
          >Delete</button>
        </div>
        {#if $features.server_stats}
          <div class="mt-3 pt-3 grid grid-cols-4 gap-2 text-center" style="border-top: 1px solid var(--border-color)">
            <div>
              <p class="text-[10px] uppercase tracking-wide" style="color: var(--text-secondary)">Status</p>
              <p class="text-xs font-semibold" style="color: var(--neon-success)">Idle</p>
            </div>
            <div>
              <p class="text-[10px] uppercase tracking-wide" style="color: var(--text-secondary)">Active</p>
              <p class="text-xs" style="font-family: var(--font-mono)">0/{server.connections}</p>
            </div>
            <div>
              <p class="text-[10px] uppercase tracking-wide" style="color: var(--text-secondary)">Articles</p>
              <p class="text-xs" style="font-family: var(--font-mono)">&mdash;</p>
            </div>
            <div>
              <p class="text-[10px] uppercase tracking-wide" style="color: var(--text-secondary)">Speed</p>
              <p class="text-xs" style="font-family: var(--font-mono)">&mdash;</p>
            </div>
          </div>
        {/if}
      </div>
    {/each}
  </div>
</section>
