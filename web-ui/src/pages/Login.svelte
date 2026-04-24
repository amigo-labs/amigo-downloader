<script lang="ts">
  import { login } from "../lib/api";
  import { authRequired } from "../lib/stores";

  let username = "";
  let password = "";
  let busy = false;
  let error = "";

  async function submit() {
    error = "";
    busy = true;
    try {
      await login(username, password);
      authRequired.set(false);
      location.hash = "#downloads";
      location.reload();
    } catch (e: any) {
      error = e.status === 401 ? "invalid username or password" : (e.message || "login failed");
      busy = false;
    }
  }
</script>

<div class="login-wrap">
  <form class="card" on:submit|preventDefault={submit}>
    <h1>Sign in to amigo</h1>

    <label>
      Username
      <input type="text" bind:value={username} autocomplete="username" autofocus />
    </label>
    <label>
      Password
      <input type="password" bind:value={password} autocomplete="current-password" />
    </label>

    <button type="submit" disabled={busy}>
      {busy ? "Signing in…" : "Sign in"}
    </button>

    {#if error}
      <p class="error">{error}</p>
    {/if}
  </form>
</div>

<style>
  .login-wrap {
    display: flex;
    min-height: 100vh;
    align-items: center;
    justify-content: center;
    padding: 1rem;
  }
  .card {
    max-width: 380px;
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
</style>
