<script lang="ts">
  import type { AppConfig } from "../../lib/api";
  import { locale, tr } from "../../lib/i18n";
  import Card from "@amigo/ui/components/Card.svelte";
  import Toggle from "@amigo/ui/components/Toggle.svelte";

  // Was one of two settings sections with no i18n at all: switching the app
  // to German left these strings in English.
  let { config, onsave }: { config: AppConfig; onsave: () => void } = $props();

  const KEYS = ["usenet", "rss_feeds", "server_stats"] as const;

  function toggle(key: (typeof KEYS)[number]) {
    config.features[key] = !config.features[key];
    onsave();
  }
</script>

<section>
  <h3 class="text-lg font-bold mb-4" style="color: var(--text-primary)">
    {tr($locale, "features.title")}
  </h3>
  <Card padding="lg">
    <div class="grid gap-4">
      {#each KEYS as key (key)}
        <Toggle
          checked={config.features[key]}
          label={tr($locale, `features.${key}`)}
          description={tr($locale, `features.${key}_desc`)}
          onchange={() => toggle(key)}
        />
      {/each}
    </div>
  </Card>
</section>
