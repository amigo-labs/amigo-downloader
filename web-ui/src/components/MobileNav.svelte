<script lang="ts">
  import type { Page } from "../lib/stores";
  import { locale, tr } from "../lib/i18n";
  import Icon from "@amigo/ui/components/Icon.svelte";

  let { current, onnavigate, onadd }:
    { current: Page; onnavigate: (page: Page) => void; onadd: () => void } = $props();

  // Two items either side of the central Add FAB.
  const left: { id: Page; key: string; icon: string }[] = [
    { id: "downloads", key: "nav.downloads", icon: "arrow-down" },
    { id: "plugins", key: "nav.plugins", icon: "puzzle" },
  ];
  const right: { id: Page; key: string; icon: string }[] = [
    { id: "history", key: "nav.history", icon: "clock" },
    { id: "settings", key: "nav.settings", icon: "gear" },
  ];
</script>

<nav
  aria-label={tr($locale, "nav.management")}
  class="mobile-nav md:hidden fixed bottom-0 inset-x-0 z-40 flex items-stretch justify-around"
  style="background: color-mix(in srgb, var(--bg-surface) 92%, transparent); border-top: 1px solid var(--border-color); backdrop-filter: blur(12px); padding-bottom: env(safe-area-inset-bottom, 0px)"
>
  {#each left as item}
    <button
      onclick={() => onnavigate(item.id)}
      aria-current={current === item.id ? "page" : undefined}
      class="mobile-tab"
      class:active={current === item.id}
    >
      <Icon name={item.icon} size={20} />
      <span class="text-[10px] font-medium">{tr($locale, item.key)}</span>
    </button>
  {/each}

  <!-- Center Add FAB -->
  <div class="relative flex items-end justify-center" style="width: 64px">
    <button
      onclick={onadd}
      aria-label={tr($locale, "cmd.add_download")}
      class="fab absolute -top-5 w-14 h-14 rounded-full flex items-center justify-center"
      style="background: var(--neon-primary); color: var(--bg-deep); box-shadow: var(--neon-glow-md), 0 6px 16px rgb(0 0 0 / 30%)"
    >
      <Icon name="plus" size={24} />
    </button>
  </div>

  {#each right as item}
    <button
      onclick={() => onnavigate(item.id)}
      aria-current={current === item.id ? "page" : undefined}
      class="mobile-tab"
      class:active={current === item.id}
    >
      <Icon name={item.icon} size={20} />
      <span class="text-[10px] font-medium">{tr($locale, item.key)}</span>
    </button>
  {/each}
</nav>

<style>
  .mobile-tab {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 2px;
    min-height: 56px;
    padding: 6px 0;
    color: var(--text-secondary);
    transition: color var(--dur-fast, 0.15s) var(--ease-out, ease);
  }

  .mobile-tab.active {
    color: var(--neon-primary);
  }

  .mobile-tab.active :global(svg) {
    filter: drop-shadow(0 0 var(--neon-drop-blur, 3px) currentcolor);
  }

  .fab {
    transition: transform var(--dur-fast, 0.15s) var(--ease-spring, ease);
  }

  .fab:active {
    transform: scale(0.92);
  }
</style>
