<script lang="ts">
  import { onMount, tick } from "svelte";
  import { locale, tr } from "../lib/i18n";
  import { openLayer, layerState } from "../lib/overlays.svelte";
  import Icon from "@amigo/ui/components/Icon.svelte";

  let { x, y, items, onclose }: {
    x: number;
    y: number;
    items: { label: string; icon: string; action: () => void; tone?: "default" | "danger" }[];
    onclose: () => void;
  } = $props();

  let menuEl: HTMLDivElement | undefined = $state();
  let itemEls: HTMLButtonElement[] = $state([]);
  let focusedIndex: number = $state(0);
  let pos = $state({ x: 0, y: 0 });
  let measured = $state(false);

  // Escape now goes through the shared layer stack, so the menu can no longer
  // be dismissed out of order relative to a dialog above it.
  const layer = layerState("context-menu");
  $effect(() =>
    openLayer({ id: "context-menu", kind: "panel", label: "Context menu", onDismiss: onclose }),
  );

  // Focus is captured on open and returned on close. Previously it was simply
  // dropped: after choosing an item or pressing Escape, focus fell to <body>
  // and keyboard users had to tab from the top of the page again.
  let previouslyFocused: HTMLElement | null = null;

  function clamp() {
    if (!menuEl) return;
    const rect = menuEl.getBoundingClientRect();
    const margin = 8;
    pos = {
      x: Math.max(margin, Math.min(x, window.innerWidth - rect.width - margin)),
      y: Math.max(margin, Math.min(y, window.innerHeight - rect.height - margin)),
    };
    measured = true;
  }

  onMount(() => {
    previouslyFocused = document.activeElement as HTMLElement | null;
    clamp();
    tick().then(() => itemEls[0]?.focus());

    // Re-clamp on resize/orientation change; the old code mutated
    // element.style once on mount, which the reactive left/top then undid.
    window.addEventListener("resize", clamp);
    return () => {
      window.removeEventListener("resize", clamp);
      previouslyFocused?.focus?.();
    };
  });

  function moveFocus(delta: number) {
    if (!items.length) return;
    focusedIndex = (focusedIndex + delta + items.length) % items.length;
    itemEls[focusedIndex]?.focus();
  }

  // Bound to the menu, not to window: arrow keys used to be captured globally,
  // so tabbing away from an open menu still let them yank focus back into it.
  function onkeydown(e: KeyboardEvent) {
    switch (e.key) {
      case "ArrowDown": e.preventDefault(); moveFocus(1); break;
      case "ArrowUp": e.preventDefault(); moveFocus(-1); break;
      case "Home": e.preventDefault(); focusedIndex = 0; itemEls[0]?.focus(); break;
      case "End": e.preventDefault(); focusedIndex = items.length - 1; itemEls[focusedIndex]?.focus(); break;
      case "Tab":
        // A menu is a single focus scope; Tab closes it rather than escaping
        // into the page behind an open menu.
        e.preventDefault();
        onclose();
        break;
    }
  }
</script>

<!-- Pointer-only dismissal surface. Not a <button>: a full-screen button put
     an extra stop in the tab ring of every overlay that used this pattern. -->
<div
  class="fixed inset-0"
  style="z-index: var(--z-menu)"
  onclick={onclose}
  oncontextmenu={(e) => { e.preventDefault(); onclose(); }}
  aria-hidden="true"
></div>

<div
  bind:this={menuEl}
  class="fixed py-1 rounded-lg min-w-[180px] max-w-[min(20rem,calc(100vw-1rem))]"
  style="left: {pos.x}px; top: {pos.y}px; visibility: {measured ? 'visible' : 'hidden'};
         z-index: var(--z-menu-content);
         background: var(--bg-surface); border: 1px solid var(--border-color);
         box-shadow: var(--elev-3)"
  role="menu"
  tabindex="-1"
  aria-orientation="vertical"
  aria-label={tr($locale, "menu.actions")}
  {onkeydown}
>
  {#each items as item, i}
    <button
      bind:this={itemEls[i]}
      role="menuitem"
      tabindex={focusedIndex === i ? 0 : -1}
      onclick={() => { item.action(); onclose(); }}
      onfocus={() => (focusedIndex = i)}
      class="menu-item"
      class:danger={item.tone === "danger"}
    >
      <Icon name={item.icon} size={14} />
      {item.label}
    </button>
  {/each}
</div>

<style>
  .menu-item {
    display: flex;
    align-items: center;
    gap: 0.625rem;
    width: 100%;
    min-height: 40px;
    padding: 0 0.75rem;
    border: none;
    background: transparent;
    color: var(--text-primary);
    font-size: var(--font-sm);
    font-family: inherit;
    text-align: left;
    cursor: pointer;
    transition: background-color var(--dur-fast) var(--ease-out);
  }

  .menu-item.danger {
    color: var(--danger-ink);
  }

  .menu-item:hover,
  .menu-item:focus-visible {
    background: var(--hover-bg);
  }
</style>
