<script lang="ts">
  import { login } from "../lib/api";
  import { authRequired } from "../lib/stores";
  import { locale, tr } from "../lib/i18n";
  import Card from "@amigo/ui/components/Card.svelte";
  import Button from "@amigo/ui/components/Button.svelte";
  import Field from "@amigo/ui/components/Field.svelte";
  import Banner from "@amigo/ui/components/Banner.svelte";

  // This screen used to live outside the design system entirely: Svelte 4
  // syntax, no i18n, and five CSS variables (--surface, --border, --accent,
  // --muted, --bg) that are declared nowhere, so it always fell through to
  // hardcoded dark hexes and ignored light mode and every palette. It is the
  // first thing a user of a secured instance sees.
  let username = $state("");
  let password = $state("");
  let busy = $state(false);
  let error = $state("");

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    error = "";
    busy = true;
    try {
      await login(username, password);
      authRequired.set(false);
      location.hash = "#downloads";
      location.reload();
    } catch (e) {
      const status = (e as { status?: number }).status;
      error = status === 401
        ? tr($locale, "login.invalid")
        : (e as Error).message || tr($locale, "login.failed");
      busy = false;
    }
  }
</script>

<div class="login-wrap">
  <Card padding="lg" elevation={3} class="login-card">
    <form onsubmit={submit} class="grid gap-4">
      <div class="text-center">
        <img src="/amigo-logo.png" alt="" width="48" height="48" class="mx-auto rounded-full" />
        <h1 class="mt-3 text-lg font-bold" style="color: var(--text-primary)">
          {tr($locale, "login.title")}
        </h1>
        <p class="text-xs mt-1" style="color: var(--text-secondary)">
          {tr($locale, "login.subtitle")}
        </p>
      </div>

      {#if error}
        <!-- role="alert" so the failure is announced; it used to be a plain
             <p> that screen readers never mentioned. -->
        <Banner tone="danger" role="alert">{error}</Banner>
      {/if}

      <Field
        label={tr($locale, "login.username")}
        bind:value={username}
        autocomplete="username"
        required
        data-autofocus
      />
      <Field
        label={tr($locale, "login.password")}
        type="password"
        bind:value={password}
        autocomplete="current-password"
        required
      />

      <Button type="submit" variant="solid" size="lg" full loading={busy}>
        {tr($locale, "login.submit")}
      </Button>
    </form>
  </Card>
</div>

<style>
  .login-wrap {
    display: flex;
    min-height: 100dvh;
    align-items: center;
    justify-content: center;
    padding: var(--space-4);
    background: var(--bg-deep);
  }

  :global(.login-card) {
    width: 100%;
    max-width: 22rem;
  }
</style>
