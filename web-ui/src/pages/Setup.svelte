<script lang="ts">
  import { onMount } from "svelte";
  import { completeSetup, getSetupStatus } from "../lib/api";
  import { setupRequired } from "../lib/stores";

  let step: "checking" | "pin" | "credentials" | "submitting" = "checking";
  let needsPin = false;
  let pin = "";
  let username = "";
  let password = "";
  let passwordConfirm = "";
  let error = "";

  onMount(async () => {
    try {
      const s = await getSetupStatus();
      if (!s.needs_setup) {
        // Setup already done — bounce back to the app.
        setupRequired.set(false);
        location.hash = "#downloads";
        location.reload();
        return;
      }
      needsPin = s.needs_pin;
      step = needsPin ? "pin" : "credentials";
    } catch (e: any) {
      error = `Could not reach the server: ${e.message}`;
    }
  });

  function advanceFromPin() {
    error = "";
    if (!pin.trim()) {
      error = "PIN required";
      return;
    }
    step = "credentials";
  }

  async function submit() {
    error = "";
    if (username.trim().length < 1) {
      error = "username required";
      return;
    }
    if (password.length < 8) {
      error = "password must be at least 8 characters";
      return;
    }
    if (password !== passwordConfirm) {
      error = "passwords do not match";
      return;
    }
    step = "submitting";
    try {
      await completeSetup({ username: username.trim(), password }, needsPin ? pin : undefined);
      setupRequired.set(false);
      location.hash = "#downloads";
      location.reload();
    } catch (e: any) {
      error = e.message || "setup failed";
      step = "credentials";
    }
  }
</script>

<div class="setup-wrap">
  <div class="card">
    <h1>Welcome to amigo</h1>
    <p class="lead">Let's create the admin account for this server.</p>

    {#if step === "checking"}
      <p>Checking server state…</p>
    {:else if step === "pin"}
      <label>
        Setup PIN
        <input type="text" bind:value={pin} autocomplete="off" />
      </label>
      <p class="hint">Provided by whoever started the server (look for <code>AMIGO_SETUP_PIN</code> in the container logs).</p>
      <button type="button" on:click={advanceFromPin}>Continue</button>
    {:else}
      <label>
        Admin username
        <input type="text" bind:value={username} autocomplete="username" />
      </label>
      <label>
        Password (min 8 chars)
        <input type="password" bind:value={password} autocomplete="new-password" />
      </label>
      <label>
        Confirm password
        <input type="password" bind:value={passwordConfirm} autocomplete="new-password" />
      </label>
      <button type="button" disabled={step === "submitting"} on:click={submit}>
        {step === "submitting" ? "Creating…" : "Create admin account"}
      </button>
    {/if}

    {#if error}
      <p class="error">{error}</p>
    {/if}
  </div>
</div>

<style>
  .setup-wrap {
    display: flex;
    min-height: 100vh;
    align-items: center;
    justify-content: center;
    padding: 1rem;
  }
  .card {
    max-width: 420px;
    width: 100%;
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    padding: 1.5rem;
    border-radius: 0.5rem;
    background: var(--surface, #1e1e24);
  }
  h1 {
    margin: 0;
  }
  .lead {
    margin: 0 0 0.5rem 0;
    color: var(--muted, #888);
  }
  label {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    font-size: 0.9rem;
  }
  input {
    padding: 0.5rem;
    background: var(--bg, #111);
    border: 1px solid var(--border, #333);
    border-radius: 0.25rem;
    color: inherit;
  }
  button {
    margin-top: 0.5rem;
    padding: 0.6rem 1rem;
    border: none;
    border-radius: 0.25rem;
    background: var(--accent, #2563eb);
    color: white;
    cursor: pointer;
  }
  button:disabled {
    opacity: 0.5;
    cursor: wait;
  }
  .error {
    color: #ef4444;
    margin: 0;
  }
  .hint {
    font-size: 0.85rem;
    color: var(--muted, #888);
    margin: 0;
  }
</style>
