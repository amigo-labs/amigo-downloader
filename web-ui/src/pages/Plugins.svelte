<script lang="ts">
  import { onMount } from "svelte";
  import { getPlugins, setPluginEnabled, checkUpdates, applyCoreUpdate } from "../lib/api";
  import { addToast } from "../lib/toast";
  import { locale, tr } from "../lib/i18n";
  import SkeletonCard from "../components/SkeletonCard.svelte";

  let plugins = $state<any[]>([]);
  let updateInfo = $state<any>(null);
  let loading = $state(true);
  let error = $state(false);
  let updating = $state(false);
  // Plugin ids with an in-flight enable/disable request.
  let toggling = $state<Set<string>>(new Set());

  onMount(async () => {
    try {
      plugins = await getPlugins();
    } catch {
      error = true;
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
      addToast("success", tr($locale, "plugins.update_started"));
    } catch (e) {
      addToast("error", tr($locale, "plugins.update_failed"), e instanceof Error ? e.message : undefined);
    } finally {
      updating = false;
    }
  }

  async function handleToggle(plugin: any) {
    if (toggling.has(plugin.id)) return;
    toggling = new Set([...toggling, plugin.id]);
    const enabled = !plugin.enabled;
    try {
      await setPluginEnabled(plugin.id, enabled);
      plugins = plugins.map((p) => (p.id === plugin.id ? { ...p, enabled } : p));
      addToast("info", tr($locale, enabled ? "plugins.enabled_toast" : "plugins.disabled_toast"), plugin.name);
    } catch {
      addToast("error", tr($locale, "plugins.toggle_failed"), plugin.name);
    } finally {
      toggling = new Set([...toggling].filter((id) => id !== plugin.id));
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
        <p class="font-semibold text-sm" style="color: var(--text-primary)">{tr($locale, "plugins.core_update")}</p>
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
        {updating ? tr($locale, "plugins.updating") : tr($locale, "plugins.update")}
      </button>
    </div>
  {/if}

  <!-- Installed Plugins -->
  <section>
    <h3 class="text-lg font-bold mb-4" style="color: var(--text-primary)">{tr($locale, "plugins.installed")}</h3>
    {#if loading}
      <div class="grid gap-3 sm:grid-cols-2">
        <SkeletonCard count={2} />
      </div>
    {:else if error}
      <div class="rounded-xl p-4" style="background: color-mix(in srgb, var(--status-error, #ef4444) 8%, transparent); border: 1px solid color-mix(in srgb, var(--status-error, #ef4444) 20%, transparent)">
        <p class="text-sm" style="color: var(--status-error, #ef4444)">{tr($locale, "plugins.load_failed")}</p>
      </div>
    {:else if plugins.length === 0}
      <p class="text-sm" style="color: var(--text-secondary)">{tr($locale, "plugins.none")}</p>
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
              <button
                onclick={() => handleToggle(plugin)}
                disabled={toggling.has(plugin.id)}
                class="px-2 py-0.5 rounded-full text-[10px] font-semibold cursor-pointer disabled:opacity-50"
                style={plugin.enabled
                  ? "background: color-mix(in srgb, var(--neon-success) 10%, transparent); color: var(--neon-success)"
                  : "background: var(--bg-surface-2); color: var(--text-secondary)"}
                aria-pressed={plugin.enabled}
                aria-label="{plugin.name}: {plugin.enabled ? tr($locale, 'plugins.active') : tr($locale, 'plugins.disabled')}"
              >
                {plugin.enabled ? tr($locale, "plugins.active") : tr($locale, "plugins.disabled")}
              </button>
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
    <h3 class="text-lg font-bold mb-4" style="color: var(--text-primary)">{tr($locale, "plugins.marketplace")}</h3>
    <div
      class="rounded-xl p-8 flex flex-col items-center justify-center"
      style="background: var(--bg-surface); border: 1px dashed var(--border-color)"
    >
      <img src="/amigo-logo.png" alt="" width="40" height="40" class="rounded-lg opacity-30 mb-3" />
      <p class="text-sm" style="color: var(--text-secondary)">
        {tr($locale, "plugins.marketplace_soon")}
      </p>
    </div>
  </section>
</div>
