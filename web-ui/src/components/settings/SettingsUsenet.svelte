<script lang="ts">
  import type { AppConfig } from "../../lib/api";
  import { locale, tr } from "../../lib/i18n";
  import Card from "@amigo/ui/components/Card.svelte";
  import Toggle from "@amigo/ui/components/Toggle.svelte";

  let { config, onsave }: { config: AppConfig; onsave: () => void } = $props();

  const KEYS = [
    "par2_repair",
    "selective_par2",
    "auto_unrar",
    "sequential_postprocess",
    "delete_archives_after_extract",
    "delete_par2_after_repair",
  ] as const;

  type UsenetFlags = Record<(typeof KEYS)[number], boolean>;
  let flags = $derived(config.usenet as unknown as UsenetFlags);

  function toggle(key: (typeof KEYS)[number]) {
    flags[key] = !flags[key];
    onsave();
  }
</script>

<section>
  <h3 class="text-lg font-bold mb-4" style="color: var(--text-primary)">
    {tr($locale, "postprocess.title")}
  </h3>
  <Card padding="lg">
    <div class="grid gap-4">
      {#each KEYS as key (key)}
        <Toggle
          checked={flags[key]}
          label={tr($locale, `postprocess.${key}`)}
          description={tr($locale, `postprocess.${key}_desc`)}
          onchange={() => toggle(key)}
        />
      {/each}
    </div>
  </Card>
</section>
