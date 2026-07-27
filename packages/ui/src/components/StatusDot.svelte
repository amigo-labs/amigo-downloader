<script lang="ts">
  // Status must never be conveyed by colour alone (WCAG 1.4.1), so the label
  // ships with the dot — visible or screen-reader-only, but always present.
  let {
    status,
    label,
    showLabel = false,
    pulse = false,
    class: className = "",
  }: {
    status: "online" | "degraded" | "error" | "idle";
    label: string;
    showLabel?: boolean;
    pulse?: boolean;
    class?: string;
  } = $props();
</script>

<span class="wrap {className}">
  <span class="dot {status}" class:pulse aria-hidden="true"></span>
  <span class="label" class:sr-only={!showLabel}>{label}</span>
</span>

<style>
  .wrap {
    display: inline-flex;
    align-items: center;
    gap: 0.375rem;
  }

  .dot {
    width: 0.5rem;
    height: 0.5rem;
    border-radius: var(--radius-full);
    flex-shrink: 0;
  }

  .dot.online {
    background: var(--status-online);
    box-shadow: 0 0 6px color-mix(in srgb, var(--status-online) 40%, transparent);
  }

  .dot.degraded {
    background: var(--status-degraded);
    box-shadow: 0 0 6px color-mix(in srgb, var(--status-degraded) 40%, transparent);
  }

  .dot.error {
    background: var(--status-error);
    box-shadow: 0 0 6px color-mix(in srgb, var(--status-error) 40%, transparent);
  }

  .dot.idle {
    background: var(--text-muted);
  }

  .dot.pulse {
    animation: dot-pulse 2s ease-in-out infinite;
  }

  @keyframes dot-pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.45; }
  }

  .label {
    font-size: var(--font-xs);
    color: var(--text-secondary);
    font-family: var(--font-mono);
  }

  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    padding: 0;
    margin: -1px;
    overflow: hidden;
    clip: rect(0, 0, 0, 0);
    white-space: nowrap;
    border-width: 0;
  }
</style>
