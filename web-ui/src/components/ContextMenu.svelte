<script lang="ts">
  import { onMount, tick } from "svelte";
  import Icon from "@amigo/ui/components/Icon.svelte";

  let { x, y, items, onclose }:
    { x: number; y: number; items: { label: string; icon: string; action: () => void; color?: string }[]; onclose: () => void } = $props();

  let menuEl: HTMLDivElement | undefined = $state();
  let itemEls: HTMLButtonElement[] = $state([]);
  let focusedIndex: number = $state(0);

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
    // Focus the first item so the menu is keyboard-operable from the
    // moment it opens (audit #38).
    tick().then(() => itemEls[0]?.focus());
  });

  function moveFocus(delta: number) {
    if (!items.length) return;
    focusedIndex = (focusedIndex + delta + items.length) % items.length;
    itemEls[focusedIndex]?.focus();
  }

  function handleKeydown(e: KeyboardEvent) {
    switch (e.key) {
      case "Escape":
        e.preventDefault();
        onclose();
        break;
      case "ArrowDown":
        e.preventDefault();
        moveFocus(1);
        break;
      case "ArrowUp":
        e.preventDefault();
        moveFocus(-1);
        break;
      case "Home":
        e.preventDefault();
        focusedIndex = 0;
        itemEls[0]?.focus();
        break;
      case "End":
        e.preventDefault();
        focusedIndex = items.length - 1;
        itemEls[focusedIndex]?.focus();
        break;
      case "Enter":
      case " ":
        // Let the active item handle Enter via its onclick. We only need
        // to ensure that pressing Enter on an item that has just been
        // moved into focus actually fires it; default browser behaviour
        // does that, so nothing to add here.
        break;
    }
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
  aria-orientation="vertical"
>
  {#each items as item, i}
    <button
      bind:this={itemEls[i]}
      role="menuitem"
      tabindex={focusedIndex === i ? 0 : -1}
      onclick={() => { item.action(); onclose(); }}
      onfocus={() => (focusedIndex = i)}
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
