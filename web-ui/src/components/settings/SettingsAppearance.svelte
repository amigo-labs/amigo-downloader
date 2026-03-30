<script lang="ts">
  import { theme, accent, type AccentPreset, type ThemeMode } from "../../lib/stores";

  const accentPresets: { id: AccentPreset; label: string; hex: string }[] = [
    { id: "electric", label: "Electric", hex: "#00D4FF" },
    { id: "hot", label: "Hot", hex: "#FF2D78" },
    { id: "cyan", label: "Cyan", hex: "#00FFD0" },
  ];
</script>

<section>
  <h3 class="text-lg font-bold mb-4" style="color: var(--text-primary)">Appearance</h3>

  <!-- Theme mode -->
  <div class="rounded-xl p-5 mb-4" style="background: var(--bg-surface); border: 1px solid var(--border-color)">
    <label class="text-sm font-semibold mb-3 block" style="color: var(--text-primary)">Theme</label>
    <div class="flex gap-3">
      {#each [{ id: "dark", label: "Dark" }, { id: "lights-on", label: "Lights On" }] as mode}
        <button
          onclick={() => theme.set(mode.id as ThemeMode)}
          class="flex-1 py-3 rounded-xl text-sm font-medium transition-colors"
          style={$theme === mode.id
            ? "background: color-mix(in srgb, var(--neon-primary) 15%, transparent); color: var(--neon-primary); border: 1px solid color-mix(in srgb, var(--neon-primary) 20%, transparent)"
            : "background: var(--bg-surface-2); color: var(--text-secondary); border: 1px solid transparent"}
        >
          {mode.label}
        </button>
      {/each}
    </div>
  </div>

  <!-- Accent preset -->
  <div class="rounded-xl p-5" style="background: var(--bg-surface); border: 1px solid var(--border-color)">
    <label class="text-sm font-semibold mb-3 block" style="color: var(--text-primary)">Accent Color</label>
    <div class="flex gap-3">
      {#each accentPresets as preset}
        <button
          onclick={() => accent.set(preset.id)}
          class="flex-1 py-3 rounded-xl text-sm font-medium transition-colors flex items-center justify-center gap-2"
          style={$accent === preset.id
            ? `background: color-mix(in srgb, ${preset.hex} 12%, transparent); color: ${preset.hex}; border: 1px solid color-mix(in srgb, ${preset.hex} 20%, transparent)`
            : "background: var(--bg-surface-2); color: var(--text-secondary); border: 1px solid transparent"}
        >
          <span class="w-3 h-3 rounded-full" style="background: {preset.hex}"></span>
          {preset.label}
        </button>
      {/each}
    </div>
  </div>
</section>
