<script lang="ts">
  import { fly } from "svelte/transition";
  import { sidePanelMode, selectedDownload, closeSidePanel } from "../lib/stores";
  import DetailPanel from "./DetailPanel.svelte";
  import AddPanel from "./AddPanel.svelte";
  import Icon from "./Icon.svelte";

  let isOpen = $derived($sidePanelMode !== null);
  let title = $derived(
    $sidePanelMode === "add"
      ? "Add Download"
      : $selectedDownload?.filename || "Download Details"
  );
</script>

{#if isOpen}
  <!-- Desktop panel (lg+) -->
  <div
    role="complementary"
    aria-label={$sidePanelMode === "add" ? "Add download" : "Download details"}
    class="hidden lg:flex flex-col w-80 shrink-0 border-l overflow-y-auto"
    style="background: var(--bg-surface); border-color: var(--border-color)"
    transition:fly={{ x: 320, duration: 200 }}
  >
    <!-- Header -->
    <div class="flex items-center justify-between px-4 py-3 border-b" style="border-color: var(--border-color)">
      <h3 class="font-semibold text-sm truncate flex-1" style="color: var(--text-primary)">{title}</h3>
      <button
        onclick={closeSidePanel}
        class="icon-btn p-1.5 rounded-lg min-w-[44px] min-h-[44px] flex items-center justify-center"
        style="color: var(--text-secondary)"
        aria-label="Close panel"
      >
        <Icon name="x" size={16} />
      </button>
    </div>

    <!-- Content -->
    {#if $sidePanelMode === "detail"}
      <DetailPanel />
    {:else if $sidePanelMode === "add"}
      <AddPanel />
    {/if}
  </div>

  <!-- Mobile overlay (<lg) -->
  <div class="fixed inset-0 z-50 lg:hidden">
    <button
      class="absolute inset-0 bg-black/60"
      onclick={closeSidePanel}
      aria-label="Close panel"
    ></button>
    <div
      role="complementary"
      aria-label={$sidePanelMode === "add" ? "Add download" : "Download details"}
      class="absolute right-0 top-0 bottom-0 w-full max-w-sm flex flex-col overflow-y-auto"
      style="background: var(--bg-surface)"
      transition:fly={{ x: 384, duration: 250 }}
    >
      <!-- Header -->
      <div class="flex items-center justify-between px-4 py-3 border-b" style="border-color: var(--border-color)">
        <h3 class="font-semibold text-sm truncate flex-1" style="color: var(--text-primary)">{title}</h3>
        <button
          onclick={closeSidePanel}
          class="icon-btn p-2 rounded-lg min-w-[44px] min-h-[44px] flex items-center justify-center"
          style="color: var(--text-secondary)"
          aria-label="Close panel"
        >
          <Icon name="x" size={18} />
        </button>
      </div>

      <!-- Content -->
      {#if $sidePanelMode === "detail"}
        <DetailPanel />
      {:else if $sidePanelMode === "add"}
        <AddPanel />
      {/if}
    </div>
  </div>
{/if}
