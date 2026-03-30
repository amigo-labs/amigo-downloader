<script lang="ts">
  import type { AppConfig } from "../../lib/api";

  let { config, onsave }: { config: AppConfig; onsave: () => void } = $props();
</script>

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
    <div>
      <h4 class="text-sm font-semibold mb-3">Retry Behavior</h4>
      <div class="space-y-3">
        <div>
          <label class="text-xs mb-1 block" style="color: var(--text-secondary-color)">Max retries before giving up</label>
          <input type="number" bind:value={config.retry.max_retries} min="0" max="20"
            class="w-32 rounded-lg px-4 py-2.5 text-sm font-mono outline-none"
            style="background: var(--surface-3-color); border: 1px solid var(--border-color); color: var(--text-color)" />
        </div>
        <div class="flex gap-4">
          <div>
            <label class="text-xs mb-1 block" style="color: var(--text-secondary-color)">Initial delay (s)</label>
            <input type="number" bind:value={config.retry.base_delay_secs} min="0.1" max="30" step="0.5"
              class="w-28 rounded-lg px-4 py-2.5 text-sm font-mono outline-none"
              style="background: var(--surface-3-color); border: 1px solid var(--border-color); color: var(--text-color)" />
          </div>
          <div>
            <label class="text-xs mb-1 block" style="color: var(--text-secondary-color)">Max delay (s)</label>
            <input type="number" bind:value={config.retry.max_delay_secs} min="1" max="600" step="1"
              class="w-28 rounded-lg px-4 py-2.5 text-sm font-mono outline-none"
              style="background: var(--surface-3-color); border: 1px solid var(--border-color); color: var(--text-color)" />
          </div>
        </div>
      </div>
    </div>
    <button
      onclick={onsave}
      class="px-4 py-2 rounded-lg text-sm font-semibold text-white"
      style="background: var(--accent-color)"
    >Save</button>
  </div>
</section>
