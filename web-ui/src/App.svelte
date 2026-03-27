<script lang="ts">
  import { onMount } from "svelte";
  import { theme, layout, accent, currentPage, downloads, stats, type Page } from "./lib/stores";
  import { getDownloads, getStats, connectWebSocket, formatSpeed } from "./lib/api";
  import Downloads from "./pages/Downloads.svelte";
  import Queue from "./pages/Queue.svelte";
  import Plugins from "./pages/Plugins.svelte";
  import History from "./pages/History.svelte";
  import Settings from "./pages/Settings.svelte";
  import AddDialog from "./components/AddDialog.svelte";

  let showAddDialog = $state(false);
  let sidebarOpen = $state(false);

  const navItems: { id: Page; label: string; icon: string }[] = [
    { id: "downloads", label: "Downloads", icon: "arrow-down" },
    { id: "queue", label: "Queue", icon: "list" },
    { id: "plugins", label: "Plugins", icon: "puzzle" },
    { id: "history", label: "History", icon: "clock" },
    { id: "settings", label: "Settings", icon: "gear" },
  ];

  onMount(() => {
    // Initialize theme
    document.documentElement.classList.toggle("dark", $theme === "dark");
    document.documentElement.classList.add(`accent-${$accent}`);

    // Fetch initial data
    loadData();
    const interval = setInterval(loadData, 2000);

    // WebSocket for live updates
    connectWebSocket((msg) => {
      if (msg.type === "progress" || msg.type === "status" || msg.type === "completed") {
        loadData();
      }
    });

    return () => clearInterval(interval);
  });

  async function loadData() {
    try {
      const [dl, st] = await Promise.all([getDownloads(), getStats()]);
      downloads.set(dl);
      stats.set(st);
    } catch {
      // Server offline
    }
  }

  function navigate(page: Page) {
    currentPage.set(page);
    sidebarOpen = false;
  }
</script>

<div class="flex h-screen overflow-hidden">
  <!-- Sidebar -->
  <aside
    class="fixed inset-y-0 left-0 z-50 w-64 transform transition-transform duration-200 md:relative md:translate-x-0"
    class:translate-x-0={sidebarOpen}
    class:-translate-x-full={!sidebarOpen}
    style="background: var(--sidebar-bg)"
  >
    <div class="flex flex-col h-full">
      <!-- Logo -->
      <div class="flex items-center gap-3 px-5 py-5 border-b border-white/10">
        <div class="pixel-logo text-2xl" style="font-family: 'Press Start 2P'; color: var(--accent-color)">a</div>
        <div>
          <h1 class="text-white font-bold text-lg leading-tight">amigo</h1>
          <span class="text-xs" style="color: var(--sidebar-text)">downloader</span>
        </div>
      </div>

      <!-- Add Button -->
      <div class="px-4 py-3">
        <button
          onclick={() => (showAddDialog = true)}
          class="w-full py-2.5 rounded-lg font-semibold text-white text-sm transition-all hover:brightness-110 active:scale-[0.98]"
          style="background: var(--accent-color)"
        >
          + Add Download
        </button>
      </div>

      <!-- Navigation -->
      <nav class="flex-1 px-3 py-1 space-y-0.5">
        {#each navItems as item}
          <button
            onclick={() => navigate(item.id)}
            class="w-full flex items-center gap-3 px-3 py-2.5 rounded-lg text-sm font-medium transition-all"
            class:text-white={$currentPage === item.id}
            style={$currentPage === item.id
              ? `background: var(--accent-color); color: white`
              : `color: var(--sidebar-text)`}
          >
            <Icon name={item.icon} />
            {item.label}
          </button>
        {/each}
      </nav>

      <!-- Stats footer -->
      <div class="px-5 py-4 border-t border-white/10">
        <div class="text-xs space-y-1" style="color: var(--sidebar-text)">
          <div class="flex justify-between">
            <span>Speed</span>
            <span class="font-mono text-white">{formatSpeed($stats.speed_bytes_per_sec)}</span>
          </div>
          <div class="flex justify-between">
            <span>Active</span>
            <span class="font-mono text-white">{$stats.active_downloads}</span>
          </div>
          <div class="flex justify-between">
            <span>Queued</span>
            <span class="font-mono text-white">{$stats.queued}</span>
          </div>
        </div>
      </div>
    </div>
  </aside>

  <!-- Mobile overlay -->
  {#if sidebarOpen}
    <div class="fixed inset-0 bg-black/50 z-40 md:hidden" onclick={() => (sidebarOpen = false)}></div>
  {/if}

  <!-- Main content -->
  <main class="flex-1 flex flex-col min-w-0 overflow-hidden" style="background: var(--surface-color)">
    <!-- Header -->
    <header class="flex items-center justify-between px-6 py-4 border-b" style="border-color: var(--border-color)">
      <div class="flex items-center gap-3">
        <button class="md:hidden p-1" onclick={() => (sidebarOpen = !sidebarOpen)}>
          <Icon name="menu" />
        </button>
        <h2 class="text-xl font-bold capitalize">{$currentPage}</h2>
      </div>
      <div class="flex items-center gap-3">
        <button
          onclick={() => theme.toggle()}
          class="p-2 rounded-lg transition-colors"
          style="color: var(--text-secondary-color)"
          title={$theme === "dark" ? "Light mode" : "Dark mode"}
        >
          <Icon name={$theme === "dark" ? "sun" : "moon"} />
        </button>
      </div>
    </header>

    <!-- Page content -->
    <div class="flex-1 overflow-y-auto p-6">
      {#if $currentPage === "downloads"}
        <Downloads />
      {:else if $currentPage === "queue"}
        <Queue />
      {:else if $currentPage === "plugins"}
        <Plugins />
      {:else if $currentPage === "history"}
        <History />
      {:else if $currentPage === "settings"}
        <Settings />
      {/if}
    </div>
  </main>
</div>

<!-- Add Download Dialog -->
{#if showAddDialog}
  <AddDialog onclose={() => (showAddDialog = false)} />
{/if}

<!-- Simple Icon component (inline SVG) -->
{#snippet Icon({ name }: { name: string })}
  <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="2">
    {#if name === "arrow-down"}
      <path d="M12 4v16m0 0l-6-6m6 6l6-6" />
    {:else if name === "list"}
      <path d="M4 6h16M4 12h16M4 18h16" />
    {:else if name === "puzzle"}
      <path d="M11 4a2 2 0 114 0v1a1 1 0 001 1h3a2 2 0 012 2v3a1 1 0 01-1 1 2 2 0 100 4 1 1 0 011 1v3a2 2 0 01-2 2h-3a1 1 0 01-1-1 2 2 0 10-4 0 1 1 0 01-1 1H7a2 2 0 01-2-2v-3a1 1 0 011-1 2 2 0 100-4 1 1 0 01-1-1V7a2 2 0 012-2h3a1 1 0 001-1V4z" />
    {:else if name === "clock"}
      <path d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z" />
    {:else if name === "gear"}
      <path d="M10.3 4.3a1 1 0 011.4 0l.3.3a1 1 0 001.1.2 8 8 0 011.5.6 1 1 0 00.9-.3l.3-.3a1 1 0 011.4 0l1 1a1 1 0 010 1.4l-.3.3a1 1 0 00-.2 1.1c.3.5.5 1 .6 1.5a1 1 0 00.8.7h.4a1 1 0 011 1v1.4a1 1 0 01-1 1h-.4a1 1 0 00-.8.7 8 8 0 01-.6 1.5 1 1 0 00.2 1.1l.3.3a1 1 0 010 1.4l-1 1a1 1 0 01-1.4 0l-.3-.3a1 1 0 00-1.1-.2 8 8 0 01-1.5.6 1 1 0 00-.7.8v.4a1 1 0 01-1 1h-1.4a1 1 0 01-1-1v-.4a1 1 0 00-.7-.8 8 8 0 01-1.5-.6 1 1 0 00-1.1.2l-.3.3a1 1 0 01-1.4 0l-1-1a1 1 0 010-1.4l.3-.3a1 1 0 00.2-1.1 8 8 0 01-.6-1.5 1 1 0 00-.8-.7H4a1 1 0 01-1-1v-1.4a1 1 0 011-1h.4a1 1 0 00.8-.7c.1-.5.3-1 .6-1.5a1 1 0 00-.2-1.1l-.3-.3a1 1 0 010-1.4l1-1a1 1 0 011.4 0l.3.3a1 1 0 001.1.2 8 8 0 011.5-.6 1 1 0 00.7-.8V4a1 1 0 011-1h1.4" />
      <circle cx="12" cy="12" r="3" />
    {:else if name === "menu"}
      <path d="M4 6h16M4 12h16M4 18h16" />
    {:else if name === "sun"}
      <path d="M12 3v1m0 16v1m-8-9H3m18 0h-1m-2.6-6.4l-.7.7m-9.4 9.4l-.7.7m12.8 0l-.7-.7M6 6l-.7-.7M16 12a4 4 0 11-8 0 4 4 0 018 0z" />
    {:else if name === "moon"}
      <path d="M20.4 12.1a8 8 0 01-11.5-7.3 8 8 0 1011.5 7.3z" />
    {:else if name === "x"}
      <path d="M6 18L18 6M6 6l12 12" />
    {/if}
  </svg>
{/snippet}
