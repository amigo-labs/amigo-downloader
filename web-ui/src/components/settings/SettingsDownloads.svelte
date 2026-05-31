<script lang="ts">
  import type { AppConfig } from "../../lib/api";
  import { locale, tr } from "../../lib/i18n";

  let { config, onsave }: { config: AppConfig; onsave: () => void } = $props();
  const inputStyle =
    "font-family: var(--font-mono);background: var(--bg-surface-2); border: 1px solid var(--border-color); color: var(--text-primary)";
</script>

<section>
  <h3 class="text-lg font-bold mb-4" style="color: var(--text-primary)">{tr($locale, "settings.downloads")}</h3>
  <div class="rounded-xl p-5 space-y-4" style="background: var(--bg-surface); border: 1px solid var(--border-color)">
    <label class="block">
      <span class="text-sm font-semibold mb-1.5 block" style="color: var(--text-primary)">{tr($locale, "settings.download_dir")}</span>
      <input
        type="text"
        bind:value={config.download_dir}
        class="w-full rounded-lg px-4 py-2.5 text-sm"
        style={inputStyle}
      />
    </label>
    <label class="block">
      <span class="text-sm font-semibold mb-1.5 block" style="color: var(--text-primary)">{tr($locale, "settings.max_concurrent")}</span>
      <input
        type="number"
        bind:value={config.max_concurrent_downloads}
        min="1"
        max="50"
        class="w-32 rounded-lg px-4 py-2.5 text-sm"
        style={inputStyle}
      />
    </label>
    <div>
      <span class="text-sm font-semibold mb-1.5 block" style="color: var(--text-primary)">{tr($locale, "settings.speed_limit")}</span>
      <div class="flex items-center gap-2">
        <input
          type="number"
          bind:value={config.bandwidth.global_limit}
          min="0"
          class="w-32 rounded-lg px-4 py-2.5 text-sm"
          style={inputStyle}
          aria-label={tr($locale, "settings.speed_limit")}
        />
        <span class="text-sm" style="color: var(--text-secondary)">{tr($locale, "settings.speed_limit_hint")}</span>
      </div>
    </div>
    <div>
      <h4 class="text-sm font-semibold mb-3" style="color: var(--text-primary)">{tr($locale, "settings.retry_behavior")}</h4>
      <div class="space-y-3">
        <label class="block">
          <span class="text-xs mb-1 block" style="color: var(--text-secondary)">{tr($locale, "settings.max_retries")}</span>
          <input type="number" bind:value={config.retry.max_retries} min="0" max="20"
            class="w-32 rounded-lg px-4 py-2.5 text-sm" style={inputStyle} />
        </label>
        <div class="flex gap-4">
          <label class="block">
            <span class="text-xs mb-1 block" style="color: var(--text-secondary)">{tr($locale, "settings.initial_delay")}</span>
            <input type="number" bind:value={config.retry.base_delay_secs} min="0.1" max="30" step="0.5"
              class="w-28 rounded-lg px-4 py-2.5 text-sm" style={inputStyle} />
          </label>
          <label class="block">
            <span class="text-xs mb-1 block" style="color: var(--text-secondary)">{tr($locale, "settings.max_delay")}</span>
            <input type="number" bind:value={config.retry.max_delay_secs} min="1" max="600" step="1"
              class="w-28 rounded-lg px-4 py-2.5 text-sm" style={inputStyle} />
          </label>
        </div>
      </div>
    </div>
    <button
      onclick={onsave}
      class="action-btn px-4 py-2 rounded-lg text-sm font-semibold"
      style="background: var(--neon-primary); color: var(--bg-deep)"
    >{tr($locale, "common.save")}</button>
  </div>
</section>
