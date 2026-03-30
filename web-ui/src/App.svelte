<script lang="ts">
  import { onMount } from "svelte";
  import { theme, accent, currentPage, sidebarCollapsed, selectedDownloadId, downloads, stats, pendingCaptcha, features, crashReport, protocolFilter, updateDownloadProgress, updateDownloadStatus, type Page, type CaptchaChallenge, type ProtocolFilter } from "./lib/stores";
  import { addDownload, getDownloads, getStats, getConfig, connectWebSocket, formatSpeed } from "./lib/api";
  import { addToast } from "./lib/toast";
  import Downloads from "./pages/Downloads.svelte";
  import Plugins from "./pages/Plugins.svelte";
  import History from "./pages/History.svelte";
  import Settings from "./pages/Settings.svelte";
  import AddDialog from "./components/AddDialog.svelte";
  import CaptchaDialog from "./components/CaptchaDialog.svelte";
  import Toasts from "./components/Toasts.svelte";
  import DropZone from "./components/DropZone.svelte";
  import FeedbackDialog from "./components/FeedbackDialog.svelte";
  import Footer from "./components/Footer.svelte";
  import DetailPanel from "./components/DetailPanel.svelte";
  import Icon from "./components/Icon.svelte";

  let showAddDialog = $state(false);
  let showFeedback = $state(false);
  let mobileMenuOpen = $state(false);
  let speedHistory = $state<number[]>([]);
  let pageKey = $state(0);

  const navItems: { id: Page; label: string; icon: string }[] = [
    { id: "downloads", label: "Downloads", icon: "arrow-down" },
    { id: "plugins", label: "Plugins", icon: "puzzle" },
    { id: "history", label: "History", icon: "clock" },
    { id: "settings", label: "Settings", icon: "gear" },
  ];

  const protocolOptions: { value: ProtocolFilter; label: string }[] = [
    { value: "all", label: "ALL" },
    { value: "http", label: "HTTP" },
    { value: "usenet", label: "USENET" },
  ];

  onMount(() => {
    // Initialize theme
    document.documentElement.classList.toggle("lights-on", $theme === "lights-on");
    if ($accent !== "electric") {
      document.documentElement.classList.add(`accent-${$accent}`);
    }

    handleShareTarget();
    loadData();
    const interval = setInterval(loadData, 10000);

    connectWebSocket((msg) => {
      if (msg.type === "progress") {
        const p = msg.data?.progress as { bytes_downloaded?: number; total_bytes?: number; speed_bytes_per_sec?: number } | undefined;
        if (p) {
          updateDownloadProgress(msg.id, p.bytes_downloaded ?? 0, p.speed_bytes_per_sec ?? 0, p.total_bytes ?? undefined);
        }
        return;
      }

      if (msg.type === "completed") {
        updateDownloadStatus(msg.id, "completed");
        addToast("success", "Download complete", msg.data?.filename as string || msg.id);
      } else if (msg.type === "failed") {
        updateDownloadStatus(msg.id, "failed");
        addToast("error", "Download failed", msg.data?.error as string || msg.id);
      } else if (msg.type === "captcha_challenge") {
        pendingCaptcha.set(msg.data as unknown as CaptchaChallenge);
      } else if (msg.type === "captcha_solved" || msg.type === "captcha_timeout") {
        pendingCaptcha.set(null);
      } else if (msg.type === "plugin_notification") {
        addToast("info", msg.data?.title as string || "Plugin", msg.data?.message as string);
      }
      loadData();
    });

    return () => clearInterval(interval);
  });

  function handleShareTarget() {
    const params = new URLSearchParams(window.location.search);
    if (!params.has("share")) return;
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
    history.replaceState({}, "", "/");
  }

  function extractUrl(text: string): string | null {
    const trimmed = text.trim();
    if (trimmed.startsWith("http://") || trimmed.startsWith("https://")) {
      return trimmed.split(/\s/)[0];
    }
    const match = trimmed.match(/https?:\/\/[^\s]+/);
    return match ? match[0] : null;
  }

  async function loadData() {
    try {
      const [dl, st, cfg] = await Promise.all([getDownloads(), getStats(), getConfig()]);
      downloads.set(dl);
      stats.set(st);
      if (cfg?.features) features.set(cfg.features);
      speedHistory = [...speedHistory.slice(-29), st.speed_bytes_per_sec];
    } catch {
      // Server offline
    }
  }

  function navigate(page: Page) {
    currentPage.set(page);
    mobileMenuOpen = false;
    pageKey++;
  }

  // Single global keyboard handler (audit M11)
  function handleKeydown(e: KeyboardEvent) {
    if ((e.ctrlKey || e.metaKey) && (e.key === "n" || e.key === "l")) {
      e.preventDefault();
      showAddDialog = true;
    }
    if (e.key === "Escape") {
      if (showAddDialog) { showAddDialog = false; return; }
      if (showFeedback) { showFeedback = false; return; }
      if ($selectedDownloadId) { selectedDownloadId.set(null); return; }
      mobileMenuOpen = false;
    }
    if (!showAddDialog && !showFeedback && !e.ctrlKey && !e.metaKey && !e.altKey) {
      const num = parseInt(e.key);
      if (num >= 1 && num <= navItems.length) {
        navigate(navItems[num - 1].id);
      }
    }
  }

  let pageTitle = $derived($currentPage.charAt(0).toUpperCase() + $currentPage.slice(1));
</script>

<svelte:window onkeydown={handleKeydown} />

<!-- Skip to content (audit L6) -->
<a class="skip-link" href="#main-content">Skip to content</a>

<!-- CRT scan lines — very subtle -->
<div class="scan-lines"></div>

<div class="flex flex-col h-screen overflow-hidden" style="background: var(--bg-deep)">
  <div class="flex flex-1 min-h-0">
    <!-- Sidebar -->
    <aside
      aria-label="Navigation"
      class="hidden md:flex flex-col shrink-0 relative neon-top-line transition-all duration-250"
      class:w-52={!$sidebarCollapsed}
      class:w-14={$sidebarCollapsed}
      style="background: var(--bg-surface); border-right: 1px solid var(--border-color)"
    >
      <!-- Logo -->
      <div class="flex items-center gap-3 px-4 py-4 border-b" style="border-color: var(--border-color)">
        <img src="/amigo-logo.png" alt="amigo-downloader" width="28" height="28" class="rounded shrink-0" />
        {#if !$sidebarCollapsed}
          <div class="min-w-0">
            <h1 class="font-bold text-base leading-tight" style="color: var(--text-primary)">AMIGO</h1>
            <span class="text-[10px] font-mono" style="font-family: 'Share Tech Mono', monospace; color: var(--text-secondary)">v0.1</span>
          </div>
        {/if}
      </div>

      <!-- Add Button -->
      <div class="px-3 py-3">
        {#if $sidebarCollapsed}
          <button
            onclick={() => (showAddDialog = true)}
            class="w-full flex items-center justify-center h-9 rounded-lg font-semibold text-sm transition-colors"
            style="background: var(--neon-primary); color: var(--bg-deep)"
            aria-label="Add download"
          >
            <Icon name="plus" size={18} />
          </button>
        {:else}
          <button
            onclick={() => (showAddDialog = true)}
            class="w-full py-2 rounded-lg font-semibold text-sm transition-colors"
            style="background: var(--neon-primary); color: var(--bg-deep)"
          >
            + ADD DOWNLOAD
            <span class="text-[10px] opacity-60 ml-1">Ctrl+N</span>
          </button>
        {/if}
      </div>

      <!-- Navigation -->
      <nav class="flex-1 px-2 py-1 space-y-0.5">
        {#each navItems as item, i}
          <button
            onclick={() => navigate(item.id)}
            class="w-full flex items-center gap-3 px-3 py-2.5 rounded-lg text-sm font-medium transition-colors"
            class:justify-center={$sidebarCollapsed}
            style={$currentPage === item.id
              ? `background: color-mix(in srgb, var(--neon-primary) 12%, transparent); color: var(--neon-primary)`
              : `color: var(--text-secondary)`}
            aria-label={$sidebarCollapsed ? item.label : undefined}
            title={$sidebarCollapsed ? `${item.label} (${i + 1})` : undefined}
          >
            <Icon name={item.icon} size={18} />
            {#if !$sidebarCollapsed}
              <span class="flex-1 text-left">{item.label}</span>
              <span class="text-[10px] opacity-30">{i + 1}</span>
            {/if}
          </button>
        {/each}
      </nav>

      <!-- Collapse toggle -->
      <div class="px-3 pb-2">
        <button
          onclick={() => sidebarCollapsed.toggle()}
          class="w-full flex items-center justify-center h-8 rounded-lg transition-colors"
          style="color: var(--text-secondary)"
          aria-label={$sidebarCollapsed ? "Expand sidebar" : "Collapse sidebar"}
        >
          <Icon name={$sidebarCollapsed ? "chevron-right" : "chevron-left"} size={16} />
        </button>
      </div>

      <!-- Feedback -->
      {#if !$sidebarCollapsed}
        <div class="px-4 pb-3">
          <button
            onclick={() => { crashReport.set(null); showFeedback = true; }}
            class="text-xs transition-opacity hover:opacity-80"
            style="color: var(--text-secondary); opacity: 0.6; font-size: 12px"
          >
            Feedback &middot; Report Issue
          </button>
        </div>
      {/if}
    </aside>

    <!-- Mobile sidebar overlay -->
    {#if mobileMenuOpen}
      <div class="fixed inset-0 z-50 md:hidden">
        <div class="absolute inset-0 bg-black/60" onclick={() => (mobileMenuOpen = false)}></div>
        <aside
          class="relative w-52 h-full flex flex-col neon-top-line"
          style="background: var(--bg-surface)"
          aria-label="Navigation"
        >
          <div class="flex items-center gap-3 px-4 py-4 border-b" style="border-color: var(--border-color)">
            <img src="/amigo-logo.png" alt="amigo-downloader" width="28" height="28" class="rounded" />
            <h1 class="font-bold text-base" style="color: var(--text-primary)">AMIGO</h1>
          </div>
          <div class="px-3 py-3">
            <button
              onclick={() => { showAddDialog = true; mobileMenuOpen = false; }}
              class="w-full py-2 rounded-lg font-semibold text-sm"
              style="background: var(--neon-primary); color: var(--bg-deep)"
            >
              + ADD DOWNLOAD
            </button>
          </div>
          <nav class="flex-1 px-2 py-1 space-y-0.5">
            {#each navItems as item, i}
              <button
                onclick={() => navigate(item.id)}
                class="w-full flex items-center gap-3 px-3 py-2.5 rounded-lg text-sm font-medium transition-colors"
                style={$currentPage === item.id
                  ? `background: color-mix(in srgb, var(--neon-primary) 12%, transparent); color: var(--neon-primary)`
                  : `color: var(--text-secondary)`}
              >
                <Icon name={item.icon} size={18} />
                <span class="flex-1 text-left">{item.label}</span>
                <span class="text-[10px] opacity-30">{i + 1}</span>
              </button>
            {/each}
          </nav>
        </aside>
      </div>
    {/if}

    <!-- Main content -->
    <main id="main-content" aria-label="Main content" class="flex-1 flex flex-col min-w-0 overflow-hidden" style="background: var(--bg-deep)">
      <!-- Header -->
      <header class="flex items-center justify-between px-6 py-3 border-b" style="border-color: var(--border-color)">
        <div class="flex items-center gap-3">
          <button class="md:hidden p-1" onclick={() => (mobileMenuOpen = !mobileMenuOpen)} aria-label="Toggle navigation">
            <Icon name="menu" size={20} class="text-[var(--text-secondary)]" />
          </button>
          <h2 class="text-xl font-bold" style="font-family: 'Rajdhani', sans-serif; font-weight: 700">{pageTitle}</h2>

          <!-- Protocol segmented control (downloads page only, when usenet enabled) -->
          {#if $currentPage === "downloads" && $features.usenet}
            <div
              role="radiogroup"
              aria-label="Protocol filter"
              class="flex rounded-lg ml-3 overflow-hidden"
              style="background: var(--bg-surface-2); border: 1px solid var(--border-color)"
            >
              {#each protocolOptions as opt}
                <button
                  role="radio"
                  aria-checked={$protocolFilter === opt.value}
                  onclick={() => protocolFilter.set(opt.value)}
                  class="px-3 py-1 text-xs font-semibold transition-colors"
                  style={$protocolFilter === opt.value
                    ? `background: color-mix(in srgb, var(--neon-primary) 15%, transparent); color: var(--neon-primary)`
                    : `color: var(--text-secondary)`}
                  style:font-family="'Share Tech Mono', monospace"
                >
                  {opt.label}
                </button>
              {/each}
            </div>
          {/if}
        </div>

        <button
          onclick={() => theme.toggle()}
          class="p-2 rounded-lg transition-colors"
          style="color: var(--text-secondary)"
          aria-label={$theme === "dark" ? "Switch to Lights On mode" : "Switch to Dark mode"}
        >
          <Icon name={$theme === "dark" ? "sun" : "moon"} size={18} />
        </button>
      </header>

      <!-- Page content -->
      <div class="flex flex-1 min-h-0">
        <div class="flex-1 overflow-y-auto p-6">
          {#key pageKey}
            <div class="page-enter">
              <svelte:boundary onerror={(e) => console.error('Page error:', e)}>
                {#if $currentPage === "downloads"}
                  <Downloads />
                {:else if $currentPage === "plugins"}
                  <Plugins />
                {:else if $currentPage === "history"}
                  <History />
                {:else if $currentPage === "settings"}
                  <Settings />
                {/if}
              </svelte:boundary>
            </div>
          {/key}
        </div>

        <!-- Detail Panel -->
        <DetailPanel />
      </div>
    </main>
  </div>

  <!-- Footer Status Bar -->
  <Footer {speedHistory} />
</div>

<!-- Dialogs -->
{#if showAddDialog}
  <AddDialog onclose={() => (showAddDialog = false)} />
{/if}

{#if showFeedback}
  <FeedbackDialog onclose={() => (showFeedback = false)} />
{/if}

{#if $pendingCaptcha}
  <CaptchaDialog captcha={$pendingCaptcha} onclose={() => pendingCaptcha.set(null)} />
{/if}

<Toasts />
<DropZone />
