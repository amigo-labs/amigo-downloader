<script lang="ts">
  import { onMount } from "svelte";
  import { completeSetup, getSetupStatus } from "../lib/api";
  import { setupRequired } from "../lib/stores";
  import { locale, tr } from "../lib/i18n";
  import Card from "@amigo/ui/components/Card.svelte";
  import Button from "@amigo/ui/components/Button.svelte";
  import Field from "@amigo/ui/components/Field.svelte";
  import Banner from "@amigo/ui/components/Banner.svelte";
  import ProgressBar from "@amigo/ui/components/ProgressBar.svelte";

  // Like Login, this screen was outside the design system: Svelte 4 syntax,
  // undefined CSS variables falling through to hardcoded hexes, no i18n and
  // lowercase English validation strings with no aria-live.
  let step = $state<"checking" | "pin" | "credentials" | "submitting">("checking");
  let needsPin = $state(false);
  let pin = $state("");
  let username = $state("");
  let password = $state("");
  let passwordConfirm = $state("");
  let error = $state("");

  const MIN_PASSWORD = 8;

  onMount(async () => {
    try {
      const s = await getSetupStatus();
      if (!s.needs_setup) {
        setupRequired.set(false);
        location.hash = "#downloads";
        location.reload();
        return;
      }
      needsPin = s.needs_pin;
      step = needsPin ? "pin" : "credentials";
    } catch (e) {
      error = tr($locale, "setup.unreachable", { message: (e as Error).message });
    }
  });

  // Total steps and the current one, so the wizard says where the user is.
  let totalSteps = $derived(needsPin ? 2 : 1);
  let currentStep = $derived(step === "pin" ? 1 : totalSteps);

  // Simple, honest strength signal: length plus character-class variety.
  let strength = $derived.by(() => {
    if (!password) return 0;
    let score = Math.min(60, (password.length / 16) * 60);
    if (/[a-z]/.test(password) && /[A-Z]/.test(password)) score += 15;
    if (/\d/.test(password)) score += 12;
    if (/[^\w\s]/.test(password)) score += 13;
    return Math.min(100, Math.round(score));
  });
  let strengthKey = $derived(
    strength < 40 ? "setup.strength_weak" : strength < 70 ? "setup.strength_fair" : "setup.strength_strong",
  );

  let passwordError = $derived(
    password && password.length < MIN_PASSWORD
      ? tr($locale, "setup.password_too_short", { min: MIN_PASSWORD })
      : "",
  );
  let confirmError = $derived(
    passwordConfirm && password !== passwordConfirm ? tr($locale, "setup.password_mismatch") : "",
  );

  function advanceFromPin(e: SubmitEvent) {
    e.preventDefault();
    error = "";
    if (!pin.trim()) {
      error = tr($locale, "setup.pin_required");
      return;
    }
    step = "credentials";
  }

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    error = "";
    if (!username.trim()) {
      error = tr($locale, "setup.username_required");
      return;
    }
    if (password.length < MIN_PASSWORD) {
      error = tr($locale, "setup.password_too_short", { min: MIN_PASSWORD });
      return;
    }
    if (password !== passwordConfirm) {
      error = tr($locale, "setup.password_mismatch");
      return;
    }
    step = "submitting";
    try {
      await completeSetup({ username: username.trim(), password }, needsPin ? pin : undefined);
      setupRequired.set(false);
      location.hash = "#downloads";
      location.reload();
    } catch (e) {
      error = (e as Error).message || tr($locale, "setup.failed");
      step = "credentials";
    }
  }
</script>

<div class="setup-wrap">
  <Card padding="lg" elevation={3} class="setup-card">
    <div class="text-center mb-4">
      <img src="/amigo-logo.png" alt="" width="48" height="48" class="mx-auto rounded-full" />
      <h1 class="mt-3 text-lg font-bold" style="color: var(--text-primary)">
        {tr($locale, "setup.title")}
      </h1>
      <p class="text-xs mt-1" style="color: var(--text-secondary)">{tr($locale, "setup.lead")}</p>
      {#if step !== "checking" && totalSteps > 1}
        <p class="text-xs mt-2 font-semibold" style="color: var(--accent-ink)">
          {tr($locale, "setup.step_of", { current: currentStep, total: totalSteps })}
        </p>
      {/if}
    </div>

    {#if error}
      <div class="mb-4">
        <Banner tone="danger" role="alert">{error}</Banner>
      </div>
    {/if}

    {#if step === "checking"}
      <p class="text-sm text-center" style="color: var(--text-secondary)">
        {tr($locale, "setup.checking")}
      </p>
    {:else if step === "pin"}
      <form onsubmit={advanceFromPin} class="grid gap-4">
        <Field
          label={tr($locale, "setup.pin")}
          bind:value={pin}
          autocomplete="off"
          mono
          required
          hint={tr($locale, "setup.pin_hint")}
          data-autofocus
        />
        <Button type="submit" variant="solid" size="lg" full>
          {tr($locale, "setup.continue")}
        </Button>
      </form>
    {:else}
      <form onsubmit={submit} class="grid gap-4">
        <Field
          label={tr($locale, "setup.username")}
          bind:value={username}
          autocomplete="username"
          required
          data-autofocus
        />
        <div class="grid gap-1">
          <Field
            label={tr($locale, "setup.password")}
            type="password"
            bind:value={password}
            autocomplete="new-password"
            required
            error={passwordError}
            hint={tr($locale, "setup.password_hint", { min: MIN_PASSWORD })}
          />
          {#if password}
            <div class="flex items-center gap-2">
              <ProgressBar
                value={strength}
                size="xs"
                tone={strength < 40 ? "danger" : strength < 70 ? "warning" : "success"}
                label={tr($locale, "setup.strength")}
                valueText={tr($locale, strengthKey)}
              />
              <span class="text-xs shrink-0" style="color: var(--text-secondary)">
                {tr($locale, strengthKey)}
              </span>
            </div>
          {/if}
        </div>
        <Field
          label={tr($locale, "setup.password_confirm")}
          type="password"
          bind:value={passwordConfirm}
          autocomplete="new-password"
          required
          error={confirmError}
        />
        <Button
          type="submit"
          variant="solid"
          size="lg"
          full
          loading={step === "submitting"}
        >
          {tr($locale, "setup.submit")}
        </Button>
      </form>
    {/if}
  </Card>
</div>

<style>
  .setup-wrap {
    display: flex;
    min-height: 100dvh;
    align-items: center;
    justify-content: center;
    padding: var(--space-4);
    background: var(--bg-deep);
  }

  :global(.setup-card) {
    width: 100%;
    max-width: 26rem;
  }
</style>
