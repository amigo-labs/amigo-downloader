<script lang="ts">
  import { onMount } from "svelte";
  import { addToast } from "../lib/toast";

  let { onclose, prefill }: {
    onclose: () => void;
    prefill?: { error_context?: { download_id?: string; error_message?: string } };
  } = $props();

  let systemInfo = $state<any>(null);
  let autoReported = $state(false);
  let resultUrl = $state("");

  const repo = "amigo-labs/amigo-downloader";

  onMount(async () => {
    try {
      const res = await fetch("/api/v1/system-info");
      systemInfo = await res.json();
    } catch { /* offline */ }

    // Auto-report crash if error_context and token configured
    if (prefill?.error_context && systemInfo?.feedback_enabled) {
      autoReportCrash();
    }
  });

  async function autoReportCrash() {
    if (!prefill?.error_context) return;
    try {
      const res = await fetch("/api/v1/feedback", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          type: "crash",
          title: prefill.error_context.error_message || "Download failed",
          description: "Automatically reported crash.",
          include_system_info: true,
          error_context: prefill.error_context,
        }),
      });
      if (res.ok) {
        const data = await res.json();
        autoReported = true;
        resultUrl = data.issue_url;
        if (data.deduplicated) {
          addToast("info", `Known issue #${data.issue_number}`, "Already reported");
        } else {
          addToast("info", `Crash reported as #${data.issue_number}`);
        }
      }
    } catch { /* silent */ }
  }

  function bugUrl() {
    let body = "## What happened?\n\nDescribe the bug...\n\n## Steps to reproduce\n\n1. \n2. \n3. \n\n## Expected behavior\n\n";
    if (systemInfo) {
      body += `\n## System Info\n\n- Version: ${systemInfo.version}\n- OS: ${systemInfo.os} (${systemInfo.arch})\n- Plugins: ${systemInfo.plugins_loaded}`;
    }
    return `https://github.com/${repo}/issues/new?` + new URLSearchParams({ title: "[Bug] ", body, labels: "bug" });
  }

  function featureUrl() {
    let body = "## What would you like?\n\nDescribe the feature...\n\n## Why?\n\nWhy is this useful?";
    if (systemInfo) {
      body += `\n\n## System Info\n\n- Version: ${systemInfo.version}`;
    }
    return `https://github.com/${repo}/issues/new?` + new URLSearchParams({ title: "[Feature] ", body, labels: "enhancement" });
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && onclose()} />

<div class="fixed inset-0 z-50 flex items-center justify-center p-4">
  <div class="fixed inset-0 bg-black/60 backdrop-blur-sm" onclick={onclose}></div>

  <div
    class="relative z-10 w-full max-w-sm rounded-2xl shadow-2xl p-6"
    style="background: var(--surface-color); border: 1px solid var(--border-color)"
  >
    <div class="flex items-center justify-between mb-5">
      <h2 class="text-lg font-bold">Feedback</h2>
      <button onclick={onclose} class="p-1 rounded-lg" style="color: var(--text-secondary-color)">
        <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="2">
          <path d="M6 18L18 6M6 6l12 12" />
        </svg>
      </button>
    </div>

    {#if autoReported}
      <div class="rounded-xl p-3 mb-4 text-center" style="background: color-mix(in srgb, var(--color-warning) 10%, transparent)">
        <p class="text-xs font-semibold" style="color: var(--color-warning)">Crash auto-reported</p>
        {#if resultUrl}
          <a href={resultUrl} target="_blank" rel="noopener" class="text-xs underline" style="color: var(--accent-color)">View issue &rarr;</a>
        {/if}
      </div>
    {/if}

    <!-- Report a Bug → GitHub -->
    <a
      href={bugUrl()}
      target="_blank"
      rel="noopener"
      class="flex items-center gap-3 rounded-xl p-4 mb-3 transition-all interactive"
      style="background: var(--surface-2-color); border: 1px solid var(--border-color)"
    >
      <div class="w-10 h-10 rounded-lg flex items-center justify-center text-lg shrink-0" style="background: color-mix(in srgb, var(--color-error) 15%, transparent); color: var(--color-error)">
        &#128027;
      </div>
      <div class="flex-1">
        <p class="font-semibold text-sm">Report a Bug</p>
        <p class="text-xs" style="color: var(--text-secondary-color)">Opens GitHub with pre-filled template</p>
      </div>
      <span style="color: var(--text-secondary-color)">&rarr;</span>
    </a>

    <!-- Request a Feature → GitHub -->
    <a
      href={featureUrl()}
      target="_blank"
      rel="noopener"
      class="flex items-center gap-3 rounded-xl p-4 mb-4 transition-all interactive"
      style="background: var(--surface-2-color); border: 1px solid var(--border-color)"
    >
      <div class="w-10 h-10 rounded-lg flex items-center justify-center text-lg shrink-0" style="background: color-mix(in srgb, var(--color-success) 15%, transparent); color: var(--color-success)">
        &#128161;
      </div>
      <div class="flex-1">
        <p class="font-semibold text-sm">Request a Feature</p>
        <p class="text-xs" style="color: var(--text-secondary-color)">Opens GitHub with pre-filled template</p>
      </div>
      <span style="color: var(--text-secondary-color)">&rarr;</span>
    </a>

    <p class="text-[10px] text-center" style="color: var(--text-secondary-color)">
      {#if systemInfo?.feedback_enabled}
        Crashes are automatically reported.
      {:else}
        Set AMIGO_GITHUB_TOKEN for automatic crash reporting.
      {/if}
    </p>
  </div>
</div>
