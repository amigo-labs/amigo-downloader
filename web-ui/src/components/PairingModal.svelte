<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import {
    approvePairing,
    denyPairing,
    listPendingPairings,
    type PendingPairing,
  } from "../lib/api";
  import { locale, tr } from "../lib/i18n";
  import { openLayer, layerState } from "../lib/overlays.svelte";
  import Dialog from "@amigo/ui/components/Dialog.svelte";
  import Button from "@amigo/ui/components/Button.svelte";
  import Card from "@amigo/ui/components/Card.svelte";

  // This modal referenced five CSS variables that are defined nowhere
  // (--surface, --border, --muted, --fg, --surface-2), so it always rendered
  // with its hardcoded dark fallbacks and was unreadable in light mode. It
  // also had no accessible name, no focus trap and no way out by keyboard.
  let pending = $state<PendingPairing[]>([]);
  let busyId = $state<string | null>(null);
  let timer: ReturnType<typeof setInterval> | undefined;

  async function refresh() {
    try {
      pending = await listPendingPairings();
    } catch {
      // Not logged in; 401 is handled globally.
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

  // Not dismissible: an unanswered pairing request is a security decision, so
  // Escape and backdrop clicks must not quietly leave it pending. Deny is a
  // real, focusable button — which it previously was not.
  const layer = layerState("pairing");
  $effect(() => {
    if (pending.length === 0) return;
    return openLayer({
      id: "pairing",
      kind: "critical",
      label: "Pairing request",
      dismissible: false,
    });
  });

  async function respond(id: string, approve: boolean) {
    if (busyId) return;
    busyId = id;
    try {
      await (approve ? approvePairing(id) : denyPairing(id));
      await refresh();
    } finally {
      busyId = null;
    }
  }
</script>

{#if pending.length > 0}
  <Dialog
    title={tr($locale, "pairing.title")}
    description={tr($locale, "pairing.subtitle")}
    size="md"
    layer="modal"
    dismissable={false}
    showClose={false}
    isTop={$layer.isTop}
  >
    <div class="grid gap-3">
      {#each pending as p (p.id)}
        <Card padding="md" neon={false}>
          <p class="font-semibold text-sm" style="color: var(--text-primary)">{p.device_name}</p>
          <p class="text-xs mt-0.5" style="color: var(--text-secondary)">
            {tr($locale, "pairing.from")}
            <code style="font-family: var(--font-mono)">{p.source_ip}</code>
          </p>
          <p class="text-xs mt-1" style="color: var(--text-secondary)">
            {tr($locale, "pairing.verification")}
            <strong style="font-family: var(--font-mono); color: var(--accent-ink)">{p.fingerprint}</strong>
          </p>
          {#if p.user_agent}
            <p class="text-xs mt-1 break-all" style="color: var(--text-muted)">{p.user_agent}</p>
          {/if}
          <div class="flex gap-2 mt-3">
            <Button
              variant="solid"
              size="sm"
              disabled={busyId !== null}
              loading={busyId === p.id}
              onclick={() => respond(p.id, true)}
            >
              {tr($locale, "pairing.approve")}
            </Button>
            <Button
              variant="outline"
              tone="danger"
              size="sm"
              disabled={busyId !== null}
              onclick={() => respond(p.id, false)}
            >
              {tr($locale, "pairing.deny")}
            </Button>
          </div>
        </Card>
      {/each}
    </div>
  </Dialog>
{/if}
