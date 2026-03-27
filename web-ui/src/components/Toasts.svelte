<script lang="ts">
  import { toasts, removeToast, type Toast } from "../lib/toast";

  function iconFor(type: Toast["type"]) {
    switch (type) {
      case "success": return "checkmark";
      case "error": return "x-circle";
      case "info": return "info";
      default: return "info";
    }
  }

  function colorFor(type: Toast["type"]) {
    switch (type) {
      case "success": return "var(--color-success)";
      case "error": return "var(--color-error)";
      case "info": return "var(--accent-color)";
      default: return "var(--accent-color)";
    }
  }
</script>

<div class="fixed bottom-4 right-4 z-[100] flex flex-col gap-2 pointer-events-none max-w-sm">
  {#each $toasts as toast (toast.id)}
    <div
      class="toast-enter pointer-events-auto flex items-start gap-3 rounded-xl px-4 py-3 shadow-xl backdrop-blur-lg border"
      style="background: color-mix(in srgb, var(--surface-color) 90%, transparent); border-color: var(--border-color)"
    >
      <!-- Color bar -->
      <div class="w-1 h-8 rounded-full shrink-0 mt-0.5" style="background: {colorFor(toast.type)}"></div>

      <div class="flex-1 min-w-0">
        <p class="text-sm font-semibold">{toast.title}</p>
        {#if toast.message}
          <p class="text-xs mt-0.5 truncate" style="color: var(--text-secondary-color)">{toast.message}</p>
        {/if}
      </div>

      <button
        onclick={() => removeToast(toast.id)}
        class="shrink-0 p-0.5 rounded transition-colors"
        style="color: var(--text-secondary-color)"
      >
        <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="2">
          <path d="M6 18L18 6M6 6l12 12" />
        </svg>
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
