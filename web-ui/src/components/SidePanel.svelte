<script lang="ts">
  import { fly } from "svelte/transition";
  import { sidePanelMode, selectedDownload, closeSidePanel } from "../lib/stores";
  import { isDesktop } from "../lib/breakpoints";
  import { openLayer, layerState } from "../lib/overlays.svelte";
  import { locale, tr } from "../lib/i18n";
  import { dur } from "../lib/motion";
  import DetailPanel from "./DetailPanel.svelte";
  import AddPanel from "./AddPanel.svelte";
  import Dialog from "@amigo/ui/components/Dialog.svelte";
  import IconButton from "@amigo/ui/components/IconButton.svelte";

  let isOpen = $derived($sidePanelMode !== null);
  let title = $derived(
    $sidePanelMode === "add"
      ? tr($locale, "add.title")
      : $selectedDownload?.filename || $selectedDownload?.url || tr($locale, "panel.details"),
  );

  // Only the overlay variant joins the Escape stack; the docked desktop panel
  // is part of the page, not a layer over it.
  const layer = layerState("side-panel");
  $effect(() => {
    if (!isOpen || $isDesktop) return;
    return openLayer({
      id: "side-panel",
      kind: "panel",
      label: title,
      onDismiss: closeSidePanel,
    });
  });
</script>

<!--
  Exactly one variant is mounted. Rendering both and hiding one with CSS
  duplicated every id inside AddPanel/DetailPanel, ran their onMount twice,
  and left a focus trap running on a display:none node.
-->
{#snippet body()}
  {#if $sidePanelMode === "detail"}
    <DetailPanel />
  {:else if $sidePanelMode === "add"}
    <AddPanel />
  {/if}
{/snippet}

{#if isOpen}
  {#if $isDesktop}
    <aside
      aria-label={$sidePanelMode === "add" ? tr($locale, "add.title") : tr($locale, "panel.details")}
      class="flex flex-col w-80 shrink-0 border-l overflow-y-auto"
      style="background: var(--bg-surface); border-color: var(--border-color)"
      transition:fly={{ x: 320, duration: dur(200) }}
    >
      <div
        class="flex items-center justify-between gap-2 px-4 py-3 border-b"
        style="border-color: var(--border-color)"
      >
        <h3 class="font-semibold text-sm truncate flex-1" style="color: var(--text-primary)">
          {title}
        </h3>
        <IconButton icon="x" label={tr($locale, "common.close")} onclick={closeSidePanel} />
      </div>
      {@render body()}
    </aside>
  {:else}
    <Dialog
      {title}
      align="right"
      layer="panel"
      isTop={$layer.isTop}
      closeLabel={tr($locale, "common.close")}
      onclose={closeSidePanel}
    >
      {@render body()}
    </Dialog>
  {/if}
{/if}
