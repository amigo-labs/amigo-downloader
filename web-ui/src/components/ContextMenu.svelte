<script lang="ts">
  import { onMount } from "svelte";
  import Icon from "@amigo/ui/components/Icon.svelte";

  let { x, y, items, onclose }:
    { x: number; y: number; items: { label: string; icon: string; action: () => void; color?: string }[]; onclose: () => void } = $props();

  let menuEl: HTMLDivElement | undefined = $state();

  onMount(() => {
    // Adjust position if menu overflows viewport
    if (menuEl) {
      const rect = menuEl.getBoundingClientRect();
      if (rect.right > window.innerWidth) {
        menuEl.style.left = `${window.innerWidth - rect.width - 8}px`;
      }
      if (rect.bottom > window.innerHeight) {
        menuEl.style.top = `${window.innerHeight - rect.height - 8}px`;
      }
    }
  });

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") onclose();
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<!-- Backdrop -->
<button class="fixed inset-0 z-[60]" onclick={onclose} aria-label="Close menu"></button>

<!-- Menu -->
<div
  bind:this={menuEl}
  class="fixed z-[61] py-1 rounded-lg shadow-xl min-w-[160px]"
  style="left: {x}px; top: {y}px; background: var(--bg-surface); border: 1px solid var(--border-color)"
  role="menu"
>
  {#each items as item}
    <button
      role="menuitem"
      onclick={() => { item.action(); onclose(); }}
      class="w-full flex items-center gap-2.5 px-3 py-2 text-sm transition-colors text-left"
      style="color: {item.color || 'var(--text-primary)'}"
      onmouseenter={(e) => (e.currentTarget as HTMLElement).style.background = 'var(--hover-bg)'}
      onmouseleave={(e) => (e.currentTarget as HTMLElement).style.background = 'transparent'}
    >
      <Icon name={item.icon} size={14} />
      {item.label}
    </button>
  {/each}
</div>
