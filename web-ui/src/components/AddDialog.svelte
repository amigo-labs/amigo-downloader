<script lang="ts">
  import { onMount } from "svelte";
  import { addDownload, addBatch } from "../lib/api";
  import Icon from "./Icon.svelte";

  let { onclose }: { onclose: () => void } = $props();
  let urlInput = $state("");
  let loading = $state(false);
  let activeTab = $state<"url" | "file">("url");
  let dialogEl: HTMLDivElement | undefined = $state();

  // Focus trap (audit C2)
  onMount(() => {
    const focusable = dialogEl?.querySelectorAll<HTMLElement>(
      'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])'
    );
    if (focusable?.length) focusable[0].focus();
  });

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
    if (e.key === "Enter" && e.ctrlKey) handleSubmit();
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="fixed inset-0 z-50 flex items-center justify-center p-4">
  <div class="fixed inset-0 bg-black/70" onclick={onclose}></div>

  <!-- Dialog (audit C2) -->
  <div
    bind:this={dialogEl}
    role="dialog"
    aria-modal="true"
    aria-labelledby="add-dialog-title"
    class="relative z-10 w-full max-w-lg rounded-2xl shadow-2xl p-6"
    style="background: var(--bg-surface); border: 1px solid var(--border-color)"
  >
    <div class="flex items-center justify-between mb-5">
      <h2 id="add-dialog-title" class="text-lg font-bold" style="color: var(--text-primary)">Add Download</h2>
      <button
        onclick={onclose}
        class="p-1.5 rounded-lg min-w-[44px] min-h-[44px] flex items-center justify-center"
        style="color: var(--text-secondary)"
        aria-label="Close dialog"
      >
        <Icon name="x" size={18} />
      </button>
    </div>

    <!-- Tabs -->
    <div class="flex gap-1 mb-4 p-1 rounded-lg" style="background: var(--bg-surface-2)">
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
        placeholder="Paste URL(s) here — one per line"
        rows="5"
        class="w-full rounded-lg px-4 py-3 text-sm resize-none"
        style="font-family: var(--font-mono);background: var(--bg-surface-2); border: 1px solid var(--border-color); color: var(--text-primary)"
      ></textarea>
      <p class="text-xs mt-1.5" style="color: var(--text-secondary)">
        Ctrl+Enter to submit. Multiple URLs supported (one per line).
      </p>
      <button
        onclick={handleSubmit}
        disabled={loading || !urlInput.trim()}
        class="mt-4 w-full py-2.5 rounded-lg font-semibold text-sm transition-colors disabled:opacity-40"
        style="background: var(--neon-primary); color: var(--bg-deep)"
      >
        {loading ? "Adding..." : "Add Download"}
      </button>
    {:else}
      <label
        class="flex flex-col items-center justify-center border-2 border-dashed rounded-xl py-10 cursor-pointer transition-colors"
        style="border-color: var(--border-color)"
      >
        <img src="/amigo-logo.png" alt="" width="40" height="40" class="rounded opacity-40 mb-3" />
        <p class="text-sm font-medium" style="color: var(--text-primary)">Drop file here or click to browse</p>
        <p class="text-xs mt-1" style="color: var(--text-secondary)">NZB, DLC, or text file with URLs</p>
        <input type="file" accept=".nzb,.dlc,.txt" class="hidden" onchange={handleFileUpload} />
      </label>
    {/if}
  </div>
</div>
