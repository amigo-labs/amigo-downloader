<script lang="ts">
  import { addDownload, addBatch } from "../lib/api";
  import { closeSidePanel } from "../lib/stores";
  import { addToast } from "../lib/toast";
  import Icon from "@amigo/ui/components/Icon.svelte";

  let urlInput = $state("");
  let loading = $state(false);
  let activeTab = $state<"url" | "file">("url");

  async function handleSubmit() {
    if (!urlInput.trim()) return;
    loading = true;
    const urls = urlInput.split("\n").map((u) => u.trim()).filter((u) => u.length > 0);
    try {
      if (urls.length === 1) {
        await addDownload(urls[0]);
      } else {
        await addBatch(urls);
      }
      addToast("success", "Download added");
      closeSidePanel();
    } catch (e: any) {
      addToast("error", "Failed to add download", e?.message);
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
      addToast("success", "File imported");
      closeSidePanel();
    } catch (e: any) {
      addToast("error", "Failed to import file", e?.message);
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
  <div class="flex gap-1 p-1 rounded-lg" style="background: var(--bg-surface-2)">
    <button
      onclick={() => (activeTab = "url")}
      class="flex-1 py-2 rounded-md text-sm font-medium transition-colors"
      style={activeTab === "url"
        ? "background: var(--bg-surface); color: var(--neon-primary)"
        : "color: var(--text-secondary)"}
    >
      URL / Links
    </button>
    <button
      onclick={() => (activeTab = "file")}
      class="flex-1 py-2 rounded-md text-sm font-medium transition-colors"
      style={activeTab === "file"
        ? "background: var(--bg-surface); color: var(--neon-primary)"
        : "color: var(--text-secondary)"}
    >
      File (NZB / DLC)
    </button>
  </div>

  {#if activeTab === "url"}
    <textarea
      bind:value={urlInput}
      onkeydown={handleKeydown}
      placeholder="Paste URL(s) here — one per line"
      rows="6"
      class="w-full rounded-lg px-4 py-3 text-sm resize-none"
      style="font-family: var(--font-mono);background: var(--bg-surface-2); border: 1px solid var(--border-color); color: var(--text-primary)"
    ></textarea>
    <p class="text-xs" style="color: var(--text-secondary)">
      Ctrl+Enter to submit. Multiple URLs supported (one per line).
    </p>
    <button
      onclick={handleSubmit}
      disabled={loading || !urlInput.trim()}
      class="w-full py-2.5 rounded-lg font-semibold text-sm transition-colors disabled:opacity-40"
      style="background: var(--neon-primary); color: var(--bg-deep)"
    >
      {loading ? "Adding..." : "Add Download"}
    </button>
  {:else}
    <label
      class="flex flex-col items-center justify-center border-2 border-dashed rounded-xl py-10 cursor-pointer transition-colors"
      style="border-color: var(--border-color)"
    >
      <Icon name="upload" size={24} />
      <p class="text-sm font-medium mt-3" style="color: var(--text-primary)">Drop file here or click to browse</p>
      <p class="text-xs mt-1" style="color: var(--text-secondary)">NZB, DLC, or text file with URLs</p>
      <input type="file" accept=".nzb,.dlc,.txt" class="hidden" onchange={handleFileUpload} />
    </label>
  {/if}
</div>
