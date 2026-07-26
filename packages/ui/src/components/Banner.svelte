<script lang="ts">
  import type { Snippet } from "svelte";
  import Icon from "./Icon.svelte";
  import IconButton from "./IconButton.svelte";

  let {
    tone = "info",
    title,
    dismissable = false,
    dismissLabel = "Dismiss",
    icon,
    role,
    class: className = "",
    ondismiss,
    children,
    actions,
  }: {
    tone?: "info" | "success" | "warning" | "danger";
    title?: string;
    dismissable?: boolean;
    dismissLabel?: string;
    icon?: string;
    /** "alert" for errors that appear after an action; omit for static notes. */
    role?: "alert" | "status";
    class?: string;
    ondismiss?: () => void;
    children: Snippet;
    actions?: Snippet;
  } = $props();

  const DEFAULT_ICON = {
    info: "info",
    success: "check",
    warning: "alert",
    danger: "alert",
  } as const;
</script>

<div class="banner {tone} {className}" {role}>
  <span class="icon" aria-hidden="true">
    <Icon name={icon ?? DEFAULT_ICON[tone]} size={16} />
  </span>
  <div class="body">
    {#if title}<p class="title">{title}</p>{/if}
    <div class="text">{@render children()}</div>
  </div>
  {#if actions}
    <div class="actions">{@render actions()}</div>
  {/if}
  {#if dismissable}
    <IconButton icon="x" label={dismissLabel} size="sm" onclick={() => ondismiss?.()} />
  {/if}
</div>

<style>
  .banner {
    --bn-ink: var(--accent-ink);
    --bn-tone: var(--accent);

    display: flex;
    align-items: flex-start;
    gap: 0.75rem;
    padding: var(--space-3) var(--space-4);
    border: 1px solid color-mix(in srgb, var(--bn-tone) 25%, transparent);
    border-radius: var(--radius-lg);
    background: color-mix(in srgb, var(--bn-tone) 8%, var(--bg-surface));
  }

  .banner.success { --bn-ink: var(--success-ink); --bn-tone: var(--success); }
  .banner.warning { --bn-ink: var(--warning-ink); --bn-tone: var(--warning); }
  .banner.danger { --bn-ink: var(--danger-ink); --bn-tone: var(--danger); }

  .icon {
    color: var(--bn-ink);
    display: flex;
    padding-top: 1px;
  }

  .body {
    flex: 1;
    min-width: 0;
  }

  .title {
    margin: 0;
    font-size: var(--font-sm);
    font-weight: 600;
    color: var(--text-primary);
  }

  .text {
    font-size: var(--font-sm);
    color: var(--text-secondary);
  }

  .title + .text {
    margin-top: 0.125rem;
  }

  .actions {
    display: flex;
    gap: 0.5rem;
    align-items: center;
    flex-shrink: 0;
  }
</style>
