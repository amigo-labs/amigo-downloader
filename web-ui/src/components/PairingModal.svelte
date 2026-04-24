<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import {
    approvePairing,
    denyPairing,
    listPendingPairings,
    type PendingPairing,
  } from "../lib/api";

  let pending: PendingPairing[] = [];
  let timer: ReturnType<typeof setInterval> | undefined;

  async function refresh() {
    try {
      pending = await listPendingPairings();
    } catch {
      // Not logged in, 401 handled globally.
      pending = [];
    }
  }

  onMount(() => {
    refresh();
    timer = setInterval(refresh, 3000);
  });

  onDestroy(() => {
    if (timer) clearInterval(timer);
  });

  async function approve(id: string) {
    await approvePairing(id);
    refresh();
  }
  async function deny(id: string) {
    await denyPairing(id);
    refresh();
  }
</script>

{#if pending.length > 0}
  <div class="overlay" role="dialog" aria-modal="true">
    <div class="card">
      <h2>Device wants to connect</h2>
      {#each pending as p (p.id)}
        <div class="request">
          <div class="name">{p.device_name}</div>
          <div class="meta">
            <span>from <code>{p.source_ip}</code></span>
            <span class="fp">Verification: <strong>{p.fingerprint}</strong></span>
          </div>
          {#if p.user_agent}
            <div class="ua">{p.user_agent}</div>
          {/if}
          <div class="actions">
            <button class="approve" on:click={() => approve(p.id)}>Approve</button>
            <button class="deny" on:click={() => deny(p.id)}>Deny</button>
          </div>
        </div>
      {/each}
    </div>
  </div>
{/if}

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
    padding: 1rem;
  }
  .card {
    max-width: 460px;
    width: 100%;
    background: var(--surface, #1e1e24);
    border-radius: 0.5rem;
    padding: 1.25rem;
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }
  h2 {
    margin: 0;
  }
  .request {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    padding: 0.75rem;
    border: 1px solid var(--border, #333);
    border-radius: 0.25rem;
  }
  .name {
    font-weight: 600;
  }
  .meta {
    display: flex;
    gap: 1rem;
    font-size: 0.9rem;
    color: var(--muted, #888);
  }
  .fp strong {
    font-family: ui-monospace, monospace;
    color: var(--fg, #fff);
    letter-spacing: 0.05em;
  }
  .ua {
    font-size: 0.8rem;
    color: var(--muted, #666);
  }
  .actions {
    display: flex;
    gap: 0.5rem;
    justify-content: flex-end;
  }
  button {
    padding: 0.4rem 1rem;
    border: none;
    border-radius: 0.25rem;
    cursor: pointer;
  }
  button.approve {
    background: #16a34a;
    color: white;
  }
  button.deny {
    background: var(--surface-2, #374151);
    color: inherit;
  }
</style>
