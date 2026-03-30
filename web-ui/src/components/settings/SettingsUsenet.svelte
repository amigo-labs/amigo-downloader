<script lang="ts">
  import type { AppConfig } from "../../lib/api";

  let { config, onsave }: { config: AppConfig; onsave: () => void } = $props();

  function toggle(key: string) {
    (config.usenet as any)[key] = !(config.usenet as any)[key];
    onsave();
  }
</script>

<section>
  <h3 class="text-lg font-bold mb-4">Usenet Post-Processing</h3>
  <div class="rounded-xl p-5 space-y-4" style="background: var(--surface-2-color); border: 1px solid var(--border-color)">
    {#each [
      { key: "par2_repair", label: "PAR2 Verify & Repair", desc: "Check file integrity and repair damaged files using PAR2 recovery data" },
      { key: "selective_par2", label: "Selective PAR2", desc: "Only download recovery volumes when repair is needed. Saves bandwidth." },
      { key: "auto_unrar", label: "Auto-Extract Archives", desc: "Automatically extract RAR, ZIP, and 7z archives after download" },
      { key: "sequential_postprocess", label: "Sequential Mode (low-power)", desc: "Run PAR2 and extraction one after another. Recommended for Raspberry Pi." },
      { key: "delete_archives_after_extract", label: "Delete Archives After Extract", desc: "Remove archive files after successful extraction" },
      { key: "delete_par2_after_repair", label: "Delete PAR2 After Repair", desc: "Remove PAR2 files after successful verification or repair" },
    ] as opt (opt.key)}
      <div class="flex items-center justify-between">
        <div>
          <p class="text-sm font-semibold">{opt.label}</p>
          <p class="text-xs" style="color: var(--text-secondary-color)">{opt.desc}</p>
        </div>
        <button
          onclick={() => toggle(opt.key)}
          class="w-12 h-6 rounded-full relative transition-colors shrink-0 ml-4"
          style="background: {(config.usenet as any)[opt.key] ? 'var(--accent-color)' : 'var(--surface-3-color)'}"
        >
          <span
            class="absolute top-0.5 w-5 h-5 rounded-full bg-white transition-all shadow"
            style="left: {(config.usenet as any)[opt.key] ? '1.625rem' : '0.125rem'}"
          ></span>
        </button>
      </div>
    {/each}
  </div>
</section>
