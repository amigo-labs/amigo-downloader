<script lang="ts">
  import { onMount } from "svelte";
  import { theme, layout, accent, currentPage, downloads, stats, pendingCaptcha, type Page, type CaptchaChallenge } from "./lib/stores";
  import { addDownload, getDownloads, getStats, connectWebSocket, formatSpeed } from "./lib/api";
  import { addToast } from "./lib/toast";
  import Downloads from "./pages/Downloads.svelte";
  import Queue from "./pages/Queue.svelte";
  import Plugins from "./pages/Plugins.svelte";
  import History from "./pages/History.svelte";
  import Settings from "./pages/Settings.svelte";
  import AddDialog from "./components/AddDialog.svelte";
  import CaptchaDialog from "./components/CaptchaDialog.svelte";
  import Mascot from "./components/Mascot.svelte";
  import Sparkline from "./components/Sparkline.svelte";
  import Toasts from "./components/Toasts.svelte";
  import DropZone from "./components/DropZone.svelte";
  import FeedbackDialog from "./components/FeedbackDialog.svelte";

  let showAddDialog = $state(false);
  let showFeedback = $state(false);
  let feedbackPrefill = $state<any>(undefined);
  let sidebarOpen = $state(false);
  let speedHistory = $state<number[]>([]);
  let pageKey = $state(0);

  // Expose for DownloadCard to trigger crash report
  function openCrashReport(errorContext: any) {
    feedbackPrefill = { error_context: errorContext };
    showFeedback = true;
  }
  // Make it available globally for child components
  if (typeof window !== "undefined") {
    (window as any).__amigo_report_crash = openCrashReport;
  }

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

    // Handle PWA Share Target — receives URLs via ?share=true&url=... or ?share=true&text=...
    handleShareTarget();

    // Fetch initial data
    loadData();
    const interval = setInterval(loadData, 2000);

    // WebSocket for live updates
    connectWebSocket((msg) => {
      if (msg.type === "completed") {
        addToast("success", "Download complete", msg.data?.filename as string || msg.id);
      } else if (msg.type === "failed") {
        addToast("error", "Download failed", msg.data?.error as string || msg.id);
      } else if (msg.type === "captcha_challenge") {
        // Show captcha dialog
        pendingCaptcha.set(msg.data as unknown as CaptchaChallenge);
      } else if (msg.type === "captcha_solved" || msg.type === "captcha_timeout") {
        pendingCaptcha.set(null);
      } else if (msg.type === "plugin_notification") {
        addToast("info", msg.data?.title as string || "Plugin", msg.data?.message as string);
      }
      // Refresh data on any event
      loadData();
    });

    return () => clearInterval(interval);
  });

  function handleShareTarget() {
    const params = new URLSearchParams(window.location.search);
    if (!params.has("share")) return;

    // Get URL from share params — could be in 'url' or 'text' field
    const sharedUrl = params.get("url") || params.get("text") || "";
    const extracted = extractUrl(sharedUrl);

    if (extracted) {
      addDownload(extracted).then(() => {
        addToast("success", "Download added!", extracted);
        loadData();
      }).catch(() => {
        addToast("error", "Failed to add download", extracted);
      });
    }

    // Clean URL so it doesn't re-trigger on refresh
    history.replaceState({}, "", "/");
  }

  /** Extract first URL from a string (share text may contain extra text around the URL) */
  function extractUrl(text: string): string | null {
    const trimmed = text.trim();
    // If it's already a clean URL
    if (trimmed.startsWith("http://") || trimmed.startsWith("https://")) {
      return trimmed.split(/\s/)[0];
    }
    // Try to find a URL in the text
    const match = trimmed.match(/https?:\/\/[^\s]+/);
    return match ? match[0] : null;
  }

  async function loadData() {
    try {
      const [dl, st] = await Promise.all([getDownloads(), getStats()]);
      downloads.set(dl);
      stats.set(st);

      // Track speed history (last 30 samples)
      speedHistory = [...speedHistory.slice(-29), st.speed_bytes_per_sec];
    } catch {
      // Server offline
    }
  }

  function navigate(page: Page) {
    currentPage.set(page);
    sidebarOpen = false;
    pageKey++;
  }

  // Global keyboard shortcuts
  function handleKeydown(e: KeyboardEvent) {
    // Ctrl+N or Ctrl+L: open add dialog
    if ((e.ctrlKey || e.metaKey) && (e.key === "n" || e.key === "l")) {
      e.preventDefault();
      showAddDialog = true;
    }
    // Escape: close dialogs
    if (e.key === "Escape") {
      showAddDialog = false;
      sidebarOpen = false;
    }
    // 1-5: navigate pages
    if (!showAddDialog && !e.ctrlKey && !e.metaKey && !e.altKey) {
      const num = parseInt(e.key);
      if (num >= 1 && num <= navItems.length) {
        navigate(navItems[num - 1].id);
      }
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="flex h-screen overflow-hidden">
  <!-- Sidebar -->
  <aside
    class="fixed inset-y-0 left-0 z-50 w-64 transform transition-transform duration-200 md:relative md:translate-x-0"
    class:translate-x-0={sidebarOpen}
    class:-translate-x-full={!sidebarOpen}
    style="background: var(--sidebar-bg)"
  >
    <div class="flex flex-col h-full">
      <!-- Logo + Mascot -->
      <div class="flex items-center gap-3 px-5 py-4 border-b border-white/10">
        <Mascot size={40} animate={$stats.active_downloads > 0} />
        <div>
          <h1 class="text-white font-bold text-lg leading-tight">amigo</h1>
          <span class="text-[10px] font-mono" style="color: var(--sidebar-text)">downloader v0.1</span>
        </div>
      </div>

      <!-- Add Button -->
      <div class="px-4 py-3">
        <button
          onclick={() => (showAddDialog = true)}
          class="w-full py-2.5 rounded-lg font-semibold text-white text-sm transition-all hover:brightness-110 active:scale-[0.98] interactive"
          style="background: var(--accent-color)"
        >
          + Add Download
          <span class="text-[10px] opacity-60 ml-1">Ctrl+N</span>
        </button>
      </div>

      <!-- Navigation -->
      <nav class="flex-1 px-3 py-1 space-y-0.5">
        {#each navItems as item, i}
          <button
            onclick={() => navigate(item.id)}
            class="w-full flex items-center gap-3 px-3 py-2.5 rounded-lg text-sm font-medium transition-all"
            class:text-white={$currentPage === item.id}
            style={$currentPage === item.id
              ? `background: var(--accent-color); color: white`
              : `color: var(--sidebar-text)`}
          >
            {@render Icon({ name: item.icon })}
            <span class="flex-1 text-left">{item.label}</span>
            <span class="text-[10px] opacity-40">{i + 1}</span>
          </button>
        {/each}
      </nav>

      <!-- Speed Sparkline -->
      <div class="px-5 py-3 border-t border-white/10">
        <div class="mb-2">
          <Sparkline values={speedHistory} width={200} height={28} color="var(--accent-color)" />
        </div>
        <div class="text-xs space-y-1" style="color: var(--sidebar-text)">
          <div class="flex justify-between">
            <span>Speed</span>
            <span class="font-mono text-white">{formatSpeed($stats.speed_bytes_per_sec)}</span>
          </div>
          <div class="flex justify-between">
            <span>Active</span>
            <span class="font-mono text-white">
              {$stats.active_downloads}
              {#if $stats.active_downloads > 0}
                <span class="inline-block w-1.5 h-1.5 rounded-full ml-1 status-pulse" style="background: var(--accent-color)"></span>
              {/if}
            </span>
          </div>
          <div class="flex justify-between">
            <span>Queued</span>
            <span class="font-mono text-white">{$stats.queued}</span>
          </div>
        </div>
        <button
          onclick={() => { feedbackPrefill = undefined; showFeedback = true; }}
          class="mt-2 text-[10px] opacity-50 hover:opacity-100 transition-opacity"
          style="color: var(--sidebar-text)"
        >
          Feedback &middot; Report Issue
        </button>
      </div>
    </div>
  </aside>

  <!-- Mobile overlay -->
  {#if sidebarOpen}
    <div class="fixed inset-0 bg-black/50 backdrop-blur-sm z-40 md:hidden" onclick={() => (sidebarOpen = false)}></div>
  {/if}

  <!-- Main content -->
  <main class="flex-1 flex flex-col min-w-0 overflow-hidden" style="background: var(--surface-color)">
    <!-- Header -->
    <header class="flex items-center justify-between px-6 py-4 border-b" style="border-color: var(--border-color)">
      <div class="flex items-center gap-3">
        <button class="md:hidden p-1" onclick={() => (sidebarOpen = !sidebarOpen)}>
          {@render Icon({ name: "menu" })}
        </button>
        <h2 class="text-xl font-bold capitalize">{$currentPage}</h2>
        {#if $stats.active_downloads > 0 && $currentPage === "downloads"}
          <span class="px-2 py-0.5 rounded-full text-xs font-mono" style="background: color-mix(in srgb, var(--accent-color) 15%, transparent); color: var(--accent-color)">
            {formatSpeed($stats.speed_bytes_per_sec)}
          </span>
        {/if}
      </div>
      <div class="flex items-center gap-2">
        <button
          onclick={() => theme.toggle()}
          class="p-2 rounded-lg transition-all hover:scale-110"
          style="color: var(--text-secondary-color)"
          title={$theme === "dark" ? "Light mode" : "Dark mode"}
        >
          {@render Icon({ name: $theme === "dark" ? "sun" : "moon" })}
        </button>
      </div>
    </header>

    <!-- Page content with transition -->
    <div class="flex-1 overflow-y-auto p-6">
      {#key pageKey}
        <div class="page-enter">
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
      {/key}
    </div>
  </main>
</div>

<!-- Add Download Dialog -->
{#if showAddDialog}
  <AddDialog onclose={() => (showAddDialog = false)} />
{/if}

<!-- Feedback Dialog -->
{#if showFeedback}
  <FeedbackDialog onclose={() => (showFeedback = false)} prefill={feedbackPrefill} />
{/if}

<!-- Captcha Dialog -->
{#if $pendingCaptcha}
  <CaptchaDialog captcha={$pendingCaptcha} onclose={() => pendingCaptcha.set(null)} />
{/if}

<!-- Toast Notifications -->
<Toasts />

<!-- Global Drag & Drop -->
<DropZone />

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
    {/if}
  </svg>
{/snippet}
