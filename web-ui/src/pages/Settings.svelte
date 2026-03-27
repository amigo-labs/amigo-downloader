<script lang="ts">
  import { theme, layout, accent, type AccentColor, type LayoutMode, type ThemeMode } from "../lib/stores";

  const accentColors: { id: AccentColor; label: string; hex: string }[] = [
    { id: "blue", label: "Blue", hex: "#3b82f6" },
    { id: "green", label: "Green", hex: "#10b981" },
    { id: "purple", label: "Purple", hex: "#8b5cf6" },
    { id: "coral", label: "Coral", hex: "#f43f5e" },
    { id: "orange", label: "Orange", hex: "#f97316" },
    { id: "cyan", label: "Cyan", hex: "#06b6d4" },
  ];
</script>

<div class="max-w-2xl space-y-8">
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
          value="downloads"
          readonly
          class="w-full rounded-lg px-4 py-2.5 text-sm font-mono outline-none"
          style="background: var(--surface-3-color); border: 1px solid var(--border-color); color: var(--text-color)"
        />
      </div>
      <div>
        <label class="text-sm font-semibold mb-1.5 block">Max Concurrent Downloads</label>
        <input
          type="number"
          value="10"
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
            value="0"
            min="0"
            class="w-32 rounded-lg px-4 py-2.5 text-sm font-mono outline-none"
            style="background: var(--surface-3-color); border: 1px solid var(--border-color); color: var(--text-color)"
          />
          <span class="text-sm" style="color: var(--text-secondary-color)">KB/s (0 = unlimited)</span>
        </div>
      </div>
    </div>
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
