<script lang="ts">
  import { onMount } from "svelte";
  import { addToast } from "../lib/toast";
  import { crashReport } from "../lib/stores";
  import Icon from "./Icon.svelte";

  let { onclose }: { onclose: () => void } = $props();

  let systemInfo = $state<any>(null);
  let autoReported = $state(false);
  let resultUrl = $state("");

  const repo = "amigo-labs/amigo-downloader";

  onMount(async () => {
    try {
      const res = await fetch("/api/v1/system-info");
      systemInfo = await res.json();
    } catch { /* offline */ }

    // Auto-report crash if error_context provided via store (audit M5)
    if ($crashReport && systemInfo?.feedback_enabled) {
      autoReportCrash();
    }
  });

  async function autoReportCrash() {
    if (!$crashReport) return;
    try {
      const res = await fetch("/api/v1/feedback", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          type: "crash",
          title: $crashReport.error_message || "Download failed",
          description: "Automatically reported crash.",
          include_system_info: true,
          error_context: $crashReport,
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

<div class="fixed inset-0 z-50 flex items-center justify-center p-4">
  <div class="fixed inset-0 bg-black/70" onclick={onclose}></div>

  <div
    role="dialog"
    aria-modal="true"
    aria-labelledby="feedback-title"
    class="relative z-10 w-full max-w-sm rounded-2xl shadow-2xl p-6"
    style="background: var(--bg-surface); border: 1px solid var(--border-color)"
  >
    <div class="flex items-center justify-between mb-5">
      <h2 id="feedback-title" class="text-lg font-bold" style="color: var(--text-primary)">Feedback</h2>
      <button
        onclick={onclose}
        class="p-1.5 rounded-lg min-w-[44px] min-h-[44px] flex items-center justify-center"
        style="color: var(--text-secondary)"
        aria-label="Close dialog"
      >
        <Icon name="x" size={18} />
      </button>
    </div>

    {#if autoReported}
      <div class="rounded-xl p-3 mb-4 text-center" style="background: color-mix(in srgb, var(--neon-warning) 8%, transparent)">
        <p class="text-xs font-semibold" style="color: var(--neon-warning)">Crash auto-reported</p>
        {#if resultUrl}
          <a href={resultUrl} target="_blank" rel="noopener" class="text-xs underline" style="color: var(--neon-primary)">View issue &rarr;</a>
        {/if}
      </div>
    {/if}

    <a
      href={bugUrl()}
      target="_blank"
      rel="noopener"
      class="flex items-center gap-3 rounded-xl p-4 mb-3 transition-colors"
      style="background: var(--bg-surface-2); border: 1px solid var(--border-color)"
    >
      <div class="w-10 h-10 rounded-lg flex items-center justify-center shrink-0" style="background: color-mix(in srgb, var(--neon-accent) 8%, transparent); color: var(--neon-accent)">
        <Icon name="flag" size={20} />
      </div>
      <div class="flex-1">
        <p class="font-semibold text-sm" style="color: var(--text-primary)">Report a Bug</p>
        <p class="text-xs" style="color: var(--text-secondary)">Opens GitHub with pre-filled template</p>
      </div>
      <Icon name="external" size={14} class="text-[var(--text-secondary)]" />
    </a>

    <a
      href={featureUrl()}
      target="_blank"
      rel="noopener"
      class="flex items-center gap-3 rounded-xl p-4 mb-4 transition-colors"
      style="background: var(--bg-surface-2); border: 1px solid var(--border-color)"
    >
      <div class="w-10 h-10 rounded-lg flex items-center justify-center shrink-0" style="background: color-mix(in srgb, var(--neon-success) 8%, transparent); color: var(--neon-success)">
        <Icon name="plus" size={20} />
      </div>
      <div class="flex-1">
        <p class="font-semibold text-sm" style="color: var(--text-primary)">Request a Feature</p>
        <p class="text-xs" style="color: var(--text-secondary)">Opens GitHub with pre-filled template</p>
      </div>
      <Icon name="external" size={14} class="text-[var(--text-secondary)]" />
    </a>

    <p class="text-[10px] text-center" style="color: var(--text-secondary)">
      {#if systemInfo?.feedback_enabled}
        Crashes are automatically reported.
      {:else}
        Set AMIGO_GITHUB_TOKEN for automatic crash reporting.
      {/if}
    </p>
  </div>
</div>
