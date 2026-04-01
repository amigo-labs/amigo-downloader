<script lang="ts">
  import { onMount } from "svelte";
  import { getPlugins, checkUpdates, applyCoreUpdate } from "../lib/api";
  import { addToast } from "../lib/toast";
  import SkeletonCard from "../components/SkeletonCard.svelte";

  let plugins = $state<any[]>([]);
  let updateInfo = $state<any>(null);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let updating = $state(false);

  onMount(async () => {
    try {
      plugins = await getPlugins();
    } catch {
      error = "Failed to load plugins. Is the server running?";
    }
    try {
      updateInfo = await checkUpdates();
    } catch {
      // Update check is optional — don't block UI
    }
    loading = false;
  });

  async function handleCoreUpdate() {
    updating = true;
    try {
      await applyCoreUpdate();
      addToast("success", "Update initiated — restart required.");
    } catch (e) {
      addToast("error", "Update failed", e instanceof Error ? e.message : "Unknown error");
    } finally {
      updating = false;
    }
  }
</script>

<div class="space-y-6">
  <!-- Update banner -->
  {#if updateInfo?.core?.update_available}
    <div
      class="rounded-xl p-4 flex items-center justify-between"
      style="background: color-mix(in srgb, var(--neon-primary) 6%, transparent); border: 1px solid color-mix(in srgb, var(--neon-primary) 15%, transparent)"
    >
      <div>
        <p class="font-semibold text-sm" style="color: var(--text-primary)">Core Update Available</p>
        <p class="text-xs" style="font-family: var(--font-mono);color: var(--text-secondary)">
          v{updateInfo.core.current_version} &rarr; v{updateInfo.core.latest_version}
        </p>
      </div>
      <button
        onclick={handleCoreUpdate}
        disabled={updating}
        class="px-4 py-2 rounded-lg text-sm font-semibold disabled:opacity-50"
        style="background: var(--neon-primary); color: var(--bg-deep)"
      >
        {updating ? "Updating…" : "Update"}
      </button>
    </div>
  {/if}

  <!-- Installed Plugins -->
  <section>
    <h3 class="text-lg font-bold mb-4" style="color: var(--text-primary)">Installed Plugins</h3>
    {#if loading}
      <div class="grid gap-3 sm:grid-cols-2">
        <SkeletonCard count={2} />
      </div>
    {:else if error}
      <div class="rounded-xl p-4" style="background: color-mix(in srgb, var(--status-error, #ef4444) 8%, transparent); border: 1px solid color-mix(in srgb, var(--status-error, #ef4444) 20%, transparent)">
        <p class="text-sm" style="color: var(--status-error, #ef4444)">{error}</p>
      </div>
    {:else if plugins.length === 0}
      <p class="text-sm" style="color: var(--text-secondary)">No plugins loaded.</p>
    {:else}
      <div class="grid gap-3 sm:grid-cols-2">
        {#each plugins as plugin}
          <div
            class="rounded-xl p-4"
            style="background: var(--bg-surface); border: 1px solid var(--border-color)"
          >
            <div class="flex items-start justify-between">
              <div>
                <h4 class="font-semibold text-sm" style="color: var(--text-primary)">{plugin.name}</h4>
                <p class="text-xs" style="font-family: var(--font-mono);color: var(--text-secondary)">
                  v{plugin.version}
                </p>
              </div>
              <span
                class="px-2 py-0.5 rounded-full text-[10px] font-semibold"
                style={plugin.enabled
                  ? "background: color-mix(in srgb, var(--neon-success) 10%, transparent); color: var(--neon-success)"
                  : "background: var(--bg-surface-2); color: var(--text-secondary)"}
              >
                {plugin.enabled ? "Active" : "Disabled"}
              </span>
            </div>
            <p class="text-xs mt-2 truncate" style="font-family: var(--font-mono);color: var(--text-secondary)">
              {plugin.url_pattern}
            </p>
          </div>
        {/each}
      </div>
    {/if}
  </section>

  <!-- Marketplace -->
  <section>
    <h3 class="text-lg font-bold mb-4" style="color: var(--text-primary)">Plugin Marketplace</h3>
    <div
      class="rounded-xl p-8 flex flex-col items-center justify-center"
      style="background: var(--bg-surface); border: 1px dashed var(--border-color)"
    >
      <img src="/amigo-logo.png" alt="" width="40" height="40" class="rounded-lg opacity-30 mb-3" />
      <p class="text-sm" style="color: var(--text-secondary)">
        Plugin marketplace coming soon.
      </p>
    </div>
  </section>
</div>
