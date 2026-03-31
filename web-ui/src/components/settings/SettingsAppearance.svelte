<script lang="ts">
  import { theme, palette, neonIntensity, getNeonLabel, type ColorPalette, type ThemeMode } from "../../lib/stores";
  import Icon from "../Icon.svelte";

  const paletteOptions: { id: ColorPalette; label: string; color: string }[] = [
    { id: "blue", label: "Blue", color: "#3b82f6" },
    { id: "teal", label: "Teal", color: "#14b8a6" },
    { id: "indigo", label: "Indigo", color: "#6366f1" },
    { id: "amber", label: "Amber", color: "#f59e0b" },
    { id: "violet", label: "Violet", color: "#8b5cf6" },
    { id: "rose", label: "Rose", color: "#f43f5e" },
  ];

  let label = $derived(getNeonLabel($neonIntensity));
</script>

<section>
  <h3 class="text-lg font-bold mb-4" style="color: var(--text-primary)">Appearance</h3>

  <!-- Theme mode -->
  <div class="neon-card rounded-xl p-5 mb-4" style="background: var(--bg-surface)">
    <label class="text-sm font-semibold mb-3 block" style="color: var(--text-primary)">Theme</label>
    <div class="flex gap-3">
      {#each [{ id: "dark", label: "Dark" }, { id: "light", label: "Light" }] as mode}
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

  <!-- Color palette -->
  <div class="neon-card rounded-xl p-5 mb-4" style="background: var(--bg-surface)">
    <label class="text-sm font-semibold mb-3 block" style="color: var(--text-primary)">Color Palette</label>
    <div class="flex gap-3 flex-wrap">
      {#each paletteOptions as opt}
        <button
          onclick={() => palette.set(opt.id)}
          class="flex flex-col items-center gap-1.5 p-2 rounded-lg transition-colors"
          style={$palette === opt.id
            ? "background: color-mix(in srgb, var(--neon-primary) 10%, transparent)"
            : ""}
        >
          <div
            class="color-swatch"
            class:active={$palette === opt.id}
            style="--swatch-color: {opt.color}; background: {opt.color}"
          ></div>
          <span class="text-[10px] font-medium" style="color: {$palette === opt.id ? opt.color : 'var(--text-secondary)'}">{opt.label}</span>
        </button>
      {/each}
    </div>
  </div>

  <!-- Neon intensity -->
  <div class="neon-card rounded-xl p-5" style="background: var(--bg-surface)">
    <label class="text-sm font-semibold mb-3 block" style="color: var(--text-primary)">Neon Intensity</label>
    <div class="neon-slider">
      <Icon name="bolt" size={16} />
      <input
        type="range"
        min="0"
        max="1"
        step="0.25"
        value={$neonIntensity}
        oninput={(e) => neonIntensity.set(parseFloat((e.target as HTMLInputElement).value))}
        aria-label="Neon intensity"
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
