<script lang="ts">
  import { onMount } from "svelte";
  import { usenetServers, features } from "../lib/stores";
  import { getUsenetServers, addUsenetServer, deleteUsenetServer } from "../lib/api";
  import { addToast } from "../lib/toast";

  let showAddForm = $state(false);
  let saving = $state(false);

  // Form fields
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
    } catch {
      // Server offline
    }
  });

  function resetForm() {
    name = "";
    host = "";
    port = 563;
    ssl = true;
    username = "";
    password = "";
    connections = 10;
    priority = 0;
  }

  async function handleAdd() {
    if (!name.trim() || !host.trim()) return;
    saving = true;
    try {
      const server = await addUsenetServer({
        name: name.trim(),
        host: host.trim(),
        port,
        ssl,
        username: username.trim(),
        password,
        connections,
        priority,
      });
      usenetServers.update((s) => [...s, server]);
      addToast("Server added", "success");
      resetForm();
      showAddForm = false;
    } catch (e: any) {
      addToast(e.message || "Failed to add server", "error");
    } finally {
      saving = false;
    }
  }

  async function handleDelete(id: string) {
    try {
      await deleteUsenetServer(id);
      usenetServers.update((s) => s.filter((srv) => srv.id !== id));
      addToast("Server removed", "success");
    } catch {
      addToast("Failed to remove server", "error");
    }
  }
</script>

<div class="max-w-2xl space-y-6">
  <!-- Header -->
  <div class="flex items-center justify-between">
    <div>
      <h3 class="text-lg font-bold">Usenet Servers</h3>
      <p class="text-xs mt-0.5" style="color: var(--text-secondary-color)">
        Manage NNTP server connections for Usenet downloads
      </p>
    </div>
    <button
      onclick={() => (showAddForm = !showAddForm)}
      class="px-3 py-1.5 rounded-lg text-xs font-semibold text-white"
      style="background: var(--accent-color)"
    >
      {showAddForm ? "Cancel" : "+ Add Server"}
    </button>
  </div>

  <!-- Add Form -->
  {#if showAddForm}
    <div
      class="rounded-xl p-5 space-y-3"
      style="background: var(--surface-2-color); border: 1px solid var(--border-color)"
    >
      <div class="grid grid-cols-2 gap-3">
        <div>
          <label class="block text-xs font-medium mb-1" style="color: var(--text-secondary-color)">Name</label>
          <input
            type="text"
            bind:value={name}
            placeholder="My Usenet Server"
            class="w-full rounded-lg px-3 py-2 text-sm outline-none"
            style="background: var(--surface-3-color); border: 1px solid var(--border-color); color: var(--text-color)"
          />
        </div>
        <div>
          <label class="block text-xs font-medium mb-1" style="color: var(--text-secondary-color)">Host</label>
          <input
            type="text"
            bind:value={host}
            placeholder="news.example.com"
            class="w-full rounded-lg px-3 py-2 text-sm font-mono outline-none"
            style="background: var(--surface-3-color); border: 1px solid var(--border-color); color: var(--text-color)"
          />
        </div>
      </div>

      <div class="grid grid-cols-3 gap-3">
        <div>
          <label class="block text-xs font-medium mb-1" style="color: var(--text-secondary-color)">Port</label>
          <input
            type="number"
            bind:value={port}
            class="w-full rounded-lg px-3 py-2 text-sm font-mono outline-none"
            style="background: var(--surface-3-color); border: 1px solid var(--border-color); color: var(--text-color)"
          />
        </div>
        <div>
          <label class="block text-xs font-medium mb-1" style="color: var(--text-secondary-color)">Connections</label>
          <input
            type="number"
            bind:value={connections}
            min="1"
            max="50"
            class="w-full rounded-lg px-3 py-2 text-sm font-mono outline-none"
            style="background: var(--surface-3-color); border: 1px solid var(--border-color); color: var(--text-color)"
          />
        </div>
        <div>
          <label class="block text-xs font-medium mb-1" style="color: var(--text-secondary-color)">Priority</label>
          <input
            type="number"
            bind:value={priority}
            min="0"
            class="w-full rounded-lg px-3 py-2 text-sm font-mono outline-none"
            style="background: var(--surface-3-color); border: 1px solid var(--border-color); color: var(--text-color)"
          />
        </div>
      </div>

      <div class="grid grid-cols-2 gap-3">
        <div>
          <label class="block text-xs font-medium mb-1" style="color: var(--text-secondary-color)">Username</label>
          <input
            type="text"
            bind:value={username}
            class="w-full rounded-lg px-3 py-2 text-sm outline-none"
            style="background: var(--surface-3-color); border: 1px solid var(--border-color); color: var(--text-color)"
          />
        </div>
        <div>
          <label class="block text-xs font-medium mb-1" style="color: var(--text-secondary-color)">Password</label>
          <input
            type="password"
            bind:value={password}
            class="w-full rounded-lg px-3 py-2 text-sm outline-none"
            style="background: var(--surface-3-color); border: 1px solid var(--border-color); color: var(--text-color)"
          />
        </div>
      </div>

      <div class="flex items-center gap-3">
        <label class="flex items-center gap-2 text-sm cursor-pointer">
          <input type="checkbox" bind:checked={ssl} class="rounded" />
          <span>SSL/TLS</span>
        </label>
      </div>

      <div class="flex gap-2 pt-2">
        <button
          onclick={handleAdd}
          disabled={saving || !name.trim() || !host.trim()}
          class="px-4 py-2 rounded-lg text-sm font-semibold text-white disabled:opacity-50"
          style="background: var(--accent-color)"
        >
          {saving ? "Saving..." : "Save Server"}
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

  <!-- Empty state -->
  {#if $usenetServers.length === 0 && !showAddForm}
    <div
      class="rounded-xl p-8 text-center"
      style="background: var(--surface-2-color); border: 1px solid var(--border-color)"
    >
      <p class="text-sm" style="color: var(--text-secondary-color)">
        No Usenet servers configured. Add a server to start downloading from Usenet.
      </p>
    </div>
  {/if}

  <!-- Server list -->
  {#each $usenetServers as server (server.id)}
    <div
      class="rounded-xl p-4"
      style="background: var(--surface-2-color); border: 1px solid var(--border-color)"
    >
      <div class="flex items-center justify-between gap-3">
        <div class="min-w-0 flex-1">
          <p class="font-semibold text-sm truncate">{server.name}</p>
          <p class="text-xs font-mono truncate" style="color: var(--text-secondary-color)">
            {server.host}:{server.port} {server.ssl ? "(SSL)" : ""}
          </p>
          <p class="text-[10px] mt-0.5" style="color: var(--text-secondary-color)">
            {server.connections} connections &middot; Priority {server.priority}
          </p>
        </div>
        <div class="flex gap-1.5 shrink-0">
          <button
            onclick={() => handleDelete(server.id)}
            class="px-2.5 py-1.5 rounded-lg text-xs text-red-400 hover:bg-red-400/10"
          >
            Delete
          </button>
        </div>
      </div>

      <!-- Server stats (only shown when feature enabled) -->
      {#if $features.server_stats}
        <div
          class="mt-3 pt-3 grid grid-cols-4 gap-2 text-center"
          style="border-top: 1px solid var(--border-color)"
        >
          <div>
            <p class="text-[10px] uppercase tracking-wide" style="color: var(--text-secondary-color)">Status</p>
            <p class="text-xs font-semibold" style="color: #10b981">Idle</p>
          </div>
          <div>
            <p class="text-[10px] uppercase tracking-wide" style="color: var(--text-secondary-color)">Active</p>
            <p class="text-xs font-mono">0/{server.connections}</p>
          </div>
          <div>
            <p class="text-[10px] uppercase tracking-wide" style="color: var(--text-secondary-color)">Articles</p>
            <p class="text-xs font-mono">—</p>
          </div>
          <div>
            <p class="text-[10px] uppercase tracking-wide" style="color: var(--text-secondary-color)">Speed</p>
            <p class="text-xs font-mono">—</p>
          </div>
        </div>
      {/if}
    </div>
  {/each}
</div>
