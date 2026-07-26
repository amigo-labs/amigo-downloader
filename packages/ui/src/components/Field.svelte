<script lang="ts">
  import type { Snippet } from "svelte";

  // Wires label/hint/error to the control so screen readers announce a name,
  // its description and its validation state. The hand-rolled forms this
  // replaces used a sibling <label> with no `for`, leaving every field
  // unlabelled.
  let {
    label,
    value = $bindable(),
    type = "text",
    hint,
    error,
    required = false,
    placeholder,
    mono = false,
    min,
    max,
    step,
    disabled = false,
    autocomplete,
    class: className = "",
    children,
    ...rest
  }: {
    label: string;
    value?: string | number;
    type?: "text" | "password" | "number" | "email" | "url" | "search";
    hint?: string;
    error?: string;
    required?: boolean;
    placeholder?: string;
    mono?: boolean;
    min?: number;
    max?: number;
    step?: number;
    disabled?: boolean;
    autocomplete?: string;
    class?: string;
    /** Render a custom control (select, textarea) instead of the input. */
    children?: Snippet<[{ id: string; describedBy: string | undefined }]>;
    [key: string]: unknown;
  } = $props();

  const id = $props.id();
  const hintId = `${id}-hint`;
  const errorId = `${id}-error`;
  const describedBy = $derived(
    [hint ? hintId : null, error ? errorId : null].filter(Boolean).join(" ") || undefined,
  );
</script>

<div class="field {className}" class:has-error={!!error}>
  <label for={id}>
    {label}{#if required}<span class="req" aria-hidden="true">*</span>{/if}
  </label>

  {#if children}
    {@render children({ id, describedBy })}
  {:else}
    <input
      {id}
      {type}
      {placeholder}
      {min}
      {max}
      {step}
      {disabled}
      {required}
      {autocomplete}
      class:mono
      bind:value
      aria-describedby={describedBy}
      aria-invalid={error ? "true" : undefined}
      aria-errormessage={error ? errorId : undefined}
      {...rest}
    />
  {/if}

  {#if hint && !error}<p id={hintId} class="hint">{hint}</p>{/if}
  {#if error}<p id={errorId} class="error">{error}</p>{/if}
</div>

<style>
  .field {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    min-width: 0;
  }

  label {
    font-size: var(--font-xs);
    font-weight: 600;
    color: var(--text-secondary);
  }

  .req {
    color: var(--danger-ink);
    margin-left: 0.15rem;
  }

  input {
    min-height: 40px;
    padding: 0 0.625rem;
    background: var(--bg-surface-2);
    border: 1px solid var(--border-color);
    border-radius: var(--radius-md);
    color: var(--text-primary);
    font-size: var(--font-sm);
    font-family: inherit;
    min-width: 0;
  }

  input.mono {
    font-family: var(--font-mono);
  }

  input:disabled {
    opacity: 0.55;
    cursor: not-allowed;
  }

  .has-error input {
    border-color: var(--danger-ink);
  }

  .hint {
    margin: 0;
    font-size: var(--font-xs);
    color: var(--text-muted);
  }

  .error {
    margin: 0;
    font-size: var(--font-xs);
    color: var(--danger-ink);
  }
</style>
