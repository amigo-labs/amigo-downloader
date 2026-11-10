<script lang="ts">
  import { theme, neonIntensity, getNeonLabel, type ThemeMode } from "../../lib/stores";
  import { locale, tr } from "../../lib/i18n";
  import Icon from "@amigo-labs/krake-ui/components/Icon.svelte";

  const themeModes: { id: ThemeMode; key: string }[] = [
    { id: "dark", key: "theme.dark" },
    { id: "light", key: "theme.light" },
  ];

  let label = $derived(getNeonLabel($neonIntensity));
</script>

<section>
  <h3 class="text-lg font-bold mb-4" style="color: var(--text-primary)">{tr($locale, "settings.appearance")}</h3>

  <!-- Theme mode -->
  <div class="neon-card rounded-xl p-5 mb-4" style="background: var(--bg-surface)">
    <span class="text-sm font-semibold mb-3 block" style="color: var(--text-primary)">{tr($locale, "settings.theme")}</span>
    <div class="flex gap-3">
      {#each themeModes as mode}
        <button
          onclick={() => theme.set(mode.id)}
          aria-pressed={$theme === mode.id}
          class="flex-1 py-3 rounded-xl text-sm font-medium transition-colors"
          style={$theme === mode.id
            ? "background: color-mix(in srgb, var(--neon-primary) 15%, transparent); color: var(--accent-ink); border: 1px solid color-mix(in srgb, var(--neon-primary) 20%, transparent)"
            : "background: var(--bg-surface-2); color: var(--text-secondary); border: 1px solid transparent"}
        >
          {tr($locale, mode.key)}
        </button>
      {/each}
    </div>
  </div>

  <!-- Neon intensity -->
  <div class="neon-card rounded-xl p-5" style="background: var(--bg-surface)">
    <span class="text-sm font-semibold mb-3 block" style="color: var(--text-primary)">{tr($locale, "settings.neon_intensity")}</span>
    <div class="neon-slider">
      <Icon name="bolt" size={16} />
      <input
        type="range"
        min="0"
        max="1"
        step="0.25"
        value={$neonIntensity}
        oninput={(e) => neonIntensity.set(parseFloat((e.target as HTMLInputElement).value))}
        aria-label={tr($locale, "settings.neon_intensity")}
      />
      <span class="neon-slider-label">{label}</span>
    </div>
    <div class="flex justify-between mt-2 px-1">
      {#each ["Off", "Low", "Mid", "High", "Full"] as l}
        <span class="text-[9px]" style="color: var(--text-secondary)">{l}</span>
      {/each}
    </div>
  </div>
</section>
