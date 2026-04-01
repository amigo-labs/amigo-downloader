<script lang="ts">
  import { onMount } from "svelte";
  import { features } from "../lib/stores";
  import { getConfig, putConfig, getWebhooks, type AppConfig } from "../lib/api";
  import { addToast } from "../lib/toast";
  import { locale, type Locale } from "../lib/i18n";
  import Icon from "../components/Icon.svelte";

  import SettingsFeatures from "../components/settings/SettingsFeatures.svelte";
  import SettingsUsenet from "../components/settings/SettingsUsenet.svelte";
  import SettingsUsenetServers from "../components/settings/SettingsUsenetServers.svelte";
  import SettingsAppearance from "../components/settings/SettingsAppearance.svelte";
  import SettingsDownloads from "../components/settings/SettingsDownloads.svelte";
  import SettingsWebhooks from "../components/settings/SettingsWebhooks.svelte";
  import SettingsRssFeeds from "../components/settings/SettingsRssFeeds.svelte";
  import SkeletonCard from "../components/SkeletonCard.svelte";

  let config = $state<AppConfig | null>(null);
  let webhooks = $state<any[]>([]);

  onMount(async () => {
    try {
      const [cfg, wh] = await Promise.all([getConfig(), getWebhooks()]);
      config = cfg;
      webhooks = wh;
      if (cfg.features) features.set(cfg.features);
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
</script>

{#if config}
<div class="max-w-2xl space-y-8">
  <SettingsFeatures {config} onsave={saveConfig} />
  {#if config.features.usenet}
    <SettingsUsenetServers />
    <SettingsUsenet {config} onsave={saveConfig} />
    {#if config.features.rss_feeds}
      <SettingsRssFeeds />
    {/if}
  {/if}
  <SettingsAppearance />
  <SettingsDownloads {config} onsave={saveConfig} />
  <SettingsWebhooks bind:webhooks />

  <!-- Language -->
  <section>
    <h3 class="text-lg font-bold mb-4" style="color: var(--text-primary)">Language</h3>
    <div class="rounded-xl p-5" style="background: var(--bg-surface); border: 1px solid var(--border-color)">
      <div class="flex items-center gap-3">
        <Icon name="globe" size={18} />
        <select
          value={$locale}
          onchange={(e) => locale.set((e.target as HTMLSelectElement).value as Locale)}
          class="flex-1 rounded-lg px-3 py-2 text-sm"
          style="background: var(--bg-surface-2); border: 1px solid var(--border-color); color: var(--text-primary)"
          aria-label="Language"
        >
          <option value="en" style="background: var(--bg-surface-2)">English</option>
          <option value="de" style="background: var(--bg-surface-2)">Deutsch</option>
        </select>
      </div>
    </div>
  </section>

  <!-- About -->
  <section>
    <h3 class="text-lg font-bold mb-4" style="color: var(--text-primary)">About</h3>
    <div class="rounded-xl p-5" style="background: var(--bg-surface); border: 1px solid var(--border-color)">
      <div class="flex items-center gap-4">
        <img src="/amigo-logo.png" alt="amigo-downloader" width="40" height="40" class="rounded-lg" />
        <div>
          <p class="font-bold" style="color: var(--text-primary)">amigo-downloader</p>
          <p class="text-sm" style="color: var(--text-secondary)">v0.1.0 — Fast, modular download manager</p>
          <p class="text-xs mt-1" style="font-family: var(--font-mono);color: var(--text-secondary)">
            github.com/amigo-labs/amigo-downloader
          </p>
        </div>
      </div>
    </div>
  </section>
</div>
{:else}
<SkeletonCard count={3} />
{/if}
