<script lang="ts">
  import { onMount } from "svelte";
  import { addToast } from "../lib/toast";
  import { crashReport } from "../lib/stores";
  import { locale, tr } from "../lib/i18n";
  import { openLayer, layerState } from "../lib/overlays.svelte";
  import Dialog from "@amigo/ui/components/Dialog.svelte";
  import Banner from "@amigo/ui/components/Banner.svelte";
  import Icon from "@amigo/ui/components/Icon.svelte";

  let { onclose }: { onclose: () => void } = $props();

  interface SystemInfo {
    version: string;
    os: string;
    arch: string;
    plugins_loaded: number;
    feedback_enabled: boolean;
  }

  let systemInfo = $state<SystemInfo | null>(null);
  // Tri-state: the footer must not claim auto-reporting is off while the
  // system-info request is still in flight.
  let infoState = $state<"loading" | "ready" | "failed">("loading");
  let autoReported = $state(false);
  let reporting = $state(false);
  let reportFailed = $state(false);
  let resultUrl = $state("");

  const layer = layerState("feedback");
  $effect(() =>
    openLayer({ id: "feedback", kind: "modal", label: "Feedback", onDismiss: onclose }),
  );

  const repo = "amigo-labs/amigo-downloader";

  onMount(async () => {
    try {
      const res = await fetch("/api/v1/system-info", { credentials: "same-origin" });
      if (res.ok) {
        systemInfo = await res.json();
        infoState = "ready";
      } else {
        infoState = "failed";
      }
    } catch (e) {
      console.error("Failed to fetch system info:", e);
      infoState = "failed";
    }

    if ($crashReport && systemInfo?.feedback_enabled) {
      autoReportCrash();
    }
  });

  async function autoReportCrash() {
    if (!$crashReport) return;
    reporting = true;
    reportFailed = false;
    try {
      const res = await fetch("/api/v1/feedback", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        credentials: "same-origin",
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
          addToast(
            "info",
            tr($locale, "feedback.known_issue", { number: data.issue_number }),
            tr($locale, "feedback.already_reported"),
          );
        } else {
          addToast(
            "info",
            tr($locale, "feedback.crash_reported_as", { number: data.issue_number }),
          );
        }
      }
      else reportFailed = true;
    } catch (e) {
      console.error("Auto crash report failed:", e);
      reportFailed = true;
    } finally {
      reporting = false;
    }
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

<Dialog
  title={tr($locale, "feedback.title")}
  size="sm"
  isTop={$layer.isTop}
  closeLabel={tr($locale, "common.close")}
  {onclose}
>
  {#if reporting}
    <div class="mb-4">
      <Banner tone="info" role="status">{tr($locale, "feedback.reporting")}</Banner>
    </div>
  {:else if reportFailed}
    <div class="mb-4">
      <Banner tone="danger" role="alert">{tr($locale, "feedback.report_failed")}</Banner>
    </div>
  {:else if autoReported}
    <div class="mb-4">
      <Banner tone="warning" role="status" title={tr($locale, "feedback.crash_reported")}>
        {#if resultUrl}
          <a href={resultUrl} target="_blank" rel="noopener" class="underline" style="color: var(--accent-ink)">
            {tr($locale, "feedback.view_issue")} &rarr;
          </a>
        {/if}
      </Banner>
    </div>
  {/if}

  <div class="grid gap-3">
    <a
      href={bugUrl()}
      target="_blank"
      rel="noopener"
      class="link-card flex items-center gap-3 rounded-xl p-4"
      style="background: var(--bg-surface-2); border: 1px solid var(--border-color)"
    >
      <span
        class="w-10 h-10 rounded-lg flex items-center justify-center shrink-0"
        style="background: color-mix(in srgb, var(--danger) 10%, transparent); color: var(--danger-ink)"
      >
        <Icon name="flag" size={20} />
      </span>
      <span class="flex-1">
        <span class="block font-semibold text-sm" style="color: var(--text-primary)">{tr($locale, "feedback.report_bug")}</span>
        <span class="block text-xs" style="color: var(--text-secondary)">{tr($locale, "feedback.opens_github")}</span>
      </span>
      <Icon name="external" size={14} />
    </a>

    <a
      href={featureUrl()}
      target="_blank"
      rel="noopener"
      class="link-card flex items-center gap-3 rounded-xl p-4"
      style="background: var(--bg-surface-2); border: 1px solid var(--border-color)"
    >
      <span
        class="w-10 h-10 rounded-lg flex items-center justify-center shrink-0"
        style="background: color-mix(in srgb, var(--success) 10%, transparent); color: var(--success-ink)"
      >
        <Icon name="plus" size={20} />
      </span>
      <span class="flex-1">
        <span class="block font-semibold text-sm" style="color: var(--text-primary)">{tr($locale, "feedback.request_feature")}</span>
        <span class="block text-xs" style="color: var(--text-secondary)">{tr($locale, "feedback.opens_github")}</span>
      </span>
      <Icon name="external" size={14} />
    </a>
  </div>

  {#snippet footer()}
    <p class="text-xs w-full text-center m-0" style="color: var(--text-secondary)">
      {#if infoState === "loading"}
        {tr($locale, "feedback.auto_checking")}
      {:else if systemInfo?.feedback_enabled}
        {tr($locale, "feedback.auto_on")}
      {:else}
        {tr($locale, "feedback.auto_off")}
      {/if}
    </p>
  {/snippet}
</Dialog>

<style>
  .link-card {
    color: inherit;
    text-decoration: none;
    transition: border-color var(--dur-fast) var(--ease-out), background-color var(--dur-fast) var(--ease-out);
  }

  .link-card:hover {
    border-color: var(--neon-border-hover);
    background: var(--hover-bg);
  }
</style>
