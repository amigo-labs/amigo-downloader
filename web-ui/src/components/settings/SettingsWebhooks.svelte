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
    <h3 class="text-lg font-bold">Webhooks</h3>
    <button
      onclick={() => (showAdd = !showAdd)}
      class="px-3 py-1.5 rounded-lg text-xs font-semibold text-white"
      style="background: var(--accent-color)"
    >+ Add Webhook</button>
  </div>

  {#if showAdd}
    <div class="rounded-xl p-5 mb-4 space-y-3" style="background: var(--surface-2-color); border: 1px solid var(--border-color)">
      <div>
        <label class="text-xs font-semibold mb-1 block">Name</label>
        <input bind:value={name} type="text" placeholder="Discord Notifications"
          class="w-full rounded-lg px-3 py-2 text-sm outline-none"
          style="background: var(--surface-3-color); border: 1px solid var(--border-color); color: var(--text-color)" />
      </div>
      <div>
        <label class="text-xs font-semibold mb-1 block">URL</label>
        <input bind:value={url} type="url" placeholder="https://discord.com/api/webhooks/..."
          class="w-full rounded-lg px-3 py-2 text-sm font-mono outline-none"
          style="background: var(--surface-3-color); border: 1px solid var(--border-color); color: var(--text-color)" />
      </div>
      <div>
        <label class="text-xs font-semibold mb-1 block">Secret <span class="opacity-50">(optional)</span></label>
        <input bind:value={secret} type="text" placeholder="my-secret-key"
          class="w-full rounded-lg px-3 py-2 text-sm font-mono outline-none"
          style="background: var(--surface-3-color); border: 1px solid var(--border-color); color: var(--text-color)" />
      </div>
      <div>
        <label class="text-xs font-semibold mb-1 block">Events <span class="opacity-50">(comma-separated, * = all)</span></label>
        <input bind:value={events} type="text" placeholder="download.completed, download.failed"
          class="w-full rounded-lg px-3 py-2 text-sm font-mono outline-none"
          style="background: var(--surface-3-color); border: 1px solid var(--border-color); color: var(--text-color)" />
      </div>
      <div class="flex gap-2 pt-1">
        <button onclick={handleAdd}
          class="px-4 py-2 rounded-lg text-sm font-semibold text-white"
          style="background: var(--accent-color)"
          disabled={!name.trim() || !url.trim()}>Save</button>
        <button onclick={() => (showAdd = false)}
          class="px-4 py-2 rounded-lg text-sm"
          style="color: var(--text-secondary-color)">Cancel</button>
      </div>
    </div>
  {/if}

  {#if webhooks.length === 0 && !showAdd}
    <div class="rounded-xl p-5 text-center" style="background: var(--surface-2-color); border: 1px solid var(--border-color)">
      <p class="text-sm" style="color: var(--text-secondary-color)">
        No webhooks configured. Add one to receive notifications on Discord, Slack, Home Assistant, etc.
      </p>
    </div>
  {:else}
    <div class="space-y-2">
      {#each webhooks as wh}
        <div class="rounded-xl p-4 flex items-center justify-between gap-3" style="background: var(--surface-2-color); border: 1px solid var(--border-color)">
          <div class="min-w-0 flex-1">
            <p class="font-semibold text-sm truncate">{wh.name}</p>
            <p class="text-xs font-mono truncate" style="color: var(--text-secondary-color)">{wh.url}</p>
            <p class="text-[10px] mt-0.5" style="color: var(--text-secondary-color)">
              Events: {wh.events?.join(", ") || "*"}
              {#if wh.secret}&middot; signed{/if}
            </p>
          </div>
          <div class="flex gap-1.5 shrink-0">
            <button onclick={() => handleTest(wh.id)}
              class="px-2.5 py-1.5 rounded-lg text-xs border"
              style="border-color: var(--border-color); color: var(--text-secondary-color)">Test</button>
            <button onclick={() => handleDelete(wh.id)}
              class="px-2.5 py-1.5 rounded-lg text-xs text-red-400 border border-red-400/30 hover:bg-red-400/10">Delete</button>
          </div>
        </div>
      {/each}
    </div>
  {/if}
</section>
