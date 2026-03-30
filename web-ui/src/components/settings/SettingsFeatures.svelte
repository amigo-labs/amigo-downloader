<script lang="ts">
  import type { AppConfig } from "../../lib/api";

  let { config, onsave }: { config: AppConfig; onsave: () => void } = $props();

  function toggle(key: "usenet" | "rss_feeds" | "server_stats") {
    config.features[key] = !config.features[key];
    onsave();
  }
</script>

<section>
  <h3 class="text-lg font-bold mb-4">Features</h3>
  <div class="rounded-xl p-5 space-y-4" style="background: var(--surface-2-color); border: 1px solid var(--border-color)">
    {#each [
      { key: "usenet" as const, label: "Usenet", desc: "Enable Usenet mode (NZB import, NNTP servers, watch folder)" },
      { key: "rss_feeds" as const, label: "RSS Feeds", desc: "Monitor RSS/Atom feeds for automatic NZB import" },
      { key: "server_stats" as const, label: "Server Statistics", desc: "Show per-server connection stats in Usenet UI" },
    ] as opt (opt.key)}
      <div class="flex items-center justify-between">
        <div>
          <p class="text-sm font-semibold">{opt.label}</p>
          <p class="text-xs" style="color: var(--text-secondary-color)">{opt.desc}</p>
        </div>
        <button
          onclick={() => toggle(opt.key)}
          class="w-12 h-6 rounded-full relative transition-colors"
          style="background: {config.features[opt.key] ? 'var(--accent-color)' : 'var(--surface-3-color)'}"
        >
          <span
            class="absolute top-0.5 w-5 h-5 rounded-full bg-white transition-all shadow"
            style="left: {config.features[opt.key] ? '1.625rem' : '0.125rem'}"
          ></span>
        </button>
      </div>
    {/each}
  </div>
</section>
