<script lang="ts">
  import { addDownload, addBatch } from "../lib/api";
  import { closeSidePanel } from "../lib/stores";
  import { addToast } from "../lib/toast";
  import { locale, tr } from "../lib/i18n";
  import Icon from "@amigo/ui/components/Icon.svelte";

  let urlInput = $state("");
  let loading = $state(false);
  let activeTab = $state<"url" | "file">("url");

  const URL_RE = /^(https?|magnet|ftp):/i;

  let lines = $derived(urlInput.split("\n").map((u) => u.trim()).filter((u) => u.length > 0));
  let validLinks = $derived(lines.filter((u) => URL_RE.test(u)));
  let invalidCount = $derived(lines.length - validLinks.length);
  let canSubmit = $derived(validLinks.length > 0 && !loading);

  async function handleSubmit() {
    if (validLinks.length === 0) return;
    loading = true;
    try {
      if (validLinks.length === 1) {
        await addDownload(validLinks[0]);
        addToast("success", tr($locale, "add.added"));
      } else {
        await addBatch(validLinks);
        addToast("success", tr($locale, "add.added_many", { count: validLinks.length }));
      }
      closeSidePanel();
    } catch (e: any) {
      addToast("error", tr($locale, "add.failed"), e?.message);
    } finally {
      loading = false;
    }
  }

  async function handleFileUpload(e: Event) {
    const input = e.target as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) return;
    loading = true;
    try {
      const text = await file.text();
      const ext = file.name.split(".").pop()?.toLowerCase();
      if (ext === "nzb") {
        const { uploadNzb } = await import("../lib/api");
        await uploadNzb(text);
      } else if (ext === "dlc") {
        const { importDlc } = await import("../lib/api");
        await importDlc(file);
      } else {
        urlInput = text;
        activeTab = "url";
        loading = false;
        return;
      }
      addToast("success", tr($locale, "add.file_imported"));
      closeSidePanel();
    } catch (e: any) {
      addToast("error", tr($locale, "add.file_failed"), e?.message);
    } finally {
      loading = false;
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Enter" && e.ctrlKey) handleSubmit();
  }
</script>

<div class="p-4 space-y-4">
  <!-- Tabs -->
  <div class="flex gap-1 p-1 rounded-lg" style="background: var(--bg-surface-2)" role="tablist">
    <button
      role="tab"
      aria-selected={activeTab === "url"}
      onclick={() => (activeTab = "url")}
      class="flex-1 py-2 rounded-md text-sm font-medium transition-colors"
      style={activeTab === "url"
        ? "background: var(--bg-surface); color: var(--neon-primary)"
        : "color: var(--text-secondary)"}
    >
      {tr($locale, "add.tab_url")}
    </button>
    <button
      role="tab"
      aria-selected={activeTab === "file"}
      onclick={() => (activeTab = "file")}
      class="flex-1 py-2 rounded-md text-sm font-medium transition-colors"
      style={activeTab === "file"
        ? "background: var(--bg-surface); color: var(--neon-primary)"
        : "color: var(--text-secondary)"}
    >
      {tr($locale, "add.tab_file")}
    </button>
  </div>

  {#if activeTab === "url"}
    <textarea
      bind:value={urlInput}
      onkeydown={handleKeydown}
      placeholder={tr($locale, "add.placeholder")}
      rows="6"
      class="w-full rounded-lg px-4 py-3 text-sm resize-none"
      style="font-family: var(--font-mono);background: var(--bg-surface-2); border: 1px solid var(--border-color); color: var(--text-primary)"
    ></textarea>

    <!-- Live link detection / validation -->
    <div class="flex items-center justify-between text-xs min-h-[1rem]">
      <span style="color: var(--text-secondary)">{tr($locale, "add.hint")}</span>
      {#if validLinks.length > 0}
        <span class="font-semibold tabular-nums" style="color: var(--neon-primary)">
          {validLinks.length === 1
            ? tr($locale, "add.links_one")
            : tr($locale, "add.links_many", { count: validLinks.length })}
        </span>
      {/if}
    </div>
    {#if invalidCount > 0}
      <p class="text-xs" style="color: var(--neon-warning)">
        {tr($locale, "add.links_invalid", { count: invalidCount })}
      </p>
    {/if}

    <button
      onclick={handleSubmit}
      disabled={!canSubmit}
      class="action-btn w-full py-2.5 rounded-lg font-semibold text-sm disabled:opacity-40"
      style="background: var(--neon-primary); color: var(--bg-deep)"
    >
      {loading ? tr($locale, "common.adding") : tr($locale, "add.submit")}
    </button>
  {:else}
    <label
      class="flex flex-col items-center justify-center border-2 border-dashed rounded-xl py-10 cursor-pointer transition-colors"
      style="border-color: var(--border-color)"
    >
      <Icon name="upload" size={24} />
      <p class="text-sm font-medium mt-3" style="color: var(--text-primary)">{tr($locale, "add.drop_hint")}</p>
      <p class="text-xs mt-1" style="color: var(--text-secondary)">{tr($locale, "add.file_types")}</p>
      <input type="file" accept=".nzb,.dlc,.txt" class="hidden" onchange={handleFileUpload} />
    </label>
  {/if}
</div>
