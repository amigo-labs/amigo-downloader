<script lang="ts">
  import Icon from "@amigo/ui/components/Icon.svelte";

  let { onclose }: { onclose: () => void } = $props();

  const shortcuts = [
    { keys: "Ctrl + N", desc: "Add download" },
    { keys: "Ctrl + L", desc: "Add download" },
    { keys: "Esc", desc: "Close panel / dialog" },
    { keys: "1 – 4", desc: "Navigate pages" },
    { keys: "?", desc: "Show this help" },
  ];
</script>

<div class="fixed inset-0 z-50 flex items-center justify-center p-4">
  <button class="fixed inset-0 bg-black/60" onclick={onclose} aria-label="Close shortcuts"></button>

  <div
    role="dialog"
    aria-modal="true"
    aria-label="Keyboard shortcuts"
    class="relative z-10 w-full max-w-sm rounded-xl p-5"
    style="background: var(--bg-surface); border: 1px solid var(--border-color)"
  >
    <div class="flex items-center justify-between mb-4">
      <h2 class="text-sm font-bold" style="color: var(--text-primary)">Keyboard Shortcuts</h2>
      <button
        onclick={onclose}
        class="icon-btn p-1.5 rounded-lg min-w-[44px] min-h-[44px] flex items-center justify-center"
        style="color: var(--text-secondary)"
        aria-label="Close"
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
