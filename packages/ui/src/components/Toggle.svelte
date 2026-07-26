<script lang="ts">
  let {
    checked = $bindable(),
    label,
    description,
    size = "md",
    disabled = false,
    class: className = "",
    onchange,
  }: {
    checked: boolean;
    label: string;
    description?: string;
    size?: "sm" | "md";
    disabled?: boolean;
    class?: string;
    onchange?: (checked: boolean) => void;
  } = $props();

  const descId = $props.id();

  function toggle() {
    if (disabled) return;
    checked = !checked;
    onchange?.(checked);
  }
</script>

<div class="row {size} {className}">
  <div class="text">
    <p class="label">{label}</p>
    {#if description}<p id={descId} class="desc">{description}</p>{/if}
  </div>
  <button
    type="button"
    role="switch"
    aria-checked={checked}
    aria-label={label}
    aria-describedby={description ? descId : undefined}
    class="switch"
    class:on={checked}
    {disabled}
    onclick={toggle}
  >
    <span class="knob"></span>
  </button>
</div>

<style>
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-4);
  }

  .text {
    min-width: 0;
  }

  .label {
    margin: 0;
    font-size: var(--font-sm);
    font-weight: 500;
    color: var(--text-primary);
  }

  .desc {
    margin: 0.125rem 0 0;
    font-size: var(--font-xs);
    color: var(--text-secondary);
  }

  /* The visible track is 24px tall but the hit area is padded to 44px so the
     control meets the touch-target minimum without looking oversized. */
  .switch {
    position: relative;
    flex-shrink: 0;
    display: inline-flex;
    align-items: center;
    border: none;
    background: transparent;
    cursor: pointer;
    padding: 10px 0;
    min-height: 44px;
  }

  .switch::before {
    content: "";
    display: block;
    border-radius: var(--radius-full);
    background: var(--bg-surface-2);
    border: 1px solid var(--border-color);
    transition: background-color var(--dur-fast) var(--ease-out);
  }

  .md .switch::before { width: 44px; height: 24px; }
  .sm .switch::before { width: 36px; height: 20px; }

  .switch.on::before {
    background: var(--accent-solid);
    border-color: var(--accent-solid);
  }

  .knob {
    position: absolute;
    border-radius: var(--radius-full);
    background: var(--text-primary);
    transition: transform var(--dur-fast) var(--ease-out), background-color var(--dur-fast) var(--ease-out);
  }

  .md .knob { width: 18px; height: 18px; left: 3px; }
  .sm .knob { width: 14px; height: 14px; left: 3px; }

  .md .switch.on .knob { transform: translateX(20px); background: var(--on-accent); }
  .sm .switch.on .knob { transform: translateX(16px); background: var(--on-accent); }

  .switch:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .switch:focus-visible {
    outline: none;
  }

  .switch:focus-visible::before {
    outline: 2px solid var(--accent-ink);
    outline-offset: 2px;
  }
</style>
