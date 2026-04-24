<script lang="ts">
  import { toasts, removeToast, type Toast } from "../lib/toast";
  import Icon from "@amigo/ui/components/Icon.svelte";

  function colorFor(type: Toast["type"]) {
    switch (type) {
      case "success": return "var(--neon-success)";
      case "error": return "var(--neon-accent)";
      case "info": return "var(--neon-primary)";
      default: return "var(--neon-primary)";
    }
  }
</script>

<!-- Scrollbar offset (audit L3) -->
<div class="fixed bottom-4 z-[100] flex flex-col gap-2 pointer-events-none max-w-sm" style="right: calc(1rem + 8px)">
  {#each $toasts as toast (toast.id)}
    <div
      class="toast-enter pointer-events-auto flex items-start gap-3 rounded-xl px-4 py-3 shadow-xl border"
      style="background: var(--bg-surface); border-color: var(--border-color)"
    >
      <!-- Color bar — neon accent, the 10% pop -->
      <div class="w-1 h-8 rounded-full shrink-0 mt-0.5" style="background: {colorFor(toast.type)}"></div>

      <div class="flex-1 min-w-0">
        <p class="text-sm font-semibold" style="color: var(--text-primary)">{toast.title}</p>
        {#if toast.message}
          <p class="text-xs mt-0.5 truncate" style="color: var(--text-secondary)">{toast.message}</p>
        {/if}
      </div>

      <button
        onclick={() => removeToast(toast.id)}
        class="shrink-0 p-1 rounded min-w-[44px] min-h-[44px] flex items-center justify-center"
        style="color: var(--text-secondary)"
        aria-label="Dismiss notification"
      >
        <Icon name="x" size={14} />
      </button>
    </div>
  {/each}
</div>

<style>
  @keyframes toast-slide-in {
    from {
      opacity: 0;
      transform: translateX(100%) scale(0.95);
    }
    to {
      opacity: 1;
      transform: translateX(0) scale(1);
    }
  }

  .toast-enter {
    animation: toast-slide-in 0.3s cubic-bezier(0.16, 1, 0.3, 1) forwards;
  }
</style>
