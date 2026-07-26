<script lang="ts">
  import type { Snippet } from "svelte";
  import Icon from "./Icon.svelte";

  let {
    icon,
    image,
    title,
    description,
    size = "md",
    class: className = "",
    action,
  }: {
    icon?: string;
    /** Decorative image src; rendered with alt="" since title carries meaning. */
    image?: string;
    title: string;
    description?: string;
    size?: "sm" | "md";
    class?: string;
    action?: Snippet;
  } = $props();
</script>

<div class="empty {size} {className}">
  {#if image}
    <img src={image} alt="" width={size === "sm" ? 40 : 64} height={size === "sm" ? 40 : 64} />
  {:else if icon}
    <span class="icon" aria-hidden="true"><Icon name={icon} size={size === "sm" ? 24 : 32} /></span>
  {/if}
  <p class="title">{title}</p>
  {#if description}<p class="desc">{description}</p>{/if}
  {#if action}
    <div class="action">{@render action()}</div>
  {/if}
</div>

<style>
  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    text-align: center;
  }

  .empty.sm { padding: var(--space-8) var(--space-4); }
  .empty.md { padding: var(--space-16) var(--space-4); }

  img {
    border-radius: var(--radius-md);
    opacity: 0.3;
  }

  .icon {
    color: var(--text-muted);
  }

  .title {
    margin: var(--space-4) 0 0;
    font-size: var(--font-sm);
    font-weight: 600;
    color: var(--text-primary);
  }

  .desc {
    margin: var(--space-1) 0 0;
    font-size: var(--font-xs);
    color: var(--text-secondary);
    max-width: 32rem;
  }

  .action {
    margin-top: var(--space-4);
  }
</style>
