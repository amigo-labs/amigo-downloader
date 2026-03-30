<script lang="ts">
  let { count = 3 }: { count?: number } = $props();
</script>

{#each Array(count) as _, i}
  <div
    class="rounded-xl p-4 animate-pulse"
    style="background: var(--bg-surface); border: 1px solid var(--border-color); --i: {i}"
  >
    <div class="flex items-start justify-between gap-3 mb-3">
      <div class="flex-1">
        <div class="h-4 rounded" style="background: var(--border-color); width: 60%"></div>
        <div class="h-3 rounded mt-2" style="background: var(--border-color); width: 80%"></div>
      </div>
      <div class="h-5 w-20 rounded-full" style="background: var(--border-color)"></div>
    </div>
    <div class="h-1 rounded-full" style="background: var(--border-color)"></div>
  </div>
{/each}
