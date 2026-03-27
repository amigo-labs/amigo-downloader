<script lang="ts">
  import { onMount } from "svelte";
  import { getPlugins, checkUpdates } from "../lib/api";

  let plugins = $state<any[]>([]);
  let updateInfo = $state<any>(null);

  onMount(async () => {
    try {
      plugins = await getPlugins();
    } catch { /* offline */ }

    try {
      updateInfo = await checkUpdates();
    } catch { /* offline */ }
  });
</script>

<div class="space-y-6">
  <!-- Update banner -->
  {#if updateInfo?.core?.update_available}
    <div
      class="rounded-xl p-4 flex items-center justify-between"
      style="background: color-mix(in srgb, var(--accent-color) 10%, transparent); border: 1px solid color-mix(in srgb, var(--accent-color) 30%, transparent)"
    >
      <div>
        <p class="font-semibold text-sm">Core Update Available</p>
        <p class="text-xs" style="color: var(--text-secondary-color)">
          v{updateInfo.core.current_version} &rarr; v{updateInfo.core.latest_version}
        </p>
      </div>
      <button
        class="px-4 py-2 rounded-lg text-sm font-semibold text-white"
        style="background: var(--accent-color)"
      >
        Update
      </button>
    </div>
  {/if}

  <!-- Installed Plugins -->
  <section>
    <h3 class="text-lg font-bold mb-4">Installed Plugins</h3>
    {#if plugins.length === 0}
      <p class="text-sm" style="color: var(--text-secondary-color)">No plugins loaded.</p>
    {:else}
      <div class="grid gap-3 sm:grid-cols-2">
        {#each plugins as plugin}
          <div
            class="rounded-xl p-4 transition-all"
            style="background: var(--surface-2-color); border: 1px solid var(--border-color)"
          >
            <div class="flex items-start justify-between">
              <div>
                <h4 class="font-semibold">{plugin.name}</h4>
                <p class="text-xs font-mono" style="color: var(--text-secondary-color)">
                  v{plugin.version}
                </p>
              </div>
              <span
                class="px-2 py-0.5 rounded-full text-xs font-semibold"
                style={plugin.enabled
                  ? "background: color-mix(in srgb, var(--color-success) 15%, transparent); color: var(--color-success)"
                  : "background: var(--surface-3-color); color: var(--text-secondary-color)"}
              >
                {plugin.enabled ? "Active" : "Disabled"}
              </span>
            </div>
            <p class="text-xs mt-2 font-mono truncate" style="color: var(--text-secondary-color)">
              {plugin.url_pattern}
            </p>
          </div>
        {/each}
      </div>
    {/if}
  </section>

  <!-- Marketplace -->
  <section>
    <h3 class="text-lg font-bold mb-4">Plugin Marketplace</h3>
    <div
      class="rounded-xl p-8 flex flex-col items-center justify-center"
      style="background: var(--surface-2-color); border: 1px dashed var(--border-color)"
    >
      <div class="pixel-logo text-2xl mb-3" style="font-family: 'Press Start 2P'; color: var(--accent-color)">?</div>
      <p class="text-sm" style="color: var(--text-secondary-color)">
        Plugin marketplace coming soon.
      </p>
    </div>
  </section>
</div>
