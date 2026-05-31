<script lang="ts">
  import { locale, tr } from "../lib/i18n";
  import { focusTrap } from "../lib/focusTrap";
  import { scaleFade, dur } from "../lib/motion";
  import { fade } from "svelte/transition";
  import Icon from "@amigo/ui/components/Icon.svelte";

  let { onclose }: { onclose: () => void } = $props();

  let shortcuts = $derived([
    { keys: "⌘ / Ctrl + K", desc: tr($locale, "shortcuts.command_palette") },
    { keys: "Ctrl + N", desc: tr($locale, "shortcuts.add") },
    { keys: "Esc", desc: tr($locale, "shortcuts.close") },
    { keys: "1 – 4", desc: tr($locale, "shortcuts.navigate") },
    { keys: "?", desc: tr($locale, "shortcuts.help") },
  ]);
</script>

<div class="fixed inset-0 z-50 flex items-center justify-center p-4">
  <button class="fixed inset-0 bg-black/60" onclick={onclose} aria-label={tr($locale, "common.close")} transition:fade={{ duration: dur(150) }}></button>

  <div
    use:focusTrap
    role="dialog"
    aria-modal="true"
    aria-label={tr($locale, "shortcuts.title")}
    class="relative z-10 w-full max-w-sm rounded-xl p-5 neon-card"
    style="background: var(--bg-surface)"
    transition:scaleFade={{ duration: dur(180), y: 8 }}
  >
    <div class="flex items-center justify-between mb-4">
      <h2 class="text-sm font-bold" style="color: var(--text-primary)">{tr($locale, "shortcuts.title")}</h2>
      <button
        onclick={onclose}
        class="icon-btn p-1.5 rounded-lg min-w-[44px] min-h-[44px] flex items-center justify-center"
        style="color: var(--text-secondary)"
        aria-label={tr($locale, "common.close")}
      >
        <Icon name="x" size={16} />
      </button>
    </div>

    <div class="space-y-2">
      {#each shortcuts as s}
        <div class="flex items-center justify-between py-1.5">
          <span class="text-sm" style="color: var(--text-primary)">{s.desc}</span>
          <kbd
            class="px-2 py-0.5 rounded text-xs font-semibold"
            style="font-family: var(--font-mono); background: var(--bg-surface-2); border: 1px solid var(--border-color); color: var(--text-secondary)"
          >{s.keys}</kbd>
        </div>
      {/each}
    </div>
  </div>
</div>
