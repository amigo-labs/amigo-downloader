<script lang="ts">
  import { flip } from "svelte/animate";
  import { fly } from "svelte/transition";
  import { toasts, removeToast, pauseToast, resumeToast, type Toast } from "../lib/toast";
  import { dur, flipConfig } from "../lib/motion";
  import { locale, tr } from "../lib/i18n";
  import Icon from "@amigo/ui/components/Icon.svelte";

  function colorFor(type: Toast["type"]) {
    switch (type) {
      case "success": return "var(--neon-success)";
      case "error": return "var(--neon-accent)";
      default: return "var(--neon-primary)";
    }
  }

  // Non-color cue: a distinct glyph per type so the meaning survives without
  // relying on the accent colour alone (WCAG — don't encode by colour only).
  function iconFor(type: Toast["type"]): string {
    return type === "success" ? "check" : "info";
  }

  function handleAction(toast: Toast) {
    toast.action?.onAction();
    removeToast(toast.id);
  }
</script>

<!-- Scrollbar offset (audit L3) -->
<div class="fixed bottom-4 z-[100] flex flex-col gap-2 pointer-events-none max-w-sm" style="right: calc(1rem + 8px)">
  {#each $toasts as toast (toast.id)}
    <div
      class="pointer-events-auto flex items-start gap-3 rounded-xl px-4 py-3 shadow-xl border"
      style="background: var(--bg-surface); border-color: var(--border-color)"
      role={toast.type === "error" ? "alert" : "status"}
      aria-live={toast.type === "error" ? "assertive" : "polite"}
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
        style="color: {colorFor(toast.type)}; background: color-mix(in srgb, {colorFor(toast.type)} 14%, transparent)"
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
            style="color: {colorFor(toast.type)}; background: color-mix(in srgb, {colorFor(toast.type)} 12%, transparent)"
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
