<script lang="ts">
  import { getWebhooks, createWebhook, deleteWebhook, testWebhook } from "../../lib/api";
  import { addToast } from "../../lib/toast";

  let { webhooks = $bindable([]) }: { webhooks: any[] } = $props();
  let showAdd = $state(false);
  let name = $state("");
  let url = $state("");
  let secret = $state("");
  let events = $state("*");

  async function handleAdd() {
    if (!name.trim() || !url.trim()) return;
    try {
      const evts = events.split(",").map(e => e.trim()).filter(Boolean);
      await createWebhook({ name, url, secret: secret || undefined, events: evts });
      webhooks = await getWebhooks();
      showAdd = false;
      name = url = secret = "";
      events = "*";
      addToast("success", "Webhook added");
    } catch {
      addToast("error", "Failed to add webhook");
    }
  }

  async function handleDelete(id: string) {
    try {
      await deleteWebhook(id);
      webhooks = webhooks.filter(w => w.id !== id);
      addToast("info", "Webhook removed");
    } catch {
      addToast("error", "Failed to delete webhook");
    }
  }

  async function handleTest(id: string) {
    try {
      const result = await testWebhook(id);
      addToast("success", "Test sent", `Status: ${result.status || "OK"}`);
    } catch {
      addToast("error", "Test failed");
    }
  }
</script>

<section>
  <div class="flex items-center justify-between mb-4">
    <h3 class="text-lg font-bold" style="color: var(--text-primary)">Webhooks</h3>
    <button
      onclick={() => (showAdd = !showAdd)}
      class="px-3 py-1.5 rounded-lg text-xs font-semibold"
      style="background: var(--neon-primary); color: var(--bg-deep)"
    >+ Add Webhook</button>
  </div>

  {#if showAdd}
    <div class="rounded-xl p-5 mb-4 space-y-3" style="background: var(--bg-surface); border: 1px solid var(--border-color)">
      <div>
        <label class="text-xs font-semibold mb-1 block" style="color: var(--text-secondary)">Name</label>
        <input bind:value={name} type="text" placeholder="Discord Notifications"
          class="w-full rounded-lg px-3 py-2 text-sm"
          style="background: var(--bg-surface-2); border: 1px solid var(--border-color); color: var(--text-primary)" />
      </div>
      <div>
        <label class="text-xs font-semibold mb-1 block" style="color: var(--text-secondary)">URL</label>
        <input bind:value={url} type="url" placeholder="https://discord.com/api/webhooks/..."
          class="w-full rounded-lg px-3 py-2 text-sm"
          style="font-family: var(--font-mono);background: var(--bg-surface-2); border: 1px solid var(--border-color); color: var(--text-primary)" />
      </div>
      <div>
        <label class="text-xs font-semibold mb-1 block" style="color: var(--text-secondary)">Secret <span class="opacity-50">(optional)</span></label>
        <input bind:value={secret} type="text" placeholder="my-secret-key"
          class="w-full rounded-lg px-3 py-2 text-sm"
          style="font-family: var(--font-mono);background: var(--bg-surface-2); border: 1px solid var(--border-color); color: var(--text-primary)" />
      </div>
      <div>
        <label class="text-xs font-semibold mb-1 block" style="color: var(--text-secondary)">Events <span class="opacity-50">(comma-separated, * = all)</span></label>
        <input bind:value={events} type="text" placeholder="download.completed, download.failed"
          class="w-full rounded-lg px-3 py-2 text-sm"
          style="font-family: var(--font-mono);background: var(--bg-surface-2); border: 1px solid var(--border-color); color: var(--text-primary)" />
      </div>
      <div class="flex gap-2 pt-1">
        <button onclick={handleAdd}
          class="px-4 py-2 rounded-lg text-sm font-semibold"
          style="background: var(--neon-primary); color: var(--bg-deep)"
          disabled={!name.trim() || !url.trim()}>Save</button>
        <button onclick={() => (showAdd = false)}
          class="px-4 py-2 rounded-lg text-sm"
          style="color: var(--text-secondary)">Cancel</button>
      </div>
    </div>
  {/if}

  {#if webhooks.length === 0 && !showAdd}
    <div class="rounded-xl p-5 text-center" style="background: var(--bg-surface); border: 1px solid var(--border-color)">
      <p class="text-sm" style="color: var(--text-secondary)">
        No webhooks configured. Add one to receive notifications on Discord, Slack, Home Assistant, etc.
      </p>
    </div>
  {:else}
    <div class="space-y-2">
      {#each webhooks as wh}
        <div class="rounded-xl p-4 flex items-center justify-between gap-3" style="background: var(--bg-surface); border: 1px solid var(--border-color)">
          <div class="min-w-0 flex-1">
            <p class="font-semibold text-sm truncate" style="color: var(--text-primary)">{wh.name}</p>
            <p class="text-xs truncate" style="font-family: var(--font-mono);color: var(--text-secondary)">{wh.url}</p>
            <p class="text-[10px] mt-0.5" style="color: var(--text-secondary)">
              Events: {wh.events?.join(", ") || "*"}
              {#if wh.secret}&middot; signed{/if}
            </p>
          </div>
          <div class="flex gap-1.5 shrink-0">
            <button onclick={() => handleTest(wh.id)}
              class="px-2.5 py-1.5 rounded-lg text-xs border"
              style="border-color: var(--border-color); color: var(--text-secondary)">Test</button>
            <button onclick={() => handleDelete(wh.id)}
              class="px-2.5 py-1.5 rounded-lg text-xs"
              style="color: var(--neon-accent); border: 1px solid color-mix(in srgb, var(--neon-accent) 20%, transparent)">Delete</button>
          </div>
        </div>
      {/each}
    </div>
  {/if}
</section>
