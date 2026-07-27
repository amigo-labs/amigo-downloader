<script lang="ts">
  import { flip } from "svelte/animate";
  import { fly } from "svelte/transition";
  import { toasts, removeToast, pauseToast, resumeToast, type Toast } from "../lib/toast";
  import { dur, flipConfig } from "../lib/motion";
  import { locale, tr } from "../lib/i18n";
  import Icon from "@amigo/ui/components/Icon.svelte";

  // -ink variants: these carry text and glyphs, so they must clear AA.
  function inkFor(type: Toast["type"]) {
    switch (type) {
      case "success": return "var(--success-ink)";
      case "error": return "var(--danger-ink)";
      default: return "var(--accent-ink)";
    }
  }

  // Non-color cue: a distinct glyph per type so the meaning survives without
  // relying on the accent colour alone (WCAG — don't encode by colour only).
  function iconFor(type: Toast["type"]): string {
    if (type === "success") return "check";
    if (type === "error") return "alert";
    return "info";
  }

  function handleAction(toast: Toast) {
    toast.action?.onAction();
    removeToast(toast.id);
  }
</script>

<!--
  Two persistent live regions, mounted for the lifetime of the app. Putting
  aria-live on each toast as it was inserted meant the region and its content
  appeared in the same tick, which most assistive tech does not announce.
  Errors are assertive, everything else polite -- so they need separate
  regions, since a region's politeness cannot change per message.
-->
<div class="sr-only" role="status" aria-live="polite">
  {#each $toasts.filter((t) => t.type !== "error") as toast (toast.id)}
    <p>{toast.title}{toast.message ? `. ${toast.message}` : ""}</p>
  {/each}
</div>
<div class="sr-only" role="alert" aria-live="assertive">
  {#each $toasts.filter((t) => t.type === "error") as toast (toast.id)}
    <p>{toast.title}{toast.message ? `. ${toast.message}` : ""}</p>
  {/each}
</div>

<!-- Sits above the mobile bottom nav, which is 56px + safe area. -->
<div
  class="toast-stack fixed flex flex-col gap-2 pointer-events-none max-w-sm"
  style="right: calc(1rem + 8px); z-index: var(--z-toast)"
>
  {#each $toasts as toast (toast.id)}
    <div
      class="pointer-events-auto flex items-start gap-3 rounded-xl px-4 py-3 shadow-xl border"
      style="background: var(--bg-surface); border-color: var(--border-color)"
      role="group"
      onmouseenter={() => pauseToast(toast.id)}
      onmouseleave={() => resumeToast(toast.id)}
      onfocusin={() => pauseToast(toast.id)}
      onfocusout={() => resumeToast(toast.id)}
      in:fly={{ x: 24, duration: dur(280), opacity: 0 }}
      out:fly={{ x: 24, duration: dur(180), opacity: 0 }}
      animate:flip={flipConfig}
    >
      <!-- Type glyph on a colour chip — the 10% neon pop + shape cue -->
      <div
        class="w-6 h-6 rounded-lg shrink-0 mt-0.5 flex items-center justify-center"
        style="color: {inkFor(toast.type)}; background: color-mix(in srgb, {inkFor(toast.type)} 14%, transparent)"
      >
        <Icon name={iconFor(toast.type)} size={14} />
      </div>

      <div class="flex-1 min-w-0">
        <p class="text-sm font-semibold" style="color: var(--text-primary)">{toast.title}</p>
        {#if toast.message}
          <p class="text-xs mt-0.5 break-words line-clamp-2" style="color: var(--text-secondary)">{toast.message}</p>
        {/if}
        {#if toast.action}
          <button
            onclick={() => handleAction(toast)}
            class="action-btn mt-2 text-xs font-semibold px-2.5 py-1 rounded-md"
            style="color: {inkFor(toast.type)}; background: color-mix(in srgb, {inkFor(toast.type)} 12%, transparent)"
          >
            {toast.action.label}
          </button>
        {/if}
      </div>

      <button
        onclick={() => removeToast(toast.id)}
        class="icon-btn shrink-0 p-1 rounded-lg min-w-[44px] min-h-[44px] flex items-center justify-center -mr-2 -mt-1"
        style="color: var(--text-secondary)"
        aria-label={tr($locale, "common.close")}
      >
        <Icon name="x" size={14} />
      </button>
    </div>
  {/each}
</div>

<style>
  .toast-stack {
    bottom: 1rem;
  }

  /* Below the desktop breakpoint the bottom nav owns the lower edge; a toast
     at bottom-4 covered the History and Settings tabs and, being
     pointer-events-auto, swallowed taps on them. */
  @media (max-width: 1023px) {
    .toast-stack {
      bottom: calc(5.5rem + env(safe-area-inset-bottom, 0px));
      left: 1rem;
      max-width: none;
    }
  }
</style>
