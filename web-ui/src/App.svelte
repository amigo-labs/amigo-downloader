<script lang="ts">
  import { onMount } from "svelte";
  import AddDialog from "./components/AddDialog.svelte";
  import CaptchaDialog from "./components/CaptchaDialog.svelte";
  import DetailPanel from "./components/DetailPanel.svelte";
  import DropZone from "./components/DropZone.svelte";
  import FeedbackDialog from "./components/FeedbackDialog.svelte";
  import Icon from "./components/Icon.svelte";
  import Toasts from "./components/Toasts.svelte";
  import {
    addDownload,
    connectWebSocket,
    formatSpeed,
    getConfig,
    getDownloads,
    getStats,
  } from "./lib/api";
  import {
    applyIntensity,
    crashReport,
    currentPage,
    downloads,
    features,
    getNeonLabel,
    neonIntensity,
    palette,
    pendingCaptcha,
    protocolFilter,
    selectedDownloadId,
    stats,
    theme,
    updateDownloadProgress,
    updateDownloadStatus,
    type CaptchaChallenge,
    type ColorPalette,
    type Page,
    type ProtocolFilter,
  } from "./lib/stores";
  import { addToast } from "./lib/toast";
  import Downloads from "./pages/Downloads.svelte";
  import History from "./pages/History.svelte";
  import Plugins from "./pages/Plugins.svelte";
  import Settings from "./pages/Settings.svelte";

  let showAddDialog = $state(false);
  let showFeedback = $state(false);
  let showPalette = $state(false);
  let mobileMenuOpen = $state(false);
  let pageKey = $state(0);

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
    document.documentElement.classList.toggle(
      "lights-on",
      $theme === "lights-on",
    );
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
        addToast(
          "success",
          "Download complete",
          (msg.data?.filename as string) || msg.id,
        );
      } else if (msg.type === "failed") {
        updateDownloadStatus(msg.id, "failed");
        addToast(
          "error",
          "Download failed",
          (msg.data?.error as string) || msg.id,
        );
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
      if (cfg?.features) features.set(cfg.features);
    } catch {
      // Server offline
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
      showAddDialog = true;
    }
    if (e.key === "Escape") {
      if (showAddDialog) {
        showAddDialog = false;
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
      if ($selectedDownloadId) {
        selectedDownloadId.set(null);
        return;
      }
      mobileMenuOpen = false;
    }
    const allNav = [...mainNavItems, ...mgmtNavItems];
    if (
      !showAddDialog &&
      !showFeedback &&
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
  <!-- Sidebar (Layout: fixed 16rem) -->
  <aside
    aria-label="Navigation"
    class="hidden md:flex flex-col shrink-0 w-64 relative neon-top-line"
    style="background: var(--bg-surface); border-right: 1px solid var(--border-color)"
  >
    <!-- Logo -->
    <div
      class="sidebar-logo flex items-center gap-3 px-4 py-4 border-b"
      style="border-color: var(--border-color)"
    >
      <img
        src="/amigo-logo.png"
        alt="amigo-downloader"
        width="40"
        height="40"
        class="shrink-0"
      />
      <div class="min-w-0">
        <h1
          class="font-bold text-base leading-tight"
          style="color: var(--neon-primary)"
        >
          AMIGO
        </h1>
        <p class="text-xs" style="color: var(--text-secondary)">
          Download Manager
        </p>
      </div>
    </div>

    <!-- Search trigger -->
    <button class="search-trigger" onclick={() => (showAddDialog = true)}>
      <Icon name="search" size={16} />
      <span>Add URL...</span>
      <kbd>Ctrl+N</kbd>
    </button>

    <!-- Navigation -->
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
          <span class="text-[10px] opacity-30"
            >{mainNavItems.length + i + 1}</span
          >
        </button>
      {/each}
    </nav>

    <!-- Stats (moved from footer) -->
    <div
      class="px-4 py-3 border-t flex flex-col gap-1.5"
      style="border-color: var(--border-color)"
    >
      <div class="flex items-center justify-between">
        <span class="stat-label">Speed</span>
        <span
          style="color: var(--neon-primary); font-family: var(--font-mono); font-size: var(--font-xs, 0.75rem)"
        >
          {formatSpeed($stats.speed_bytes_per_sec)}
        </span>
      </div>
      <div class="flex items-center justify-between">
        <span class="stat-label">Active</span>
        <span class="text-xs" style="color: var(--text-primary)">
          {$stats.active_downloads}
          {#if $stats.active_downloads > 0}
            <span class="status-dot status-dot--online ml-1"></span>
          {/if}
        </span>
      </div>
      <div class="flex items-center justify-between">
        <span class="stat-label">Queued</span>
        <span class="text-xs" style="color: var(--text-primary)"
          >{$stats.queued}</span
        >
      </div>
      <div class="flex items-center justify-between">
        <span class="stat-label">Done</span>
        <span class="text-xs" style="color: var(--status-online)"
          >{$stats.completed}</span
        >
      </div>
    </div>

    <!-- Sidebar footer: theme controls -->
    <div class="px-4 py-3 border-t" style="border-color: var(--border-color)">
      <!-- Palette picker -->
      {#if showPalette}
        <div class="flex items-center gap-2 mb-3">
          {#each paletteOptions as opt}
            <button
              class="color-swatch"
              class:active={$palette === opt.id}
              style="--swatch-color: {opt.color}; background: {opt.color}"
              aria-label={opt.label}
              onclick={() => {
                palette.set(opt.id);
                showPalette = false;
              }}
            ></button>
          {/each}
        </div>
      {/if}

      <!-- Neon intensity slider -->
      <div class="neon-slider mb-2">
        <Icon name="bolt" size={14} />
        <input
          type="range"
          min="0"
          max="1"
          step="0.25"
          value={$neonIntensity}
          oninput={(e) =>
            neonIntensity.set(parseFloat((e.target as HTMLInputElement).value))}
          aria-label="Neon intensity"
        />
        <span class="neon-slider-label">{neonLabel}</span>
      </div>

      <!-- Bottom row: version + controls -->
      <div class="flex items-center justify-between">
        <span
          class="text-[10px]"
          style="color: var(--text-secondary); font-family: var(--font-mono)"
          >v0.1</span
        >
        <div class="flex items-center gap-1">
          <button
            class="p-1.5 rounded-lg transition-colors"
            style="color: var(--text-secondary)"
            aria-label="Color theme"
            onclick={() => (showPalette = !showPalette)}
          >
            <div
              class="w-4 h-4 rounded-full"
              style="background: var(--neon-primary); box-shadow: var(--neon-glow-sm)"
            ></div>
          </button>
          <button
            onclick={() => theme.toggle()}
            class="p-1.5 rounded-lg transition-colors"
            style="color: var(--text-secondary)"
            aria-label={$theme === "dark"
              ? "Switch to Lights On mode"
              : "Switch to Dark mode"}
          >
            <Icon name={$theme === "dark" ? "sun" : "moon"} size={16} />
          </button>
        </div>
      </div>

      <!-- Feedback link -->
      <button
        onclick={() => {
          crashReport.set(null);
          showFeedback = true;
        }}
        class="mt-2 text-xs transition-opacity hover:opacity-80"
        style="color: var(--text-secondary); opacity: 0.5"
      >
        Feedback
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
            showAddDialog = true;
            mobileMenuOpen = false;
          }}
        >
          <Icon name="search" size={16} />
          <span>Add URL...</span>
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

      <!-- Detail Panel -->
      <DetailPanel />
    </div>
  </main>
</div>

<!-- Dialogs -->
{#if showAddDialog}
  <AddDialog onclose={() => (showAddDialog = false)} />
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

<Toasts />
<DropZone />
