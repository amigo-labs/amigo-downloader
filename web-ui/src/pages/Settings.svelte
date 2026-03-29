<script lang="ts">
  import { onMount } from "svelte";
  import { theme, layout, accent, features, type AccentColor, type LayoutMode, type ThemeMode } from "../lib/stores";
  import { getConfig, putConfig, getWebhooks, createWebhook, deleteWebhook, testWebhook, type AppConfig } from "../lib/api";
  import { addToast } from "../lib/toast";

  // Full config state
  let config = $state<AppConfig | null>(null);

  // Webhook state
  let webhooks = $state<any[]>([]);
  let showAddWebhook = $state(false);
  let newWebhookName = $state("");
  let newWebhookUrl = $state("");
  let newWebhookSecret = $state("");
  let newWebhookEvents = $state("*");

  onMount(async () => {
    try {
      const [cfg, wh] = await Promise.all([getConfig(), getWebhooks()]);
      config = cfg;
      webhooks = wh;
      // Sync feature flags store
      if (cfg.features) {
        features.set(cfg.features);
      }
    } catch { /* server offline */ }
  });

  async function saveConfig() {
    if (!config) return;
    try {
      config = await putConfig(config);
      features.set(config.features);
      addToast("success", "Settings saved");
    } catch {
      addToast("error", "Failed to save settings");
    }
  }

  function toggleUsenetProc(key: string) {
    if (!config) return;
    (config.usenet as any)[key] = !(config.usenet as any)[key];
    saveConfig();
  }

  function toggleFeature(key: "rss_feeds" | "server_stats") {
    if (!config) return;
    config.features[key] = !config.features[key];
    saveConfig();
  }

  async function handleAddWebhook() {
    if (!newWebhookName.trim() || !newWebhookUrl.trim()) return;
    try {
      const events = newWebhookEvents.split(",").map(e => e.trim()).filter(Boolean);
      await createWebhook({
        name: newWebhookName,
        url: newWebhookUrl,
        secret: newWebhookSecret || undefined,
        events,
      });
      webhooks = await getWebhooks();
      showAddWebhook = false;
      newWebhookName = "";
      newWebhookUrl = "";
      newWebhookSecret = "";
      newWebhookEvents = "*";
      addToast("success", "Webhook added");
    } catch {
      addToast("error", "Failed to add webhook");
    }
  }

  async function handleDeleteWebhook(id: string) {
    try {
      await deleteWebhook(id);
      webhooks = webhooks.filter(w => w.id !== id);
      addToast("info", "Webhook removed");
    } catch {
      addToast("error", "Failed to delete webhook");
    }
  }

  async function handleTestWebhook(id: string) {
    try {
      const result = await testWebhook(id);
      addToast("success", "Test sent", `Status: ${result.status || "OK"}`);
    } catch {
      addToast("error", "Test failed");
    }
  }

  const accentColors: { id: AccentColor; label: string; hex: string }[] = [
    { id: "blue", label: "Blue", hex: "#3b82f6" },
    { id: "green", label: "Green", hex: "#10b981" },
    { id: "purple", label: "Purple", hex: "#8b5cf6" },
    { id: "coral", label: "Coral", hex: "#f43f5e" },
    { id: "orange", label: "Orange", hex: "#f97316" },
    { id: "cyan", label: "Cyan", hex: "#06b6d4" },
  ];
</script>

{#if config}
<div class="max-w-2xl space-y-8">
  <!-- Optional Features -->
  <section>
    <h3 class="text-lg font-bold mb-4">Features</h3>
    <div class="rounded-xl p-5 space-y-4" style="background: var(--surface-2-color); border: 1px solid var(--border-color)">
      <div class="flex items-center justify-between">
        <div>
          <p class="text-sm font-semibold">RSS Feeds</p>
          <p class="text-xs" style="color: var(--text-secondary-color)">
            Monitor RSS/Atom feeds for automatic NZB import
          </p>
        </div>
        <button
          onclick={() => toggleFeature("rss_feeds")}
          class="w-12 h-6 rounded-full relative transition-colors"
          style="background: {config.features.rss_feeds ? 'var(--accent-color)' : 'var(--surface-3-color)'}"
        >
          <span
            class="absolute top-0.5 w-5 h-5 rounded-full bg-white transition-all shadow"
            style="left: {config.features.rss_feeds ? '1.625rem' : '0.125rem'}"
          ></span>
        </button>
      </div>
      <div class="flex items-center justify-between">
        <div>
          <p class="text-sm font-semibold">Server Statistics</p>
          <p class="text-xs" style="color: var(--text-secondary-color)">
            Show per-server connection stats in Usenet UI
          </p>
        </div>
        <button
          onclick={() => toggleFeature("server_stats")}
          class="w-12 h-6 rounded-full relative transition-colors"
          style="background: {config.features.server_stats ? 'var(--accent-color)' : 'var(--surface-3-color)'}"
        >
          <span
            class="absolute top-0.5 w-5 h-5 rounded-full bg-white transition-all shadow"
            style="left: {config.features.server_stats ? '1.625rem' : '0.125rem'}"
          ></span>
        </button>
      </div>
    </div>
  </section>

  <!-- Usenet Post-Processing -->
  <section>
    <h3 class="text-lg font-bold mb-4">Usenet Post-Processing</h3>
    <div class="rounded-xl p-5 space-y-4" style="background: var(--surface-2-color); border: 1px solid var(--border-color)">
      {#each [
        { key: "par2_repair", label: "PAR2 Verify & Repair", desc: "Check file integrity and repair damaged files using PAR2 recovery data" },
        { key: "selective_par2", label: "Selective PAR2", desc: "Only download recovery volumes when repair is needed. Saves bandwidth. Disable to pre-download all PAR2 files." },
        { key: "auto_unrar", label: "Auto-Extract Archives", desc: "Automatically extract RAR, ZIP, and 7z archives after download" },
        { key: "sequential_postprocess", label: "Sequential Mode (low-power)", desc: "Run PAR2 and extraction one after another instead of parallel. Recommended for Raspberry Pi and NAS devices." },
        { key: "delete_archives_after_extract", label: "Delete Archives After Extract", desc: "Remove archive files after successful extraction" },
        { key: "delete_par2_after_repair", label: "Delete PAR2 After Repair", desc: "Remove PAR2 files after successful verification or repair" },
      ] as opt (opt.key)}
        <div class="flex items-center justify-between">
          <div>
            <p class="text-sm font-semibold">{opt.label}</p>
            <p class="text-xs" style="color: var(--text-secondary-color)">{opt.desc}</p>
          </div>
          <button
            onclick={() => toggleUsenetProc(opt.key)}
            class="w-12 h-6 rounded-full relative transition-colors shrink-0 ml-4"
            style="background: {(config.usenet as any)[opt.key] ? 'var(--accent-color)' : 'var(--surface-3-color)'}"
          >
            <span
              class="absolute top-0.5 w-5 h-5 rounded-full bg-white transition-all shadow"
              style="left: {(config.usenet as any)[opt.key] ? '1.625rem' : '0.125rem'}"
            ></span>
          </button>
        </div>
      {/each}
    </div>
  </section>

  <!-- Appearance -->
  <section>
    <h3 class="text-lg font-bold mb-4">Appearance</h3>

    <!-- Theme Mode -->
    <div class="rounded-xl p-5 mb-4" style="background: var(--surface-2-color); border: 1px solid var(--border-color)">
      <label class="text-sm font-semibold mb-3 block">Theme</label>
      <div class="flex gap-3">
        {#each ["light", "dark"] as mode}
          <button
            onclick={() => theme.set(mode as ThemeMode)}
            class="flex-1 py-3 rounded-xl text-sm font-medium transition-all capitalize"
            style={$theme === mode
              ? "background: var(--accent-color); color: white; box-shadow: 0 4px 12px color-mix(in srgb, var(--accent-color) 40%, transparent)"
              : "background: var(--surface-3-color); color: var(--text-secondary-color)"}
          >
            {mode === "light" ? "Light" : "Dark"}
          </button>
        {/each}
      </div>
    </div>

    <!-- Layout Mode -->
    <div class="rounded-xl p-5 mb-4" style="background: var(--surface-2-color); border: 1px solid var(--border-color)">
      <label class="text-sm font-semibold mb-3 block">Layout</label>
      <div class="flex gap-3">
        {#each ["modern", "classic"] as mode}
          <button
            onclick={() => layout.set(mode as LayoutMode)}
            class="flex-1 py-3 rounded-xl text-sm font-medium transition-all capitalize"
            style={$layout === mode
              ? "background: var(--accent-color); color: white; box-shadow: 0 4px 12px color-mix(in srgb, var(--accent-color) 40%, transparent)"
              : "background: var(--surface-3-color); color: var(--text-secondary-color)"}
          >
            {mode}
          </button>
        {/each}
      </div>
      <p class="text-xs mt-2" style="color: var(--text-secondary-color)">
        Modern: card-based layout. Classic: compact table view.
      </p>
    </div>

    <!-- Accent Color -->
    <div class="rounded-xl p-5" style="background: var(--surface-2-color); border: 1px solid var(--border-color)">
      <label class="text-sm font-semibold mb-3 block">Accent Color</label>
      <div class="flex gap-3 flex-wrap">
        {#each accentColors as color}
          <button
            onclick={() => accent.set(color.id)}
            class="w-10 h-10 rounded-full transition-all hover:scale-110"
            style="background: {color.hex}; {$accent === color.id ? 'box-shadow: 0 0 0 3px var(--surface-color), 0 0 0 5px ' + color.hex + '; transform: scale(1.1)' : ''}"
            title={color.label}
          ></button>
        {/each}
      </div>
    </div>
  </section>

  <!-- Downloads -->
  <section>
    <h3 class="text-lg font-bold mb-4">Downloads</h3>
    <div class="rounded-xl p-5 space-y-4" style="background: var(--surface-2-color); border: 1px solid var(--border-color)">
      <div>
        <label class="text-sm font-semibold mb-1.5 block">Download Directory</label>
        <input
          type="text"
          bind:value={config.download_dir}
          class="w-full rounded-lg px-4 py-2.5 text-sm font-mono outline-none"
          style="background: var(--surface-3-color); border: 1px solid var(--border-color); color: var(--text-color)"
        />
      </div>
      <div>
        <label class="text-sm font-semibold mb-1.5 block">Max Concurrent Downloads</label>
        <input
          type="number"
          bind:value={config.max_concurrent_downloads}
          min="1"
          max="50"
          class="w-32 rounded-lg px-4 py-2.5 text-sm font-mono outline-none"
          style="background: var(--surface-3-color); border: 1px solid var(--border-color); color: var(--text-color)"
        />
      </div>
      <div>
        <label class="text-sm font-semibold mb-1.5 block">Global Speed Limit</label>
        <div class="flex items-center gap-2">
          <input
            type="number"
            bind:value={config.bandwidth.global_limit}
            min="0"
            class="w-32 rounded-lg px-4 py-2.5 text-sm font-mono outline-none"
            style="background: var(--surface-3-color); border: 1px solid var(--border-color); color: var(--text-color)"
          />
          <span class="text-sm" style="color: var(--text-secondary-color)">B/s (0 = unlimited)</span>
        </div>
      </div>
    </div>
  </section>

  <!-- Retry Behavior -->
  <section>
    <h3 class="text-lg font-bold mb-4">Retry Behavior</h3>
    <div class="rounded-xl p-5 space-y-4" style="background: var(--surface-2-color); border: 1px solid var(--border-color)">
      <div>
        <label class="text-sm font-semibold mb-1.5 block">Max Retries</label>
        <p class="text-xs mb-2" style="color: var(--text-secondary-color)">
          Number of retry attempts before marking a download as failed
        </p>
        <input
          type="number"
          bind:value={config.retry.max_retries}
          min="0"
          max="20"
          class="w-32 rounded-lg px-4 py-2.5 text-sm font-mono outline-none"
          style="background: var(--surface-3-color); border: 1px solid var(--border-color); color: var(--text-color)"
        />
      </div>
      <div>
        <label class="text-sm font-semibold mb-1.5 block">Initial Delay</label>
        <p class="text-xs mb-2" style="color: var(--text-secondary-color)">
          Wait time before the first retry. Doubles with each attempt (exponential backoff).
        </p>
        <div class="flex items-center gap-2">
          <input
            type="number"
            bind:value={config.retry.base_delay_secs}
            min="0.1"
            max="30"
            step="0.5"
            class="w-32 rounded-lg px-4 py-2.5 text-sm font-mono outline-none"
            style="background: var(--surface-3-color); border: 1px solid var(--border-color); color: var(--text-color)"
          />
          <span class="text-sm" style="color: var(--text-secondary-color)">seconds</span>
        </div>
      </div>
      <div>
        <label class="text-sm font-semibold mb-1.5 block">Max Delay</label>
        <p class="text-xs mb-2" style="color: var(--text-secondary-color)">
          Maximum wait time between retries, even after exponential growth
        </p>
        <div class="flex items-center gap-2">
          <input
            type="number"
            bind:value={config.retry.max_delay_secs}
            min="1"
            max="600"
            step="1"
            class="w-32 rounded-lg px-4 py-2.5 text-sm font-mono outline-none"
            style="background: var(--surface-3-color); border: 1px solid var(--border-color); color: var(--text-color)"
          />
          <span class="text-sm" style="color: var(--text-secondary-color)">seconds</span>
        </div>
      </div>
      <button
        onclick={saveConfig}
        class="px-4 py-2 rounded-lg text-sm font-semibold text-white"
        style="background: var(--accent-color)"
      >Save</button>
    </div>
  </section>

  <!-- Webhooks -->
  <section>
    <div class="flex items-center justify-between mb-4">
      <h3 class="text-lg font-bold">Webhooks</h3>
      <button
        onclick={() => (showAddWebhook = !showAddWebhook)}
        class="px-3 py-1.5 rounded-lg text-xs font-semibold text-white"
        style="background: var(--accent-color)"
      >
        + Add Webhook
      </button>
    </div>

    {#if showAddWebhook}
      <div class="rounded-xl p-5 mb-4 space-y-3" style="background: var(--surface-2-color); border: 1px solid var(--border-color)">
        <div>
          <label class="text-xs font-semibold mb-1 block">Name</label>
          <input bind:value={newWebhookName} type="text" placeholder="Discord Notifications"
            class="w-full rounded-lg px-3 py-2 text-sm outline-none"
            style="background: var(--surface-3-color); border: 1px solid var(--border-color); color: var(--text-color)" />
        </div>
        <div>
          <label class="text-xs font-semibold mb-1 block">URL</label>
          <input bind:value={newWebhookUrl} type="url" placeholder="https://discord.com/api/webhooks/..."
            class="w-full rounded-lg px-3 py-2 text-sm font-mono outline-none"
            style="background: var(--surface-3-color); border: 1px solid var(--border-color); color: var(--text-color)" />
        </div>
        <div>
          <label class="text-xs font-semibold mb-1 block">Secret <span class="opacity-50">(optional, for HMAC signing)</span></label>
          <input bind:value={newWebhookSecret} type="text" placeholder="my-secret-key"
            class="w-full rounded-lg px-3 py-2 text-sm font-mono outline-none"
            style="background: var(--surface-3-color); border: 1px solid var(--border-color); color: var(--text-color)" />
        </div>
        <div>
          <label class="text-xs font-semibold mb-1 block">Events <span class="opacity-50">(comma-separated, * = all)</span></label>
          <input bind:value={newWebhookEvents} type="text" placeholder="download.completed, download.failed"
            class="w-full rounded-lg px-3 py-2 text-sm font-mono outline-none"
            style="background: var(--surface-3-color); border: 1px solid var(--border-color); color: var(--text-color)" />
        </div>
        <div class="flex gap-2 pt-1">
          <button onclick={handleAddWebhook}
            class="px-4 py-2 rounded-lg text-sm font-semibold text-white"
            style="background: var(--accent-color)"
            disabled={!newWebhookName.trim() || !newWebhookUrl.trim()}
          >Save</button>
          <button onclick={() => (showAddWebhook = false)}
            class="px-4 py-2 rounded-lg text-sm"
            style="color: var(--text-secondary-color)"
          >Cancel</button>
        </div>
      </div>
    {/if}

    {#if webhooks.length === 0 && !showAddWebhook}
      <div class="rounded-xl p-5 text-center" style="background: var(--surface-2-color); border: 1px solid var(--border-color)">
        <p class="text-sm" style="color: var(--text-secondary-color)">
          No webhooks configured. Add one to receive notifications on Discord, Slack, Home Assistant, etc.
        </p>
      </div>
    {:else}
      <div class="space-y-2">
        {#each webhooks as wh}
          <div class="rounded-xl p-4 flex items-center justify-between gap-3" style="background: var(--surface-2-color); border: 1px solid var(--border-color)">
            <div class="min-w-0 flex-1">
              <p class="font-semibold text-sm truncate">{wh.name}</p>
              <p class="text-xs font-mono truncate" style="color: var(--text-secondary-color)">{wh.url}</p>
              <p class="text-[10px] mt-0.5" style="color: var(--text-secondary-color)">
                Events: {wh.events?.join(", ") || "*"}
                {#if wh.secret}&middot; signed{/if}
              </p>
            </div>
            <div class="flex gap-1.5 shrink-0">
              <button onclick={() => handleTestWebhook(wh.id)}
                class="px-2.5 py-1.5 rounded-lg text-xs border"
                style="border-color: var(--border-color); color: var(--text-secondary-color)"
                title="Send test event"
              >Test</button>
              <button onclick={() => handleDeleteWebhook(wh.id)}
                class="px-2.5 py-1.5 rounded-lg text-xs text-red-400 border border-red-400/30 hover:bg-red-400/10"
                title="Delete webhook"
              >Delete</button>
            </div>
          </div>
        {/each}
      </div>
    {/if}
  </section>

  <!-- About -->
  <section>
    <h3 class="text-lg font-bold mb-4">About</h3>
    <div class="rounded-xl p-5" style="background: var(--surface-2-color); border: 1px solid var(--border-color)">
      <div class="flex items-center gap-4">
        <div class="pixel-logo text-3xl" style="font-family: 'Press Start 2P'; color: var(--accent-color)">a</div>
        <div>
          <p class="font-bold">amigo-downloader</p>
          <p class="text-sm" style="color: var(--text-secondary-color)">v0.1.0 — Fast, modular download manager</p>
          <p class="text-xs mt-1" style="color: var(--text-secondary-color)">
            github.com/amigo-labs/amigo-downloader
          </p>
        </div>
      </div>
    </div>
  </section>
</div>
{:else}
<div class="flex items-center justify-center h-32">
  <p class="text-sm" style="color: var(--text-secondary-color)">Loading settings...</p>
</div>
{/if}
