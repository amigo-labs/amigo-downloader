<script lang="ts">
  import { addDownload, addBatch } from "../lib/api";

  let { onclose }: { onclose: () => void } = $props();
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
      onclose();
    } catch (e) {
      console.error("Failed to add download:", e);
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
        // Treat as text file with URLs
        urlInput = text;
        activeTab = "url";
        loading = false;
        return;
      }
      onclose();
    } catch (e) {
      console.error("Failed to upload:", e);
    } finally {
      loading = false;
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") onclose();
    if (e.key === "Enter" && e.ctrlKey) handleSubmit();
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<!-- Backdrop -->
<div class="fixed inset-0 z-50 flex items-center justify-center p-4">
  <div class="fixed inset-0 bg-black/60 backdrop-blur-sm" onclick={onclose}></div>

  <!-- Dialog -->
  <div
    class="relative z-10 w-full max-w-lg rounded-2xl shadow-2xl p-6"
    style="background: var(--surface-color); border: 1px solid var(--border-color)"
  >
    <div class="flex items-center justify-between mb-5">
      <h2 class="text-lg font-bold">Add Download</h2>
      <button
        onclick={onclose}
        class="p-1 rounded-lg transition-colors"
        style="color: var(--text-secondary-color)"
      >
        <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="2">
          <path d="M6 18L18 6M6 6l12 12" />
        </svg>
      </button>
    </div>

    <!-- Tabs -->
    <div class="flex gap-1 mb-4 p-1 rounded-lg" style="background: var(--surface-3-color)">
      <button
        onclick={() => (activeTab = "url")}
        class="flex-1 py-2 rounded-md text-sm font-medium transition-all"
        style={activeTab === "url"
          ? "background: var(--surface-color); box-shadow: 0 1px 3px rgba(0,0,0,.1)"
          : "color: var(--text-secondary-color)"}
      >
        URL / Links
      </button>
      <button
        onclick={() => (activeTab = "file")}
        class="flex-1 py-2 rounded-md text-sm font-medium transition-all"
        style={activeTab === "file"
          ? "background: var(--surface-color); box-shadow: 0 1px 3px rgba(0,0,0,.1)"
          : "color: var(--text-secondary-color)"}
      >
        File (NZB / DLC)
      </button>
    </div>

    {#if activeTab === "url"}
      <textarea
        bind:value={urlInput}
        placeholder="Paste URL(s) here — one per line"
        rows="5"
        class="w-full rounded-lg px-4 py-3 text-sm font-mono resize-none outline-none transition-all focus:ring-2"
        style="background: var(--surface-2-color); border: 1px solid var(--border-color); color: var(--text-color); --tw-ring-color: var(--accent-color)"
      ></textarea>
      <p class="text-xs mt-1.5" style="color: var(--text-secondary-color)">
        Ctrl+Enter to submit. Multiple URLs supported (one per line).
      </p>
      <button
        onclick={handleSubmit}
        disabled={loading || !urlInput.trim()}
        class="mt-4 w-full py-2.5 rounded-lg font-semibold text-white text-sm transition-all disabled:opacity-50 hover:brightness-110"
        style="background: var(--accent-color)"
      >
        {loading ? "Adding..." : "Add Download"}
      </button>
    {:else}
      <label
        class="flex flex-col items-center justify-center border-2 border-dashed rounded-xl py-10 cursor-pointer transition-all hover:border-opacity-80"
        style="border-color: var(--border-color)"
      >
        <div class="pixel-logo text-3xl mb-3" style="font-family: 'Press Start 2P'; color: var(--accent-color)">^</div>
        <p class="text-sm font-medium mb-1">Drop file here or click to browse</p>
        <p class="text-xs" style="color: var(--text-secondary-color)">NZB, DLC, or text file with URLs</p>
        <input type="file" accept=".nzb,.dlc,.txt" class="hidden" onchange={handleFileUpload} />
      </label>
    {/if}
  </div>
</div>
