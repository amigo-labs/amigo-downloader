<script lang="ts">
  import { getWebhooks, createWebhook, deleteWebhook, testWebhook } from "../../lib/api";
  import { addToast } from "../../lib/toast";
  import { locale, tr } from "../../lib/i18n";

  let { webhooks = $bindable([]) }: { webhooks: any[] } = $props();
  let showAdd = $state(false);
  let name = $state("");
  let url = $state("");
  let secret = $state("");
  let events = $state("*");

  const inputStyle =
    "background: var(--bg-surface-2); border: 1px solid var(--border-color); color: var(--text-primary)";
  const monoInputStyle = `font-family: var(--font-mono);${inputStyle}`;

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
    <h3 class="text-lg font-bold" style="color: var(--text-primary)">{tr($locale, "settings.webhooks")}</h3>
    <button
      onclick={() => (showAdd = !showAdd)}
      class="action-btn px-3 py-1.5 rounded-lg text-xs font-semibold"
      style="background: var(--neon-primary); color: var(--bg-deep)"
    >+ {tr($locale, "settings.add_webhook")}</button>
  </div>

  {#if showAdd}
    <div class="rounded-xl p-5 mb-4 space-y-3" style="background: var(--bg-surface); border: 1px solid var(--border-color)">
      <label class="block">
        <span class="text-xs font-semibold mb-1 block" style="color: var(--text-secondary)">Name</span>
        <input bind:value={name} type="text" placeholder="Discord Notifications"
          class="w-full rounded-lg px-3 py-2 text-sm" style={inputStyle} />
      </label>
      <label class="block">
        <span class="text-xs font-semibold mb-1 block" style="color: var(--text-secondary)">URL</span>
        <input bind:value={url} type="url" placeholder="https://discord.com/api/webhooks/..."
          class="w-full rounded-lg px-3 py-2 text-sm" style={monoInputStyle} />
      </label>
      <label class="block">
        <span class="text-xs font-semibold mb-1 block" style="color: var(--text-secondary)">Secret <span class="opacity-50">(optional)</span></span>
        <input bind:value={secret} type="text" placeholder="my-secret-key"
          class="w-full rounded-lg px-3 py-2 text-sm" style={monoInputStyle} />
      </label>
      <label class="block">
        <span class="text-xs font-semibold mb-1 block" style="color: var(--text-secondary)">Events <span class="opacity-50">(comma-separated, * = all)</span></span>
        <input bind:value={events} type="text" placeholder="download.completed, download.failed"
          class="w-full rounded-lg px-3 py-2 text-sm" style={monoInputStyle} />
      </label>
      <div class="flex gap-2 pt-1">
        <button onclick={handleAdd}
          class="action-btn px-4 py-2 rounded-lg text-sm font-semibold"
          style="background: var(--neon-primary); color: var(--bg-deep)"
          disabled={!name.trim() || !url.trim()}>{tr($locale, "common.save")}</button>
        <button onclick={() => (showAdd = false)}
          class="px-4 py-2 rounded-lg text-sm"
          style="color: var(--text-secondary)">{tr($locale, "common.cancel")}</button>
      </div>
    </div>
  {/if}

  {#if webhooks.length === 0 && !showAdd}
    <div class="rounded-xl p-5 text-center" style="background: var(--bg-surface); border: 1px solid var(--border-color)">
      <p class="text-sm" style="color: var(--text-secondary)">
        {tr($locale, "settings.webhooks_empty")}
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
              style="border-color: var(--border-color); color: var(--text-secondary)">{tr($locale, "common.test")}</button>
            <button onclick={() => handleDelete(wh.id)}
              class="px-2.5 py-1.5 rounded-lg text-xs"
              style="color: var(--neon-accent); border: 1px solid color-mix(in srgb, var(--neon-accent) 20%, transparent)">{tr($locale, "common.delete")}</button>
          </div>
        </div>
      {/each}
    </div>
  {/if}
</section>
