<script lang="ts">
  import { onMount } from "svelte";
  import CaptchaDialog from "./components/CaptchaDialog.svelte";
  import DropZone from "./components/DropZone.svelte";
  import FeedbackDialog from "./components/FeedbackDialog.svelte";
  import Icon from "./components/Icon.svelte";
  import ShortcutsDialog from "./components/ShortcutsDialog.svelte";
  import ProgressRing from "./components/ProgressRing.svelte";
  import SidePanel from "./components/SidePanel.svelte";
  import Sparkline from "./components/Sparkline.svelte";
  import Toasts from "./components/Toasts.svelte";
  import {
    addDownload,
    connectWebSocket,
    formatSpeed,
    getConfig,
    getDownloads,
    getStats,
    putConfig,
    type AppConfig,
  } from "./lib/api";
  import {
    applyIntensity,
    ariaAnnouncement,
    closeSidePanel,
    crashReport,
    currentPage,
    downloads,
    features,
    getNeonLabel,
    neonIntensity,
    openAddPanel,
    palette,
    pendingCaptcha,
    protocolFilter,
    pushSpeedSample,
    sidebarCollapsed,
    sidePanelMode,
    speedHistory,
    stats,
    theme,
    updateDownloadProgress,
    updateDownloadStatus,
    wsConnected,
    type CaptchaChallenge,
    type ColorPalette,
    type Page,
    type ProtocolFilter,
  } from "./lib/stores";
  import { locale, tr } from "./lib/i18n";
  import { addToast } from "./lib/toast";
  import Downloads from "./pages/Downloads.svelte";
  import History from "./pages/History.svelte";
  import Plugins from "./pages/Plugins.svelte";
  import Settings from "./pages/Settings.svelte";

  let showFeedback = $state(false);
  let showShortcuts = $state(false);
  let showPalette = $state(false);
  let mobileMenuOpen = $state(false);
  let pageKey = $state(0);

  // Bandwidth limit
  let appConfig = $state<AppConfig | null>(null);
  let limitEnabled = $state(false);
  let limitMbps = $state(5);

  // Overall progress for active downloads
  let overallProgress = $derived(() => {
    const active = $downloads.filter((d) => d.status === "downloading");
    if (active.length === 0) return 0;
    const totalBytes = active.reduce((sum, d) => sum + (d.filesize ?? 0), 0);
    const dlBytes = active.reduce((sum, d) => sum + d.bytes_downloaded, 0);
    return totalBytes > 0 ? Math.round((dlBytes / totalBytes) * 100) : 0;
  });

  const mainNavItems: { id: Page; label: string; icon: string }[] = [
    { id: "downloads", label: "Downloads", icon: "arrow-down" },
    { id: "plugins", label: "Plugins", icon: "puzzle" },
    { id: "history", label: "History", icon: "clock" },
  ];

  const mgmtNavItems: { id: Page; label: string; icon: string }[] = [
    { id: "settings", label: "Settings", icon: "gear" },
  ];

  const paletteOptions: { id: ColorPalette; label: string; color: string }[] = [
    { id: "blue", label: "Blue", color: "#3b82f6" },
    { id: "teal", label: "Teal", color: "#14b8a6" },
    { id: "indigo", label: "Indigo", color: "#6366f1" },
    { id: "amber", label: "Amber", color: "#f59e0b" },
    { id: "violet", label: "Violet", color: "#8b5cf6" },
    { id: "rose", label: "Rose", color: "#f43f5e" },
  ];

  const protocolOptions: { value: ProtocolFilter; label: string }[] = [
    { value: "all", label: "ALL" },
    { value: "http", label: "HTTP" },
    { value: "usenet", label: "USENET" },
  ];

  onMount(() => {
    // Initialize theme + palette + intensity
    // Apply theme class
    document.documentElement.classList.toggle("light", $theme === "light");
    document.documentElement.classList.add(`palette-${$palette}`);
    applyIntensity($neonIntensity);

    handleShareTarget();
    loadData();
    const interval = setInterval(loadData, 10000);

    connectWebSocket((msg) => {
      if (msg.type === "progress") {
        const p = msg.data?.progress as
          | {
              bytes_downloaded?: number;
              total_bytes?: number;
              speed_bytes_per_sec?: number;
            }
          | undefined;
        if (p) {
          updateDownloadProgress(
            msg.id,
            p.bytes_downloaded ?? 0,
            p.speed_bytes_per_sec ?? 0,
            p.total_bytes ?? undefined,
          );
        }
        return;
      }

      if (msg.type === "completed") {
        updateDownloadStatus(msg.id, "completed");
        const fname = (msg.data?.filename as string) || msg.id;
        addToast("success", "Download complete", fname);
        ariaAnnouncement.set(`Download complete: ${fname}`);
        notifyIfHidden("Download complete", fname);
      } else if (msg.type === "failed") {
        updateDownloadStatus(msg.id, "failed");
        const errMsg = (msg.data?.error as string) || msg.id;
        addToast("error", "Download failed", errMsg);
        ariaAnnouncement.set(`Download failed: ${errMsg}`);
        notifyIfHidden("Download failed", errMsg);
      } else if (msg.type === "captcha_challenge") {
        pendingCaptcha.set(msg.data as unknown as CaptchaChallenge);
      } else if (
        msg.type === "captcha_solved" ||
        msg.type === "captcha_timeout"
      ) {
        pendingCaptcha.set(null);
      } else if (msg.type === "plugin_notification") {
        addToast(
          "info",
          (msg.data?.title as string) || "Plugin",
          msg.data?.message as string,
        );
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
      addDownload(extracted)
        .then(() => {
          addToast("success", "Download added!", extracted);
          loadData();
        })
        .catch(() => {
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
      const [dl, st, cfg] = await Promise.all([
        getDownloads(),
        getStats(),
        getConfig(),
      ]);
      downloads.set(dl);
      stats.set(st);
      pushSpeedSample(st.speed_bytes_per_sec ?? 0);
      if (cfg) {
        if (cfg.features) features.set(cfg.features);
        if (!appConfig) {
          // First load — init bandwidth limit state
          appConfig = cfg;
          const limit = cfg.bandwidth?.global_limit ?? 0;
          limitEnabled = limit > 0;
          limitMbps = limit > 0 ? Math.round(limit / 1_000_000) : 5;
        }
      }
    } catch {
      // Server offline
    }
  }

  function notifyIfHidden(title: string, body: string) {
    if (document.hasFocus()) return;
    if (typeof Notification === "undefined") return;
    if (Notification.permission === "granted") {
      new Notification(title, { body, icon: "/amigo-logo.png" });
    } else if (Notification.permission !== "denied") {
      Notification.requestPermission().then((p) => {
        if (p === "granted") new Notification(title, { body, icon: "/amigo-logo.png" });
      });
    }
  }

  async function saveBandwidthLimit() {
    if (!appConfig) return;
    const limitBytes = limitEnabled ? limitMbps * 1_000_000 : 0;
    appConfig.bandwidth.global_limit = limitBytes;
    try {
      await putConfig(appConfig);
      addToast("info", limitEnabled ? `Limit set to ${limitMbps} MB/s` : "Speed limit disabled");
    } catch {
      addToast("error", "Failed to save limit");
    }
  }

  function navigate(page: Page) {
    currentPage.set(page);
    mobileMenuOpen = false;
    pageKey++;
  }

  // Single global keyboard handler
  function handleKeydown(e: KeyboardEvent) {
    if ((e.ctrlKey || e.metaKey) && (e.key === "n" || e.key === "l")) {
      e.preventDefault();
      openAddPanel();
    }
    if (e.key === "Escape") {
      if ($sidePanelMode) {
        closeSidePanel();
        return;
      }
      if (showShortcuts) {
        showShortcuts = false;
        return;
      }
      if (showFeedback) {
        showFeedback = false;
        return;
      }
      if (showPalette) {
        showPalette = false;
        return;
      }
      mobileMenuOpen = false;
    }
    if (e.key === "?" && !e.ctrlKey && !e.metaKey && !e.altKey) {
      showShortcuts = !showShortcuts;
      return;
    }
    const allNav = [...mainNavItems, ...mgmtNavItems];
    if (
      !$sidePanelMode &&
      !showFeedback &&
      !showShortcuts &&
      !e.ctrlKey &&
      !e.metaKey &&
      !e.altKey
    ) {
      const num = parseInt(e.key);
      if (num >= 1 && num <= allNav.length) {
        navigate(allNav[num - 1].id);
      }
    }
  }

  let pageTitle = $derived(
    $currentPage.charAt(0).toUpperCase() + $currentPage.slice(1),
  );
  let neonLabel = $derived(getNeonLabel($neonIntensity));
</script>

<svelte:window onkeydown={handleKeydown} />

<!-- Skip to content (audit L6) -->
<a class="skip-link" href="#main-content">Skip to content</a>

<div class="flex h-screen overflow-hidden" style="background: var(--bg-deep)">
  <!-- Sidebar -->
  <aside
    aria-label="Navigation"
    class="hidden md:flex flex-col shrink-0 relative neon-top-line transition-all duration-200"
    style="width: {$sidebarCollapsed ? '56px' : '256px'}; background: var(--bg-surface); border-right: 1px solid var(--border-color)"
  >
    <!-- Logo -->
    <div
      class="sidebar-logo flex items-center gap-3 px-4 py-4 border-b overflow-hidden"
      style="border-color: var(--border-color)"
    >
      <img
        src="/amigo-logo.png"
        alt="amigo-downloader"
        width="28"
        height="28"
        class="shrink-0"
      />
      {#if !$sidebarCollapsed}
        <div class="min-w-0">
          <h1 class="font-bold text-base leading-tight" style="color: var(--neon-primary)">AMIGO</h1>
          <p class="text-xs" style="color: var(--text-secondary)">Download Manager</p>
        </div>
      {/if}
    </div>

    <!-- Add download trigger -->
    {#if $sidebarCollapsed}
      <button class="icon-btn flex items-center justify-center p-3 mx-1 my-2 rounded-lg" onclick={() => openAddPanel()} aria-label={tr($locale, "sidebar.add")} style="color: var(--text-secondary)">
        <Icon name="plus" size={18} />
      </button>
    {:else}
      <button class="search-trigger" onclick={() => openAddPanel()}>
        <Icon name="plus" size={16} />
        <span>{tr($locale, "sidebar.add")}</span>
        <kbd>Ctrl+N</kbd>
      </button>
    {/if}

    <!-- Navigation -->
    <nav class="flex-1 px-2 py-1">
      {#each mainNavItems as item, i}
        <button
          onclick={() => navigate(item.id)}
          class="nav-link w-full"
          class:active={$currentPage === item.id}
          title={$sidebarCollapsed ? item.label : undefined}
        >
          <Icon name={item.icon} size={18} />
          {#if !$sidebarCollapsed}
            <span class="flex-1 text-left">{item.label}</span>
            <span class="text-[10px] opacity-30">{i + 1}</span>
          {/if}
        </button>
      {/each}

      {#if !$sidebarCollapsed}
        <div class="nav-section-label">{tr($locale, "nav.management")}</div>
      {/if}

      {#each mgmtNavItems as item, i}
        <button
          onclick={() => navigate(item.id)}
          class="nav-link w-full"
          class:active={$currentPage === item.id}
          title={$sidebarCollapsed ? item.label : undefined}
        >
          <Icon name={item.icon} size={18} />
          {#if !$sidebarCollapsed}
            <span class="flex-1 text-left">{item.label}</span>
            <span class="text-[10px] opacity-30">{mainNavItems.length + i + 1}</span>
          {/if}
        </button>
      {/each}
    </nav>

    <!-- Stats + Speed Graph -->
    {#if !$sidebarCollapsed}
      <div
        class="px-4 py-3 border-t flex flex-col gap-1.5"
        style="border-color: var(--border-color)"
      >
        <div class="flex items-center justify-between">
          <span class="stat-label">{tr($locale, "sidebar.speed")}</span>
          <span style="color: var(--neon-primary); font-family: var(--font-mono); font-size: var(--font-xs, 0.75rem)">
            {formatSpeed($stats.speed_bytes_per_sec)}
          </span>
        </div>
        {#if $speedHistory.length > 1}
          <Sparkline values={$speedHistory} width={200} height={28} />
        {/if}

        <!-- Bandwidth limit -->
        <div class="flex items-center gap-2 mt-1">
          <span class="stat-label shrink-0">{tr($locale, "sidebar.limit")}</span>
          <button
            onclick={() => { limitEnabled = !limitEnabled; saveBandwidthLimit(); }}
            class="text-[10px] font-semibold px-1.5 py-0.5 rounded"
            style={limitEnabled
              ? "background: color-mix(in srgb, var(--neon-primary) 15%, transparent); color: var(--neon-primary)"
              : "background: var(--bg-surface-2); color: var(--text-secondary)"}
          >
            {limitEnabled ? "On" : "Off"}
          </button>
          <input
            type="number"
            min="1"
            max="1000"
            bind:value={limitMbps}
            onchange={saveBandwidthLimit}
            disabled={!limitEnabled}
            class="w-12 text-center rounded px-1 py-0.5 text-xs"
            style="font-family: var(--font-mono); background: var(--bg-surface-2); border: 1px solid var(--border-color); color: {limitEnabled ? 'var(--text-primary)' : 'var(--text-secondary)'}; opacity: {limitEnabled ? 1 : 0.4}"
            aria-label="Speed limit in MB/s"
          />
          <span class="text-[10px]" style="color: var(--text-secondary)">MB/s</span>
        </div>

        <div class="flex items-center justify-between mt-1">
          <span class="stat-label">{tr($locale, "sidebar.active")}</span>
          <span class="flex items-center gap-1.5 text-xs" style="color: var(--text-primary)">
            {$stats.active_downloads}
            {#if $stats.active_downloads > 0}
              <ProgressRing progress={overallProgress()} size={18} stroke={2} active={true} />
            {/if}
          </span>
        </div>
        <div class="flex items-center justify-between">
          <span class="stat-label">{tr($locale, "sidebar.queued")}</span>
          <span class="text-xs" style="color: var(--text-primary)">{$stats.queued}</span>
        </div>
        <div class="flex items-center justify-between">
          <span class="stat-label">{tr($locale, "sidebar.done")}</span>
          <span class="text-xs" style="color: var(--status-online)">{$stats.completed}</span>
        </div>
      </div>
    {:else}
      <!-- Collapsed: just show progress ring -->
      <div class="flex flex-col items-center gap-2 py-3 border-t" style="border-color: var(--border-color)">
        {#if $stats.active_downloads > 0}
          <ProgressRing progress={overallProgress()} size={24} stroke={2.5} active={true} />
        {/if}
      </div>
    {/if}

    <!-- Sidebar footer -->
    <div class="px-2 py-3 border-t" style="border-color: var(--border-color)">
      {#if !$sidebarCollapsed}
        <!-- Palette picker -->
        {#if showPalette}
          <div class="flex items-center gap-2 mb-3 px-2">
            {#each paletteOptions as opt}
              <button
                class="color-swatch"
                class:active={$palette === opt.id}
                style="--swatch-color: {opt.color}; background: {opt.color}"
                aria-label={opt.label}
                onclick={() => { palette.set(opt.id); showPalette = false; }}
              ></button>
            {/each}
          </div>
        {/if}

        <!-- Neon intensity slider -->
        <div class="neon-slider mb-2 px-2">
          <Icon name="bolt" size={14} />
          <input
            type="range" min="0" max="1" step="0.25" value={$neonIntensity}
            oninput={(e) => neonIntensity.set(parseFloat((e.target as HTMLInputElement).value))}
            aria-label="Neon intensity"
          />
          <span class="neon-slider-label">{neonLabel}</span>
        </div>

        <!-- Controls row -->
        <div class="flex items-center justify-between px-2">
          <!-- Connection status -->
          <div class="flex items-center gap-1.5">
            <span
              class="w-2 h-2 rounded-full shrink-0"
              style="background: {$wsConnected ? 'var(--status-online)' : 'var(--status-error)'}; box-shadow: 0 0 4px {$wsConnected ? 'var(--status-online)' : 'var(--status-error)'}"
            ></span>
            <span class="text-[10px]" style="color: var(--text-secondary); font-family: var(--font-mono)">v0.1</span>
          </div>
          <div class="flex items-center gap-1">
            <button class="icon-btn p-1.5 rounded-lg" style="color: var(--text-secondary)" aria-label="Color theme" onclick={() => (showPalette = !showPalette)}>
              <div class="w-4 h-4 rounded-full" style="background: var(--neon-primary); box-shadow: var(--neon-glow-sm)"></div>
            </button>
            <button onclick={() => theme.toggle()} class="icon-btn p-1.5 rounded-lg" style="color: var(--text-secondary)" aria-label={$theme === "dark" ? "Switch to Light mode" : "Switch to Dark mode"}>
              <Icon name={$theme === "light" ? "moon" : "sun"} size={16} />
            </button>
          </div>
        </div>
      {/if}

      <!-- Collapse toggle -->
      <button
        onclick={() => sidebarCollapsed.toggle()}
        class="icon-btn w-full flex items-center justify-center p-1.5 rounded-lg mt-1"
        style="color: var(--text-secondary)"
        aria-label={$sidebarCollapsed ? "Expand sidebar" : "Collapse sidebar"}
      >
        <Icon name={$sidebarCollapsed ? "chevron-right" : "chevron-left"} size={14} />
      </button>
    </div>
  </aside>

  <!-- Mobile sidebar overlay -->
  {#if mobileMenuOpen}
    <div class="fixed inset-0 z-50 md:hidden">
      <button
        class="absolute inset-0 bg-black/60"
        onclick={() => (mobileMenuOpen = false)}
        aria-label="Close navigation"
      ></button>
      <aside
        class="relative w-64 h-full flex flex-col neon-top-line"
        style="background: var(--bg-surface)"
        aria-label="Navigation"
      >
        <div
          class="sidebar-logo flex items-center gap-3 px-4 py-4 border-b"
          style="border-color: var(--border-color)"
        >
          <img
            src="/amigo-logo.png"
            alt="amigo-downloader"
            width="40"
            height="40"
          />
          <h1 class="font-bold text-base" style="color: var(--neon-primary)">
            AMIGO
          </h1>
        </div>
        <button
          class="search-trigger"
          onclick={() => {
            openAddPanel();
            mobileMenuOpen = false;
          }}
        >
          <Icon name="plus" size={16} />
          <span>Add Download...</span>
        </button>
        <nav class="flex-1 px-2 py-1">
          {#each mainNavItems as item, i}
            <button
              onclick={() => navigate(item.id)}
              class="nav-link w-full"
              class:active={$currentPage === item.id}
            >
              <Icon name={item.icon} size={18} />
              <span class="flex-1 text-left">{item.label}</span>
              <span class="text-[10px] opacity-30">{i + 1}</span>
            </button>
          {/each}
          <div class="nav-section-label">Management</div>
          {#each mgmtNavItems as item, i}
            <button
              onclick={() => navigate(item.id)}
              class="nav-link w-full"
              class:active={$currentPage === item.id}
            >
              <Icon name={item.icon} size={18} />
              <span class="flex-1 text-left">{item.label}</span>
            </button>
          {/each}
        </nav>
      </aside>
    </div>
  {/if}

  <!-- Main content -->
  <main
    id="main-content"
    aria-label="Main content"
    class="flex-1 flex flex-col min-w-0 overflow-hidden"
    style="background: var(--bg-deep)"
  >
    <!-- Header -->
    <header
      class="flex items-center justify-between px-8 py-3 border-b"
      style="border-color: var(--border-color)"
    >
      <div class="flex items-center gap-3">
        <button
          class="md:hidden p-1"
          onclick={() => (mobileMenuOpen = !mobileMenuOpen)}
          aria-label="Toggle navigation"
        >
          <Icon name="menu" size={20} class="text-[var(--text-secondary)]" />
        </button>
        <h2
          class="text-xl font-semibold neon-flicker-text"
          style="color: var(--text-primary)"
        >
          {pageTitle}
        </h2>

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
                style:font-family="var(--font-mono)"
              >
                {opt.label}
              </button>
            {/each}
          </div>
        {/if}
      </div>
      <button
        onclick={() => openAddPanel()}
        class="icon-btn p-2 rounded-lg min-w-[44px] min-h-[44px] flex items-center justify-center"
        style="color: var(--text-secondary)"
        aria-label="Add download"
      >
        <Icon name="plus" size={20} />
      </button>
    </header>

    <!-- Page content -->
    <div class="flex flex-1 min-h-0">
      <div class="flex-1 overflow-y-auto p-8 md:p-8 max-md:p-4">
        {#key pageKey}
          <div class="page-enter">
            <svelte:boundary onerror={(e) => console.error("Page error:", e)}>
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

      <!-- Side Panel (detail or add) -->
      <SidePanel />
    </div>
  </main>
</div>

<!-- Dialogs -->
{#if showShortcuts}
  <ShortcutsDialog onclose={() => (showShortcuts = false)} />
{/if}

{#if showFeedback}
  <FeedbackDialog onclose={() => (showFeedback = false)} />
{/if}

{#if $pendingCaptcha}
  <CaptchaDialog
    captcha={$pendingCaptcha}
    onclose={() => pendingCaptcha.set(null)}
  />
{/if}

<!-- aria-live announcer for screen readers -->
<div aria-live="polite" class="sr-only">{$ariaAnnouncement}</div>

<Toasts />
<DropZone />
