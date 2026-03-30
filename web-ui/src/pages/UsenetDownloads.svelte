<script lang="ts">
  import { onMount } from "svelte";
  import { usenetDownloads, layout } from "../lib/stores";
  import {
    getUsenetDownloads,
    uploadNzb,
    pauseDownload,
    resumeDownload,
    deleteDownload,
    getNzbWatchDir,
    setNzbWatchDir,
    formatBytes,
    formatSpeed,
  } from "../lib/api";
  import { addToast } from "../lib/toast";
  import Mascot from "../components/Mascot.svelte";

  let filter = $state<string>("all");
  let dragging = $state(false);
  let watchDir = $state("");
  let watchDirSaving = $state(false);
  let showWatchConfig = $state(false);

  let filtered = $derived(
    $usenetDownloads.filter((d) => {
      if (filter === "all") return true;
      return d.status === filter;
    })
  );

  onMount(async () => {
    try {
      const [downloads, wd] = await Promise.all([
        getUsenetDownloads(),
        getNzbWatchDir(),
      ]);
      usenetDownloads.set(downloads);
      watchDir = wd.path || "";
    } catch {
      // Server offline
    }
  });

  async function handleNzbFile(file: File) {
    if (!file.name.endsWith(".nzb")) {
      addToast("error", "Only .nzb files are supported");
      return;
    }
    try {
      const text = await file.text();
      await uploadNzb(text);
      addToast("success", `NZB imported: ${file.name}`);
      const downloads = await getUsenetDownloads();
      usenetDownloads.set(downloads);
    } catch (e: any) {
      addToast(e.message || "Failed to import NZB", "error");
    }
  }

  function handleDrop(e: DragEvent) {
    e.preventDefault();
    dragging = false;
    const files = e.dataTransfer?.files;
    if (files) {
      for (const file of files) {
        handleNzbFile(file);
      }
    }
  }

  function handleDragOver(e: DragEvent) {
    e.preventDefault();
    dragging = true;
  }

  function handleFileInput(e: Event) {
    const input = e.target as HTMLInputElement;
    const files = input.files;
    if (files) {
      for (const file of files) {
        handleNzbFile(file);
      }
    }
    input.value = "";
  }

  async function handleSaveWatchDir() {
    watchDirSaving = true;
    try {
      await setNzbWatchDir(watchDir.trim());
      addToast("success", watchDir.trim() ? "Watch folder set" : "Watch folder disabled");
    } catch {
      addToast("error", "Failed to save watch folder");
    } finally {
      watchDirSaving = false;
    }
  }

  async function handleAction(id: string, action: string) {
    try {
      if (action === "pause") await pauseDownload(id);
      else if (action === "resume") await resumeDownload(id);
      else if (action === "delete") await deleteDownload(id);
      const downloads = await getUsenetDownloads();
      usenetDownloads.set(downloads);
    } catch {
      addToast("error", "Action failed");
    }
  }
</script>

<div
  class="space-y-4"
  ondrop={handleDrop}
  ondragover={handleDragOver}
  ondragleave={() => (dragging = false)}
  role="application"
>
  <!-- Drop overlay -->
  {#if dragging}
    <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/40 backdrop-blur-sm pointer-events-none">
      <div
        class="rounded-2xl p-10 text-center"
        style="background: var(--surface-color); border: 2px dashed var(--accent-color)"
      >
        <p class="text-lg font-semibold">Drop NZB files here</p>
      </div>
    </div>
  {/if}

  <!-- Top bar: Import + Watch folder -->
  <div class="flex items-center gap-3 flex-wrap">
    <label
      class="px-4 py-2 rounded-lg text-sm font-semibold text-white cursor-pointer"
      style="background: var(--accent-color)"
    >
      Import NZB
      <input type="file" accept=".nzb" multiple class="hidden" onchange={handleFileInput} />
    </label>

    <button
      onclick={() => (showWatchConfig = !showWatchConfig)}
      class="px-3 py-2 rounded-lg text-sm"
      style="background: var(--surface-3-color); color: var(--text-secondary-color)"
    >
      {showWatchConfig ? "Hide" : "Watch Folder"}
    </button>

    <p class="text-xs ml-auto" style="color: var(--text-secondary-color)">
      Drag &amp; drop .nzb files anywhere on this page
    </p>
  </div>

  <!-- Watch folder config -->
  {#if showWatchConfig}
    <div
      class="rounded-xl p-4 flex items-end gap-3"
      style="background: var(--surface-2-color); border: 1px solid var(--border-color)"
    >
      <div class="flex-1">
        <label class="block text-xs font-medium mb-1" style="color: var(--text-secondary-color)">
          Watch Folder Path
        </label>
        <input
          type="text"
          bind:value={watchDir}
          placeholder="/path/to/nzb-watch (leave empty to disable)"
          class="w-full rounded-lg px-3 py-2 text-sm font-mono outline-none"
          style="background: var(--surface-3-color); border: 1px solid var(--border-color); color: var(--text-color)"
        />
        <p class="text-[10px] mt-1" style="color: var(--text-secondary-color)">
          New .nzb files dropped into this folder will be automatically imported.
        </p>
      </div>
      <button
        onclick={handleSaveWatchDir}
        disabled={watchDirSaving}
        class="px-4 py-2 rounded-lg text-sm font-semibold text-white shrink-0 disabled:opacity-50"
        style="background: var(--accent-color)"
      >
        {watchDirSaving ? "Saving..." : "Save"}
      </button>
    </div>
  {/if}

  <!-- Filters -->
  <div class="flex gap-2 flex-wrap">
    {#each ["all", "downloading", "queued", "paused", "completed", "failed"] as f}
      <button
        onclick={() => (filter = f)}
        class="px-3 py-1.5 rounded-lg text-sm font-medium transition-all capitalize"
        style={filter === f
          ? "background: var(--accent-color); color: white"
          : "background: var(--surface-3-color); color: var(--text-secondary-color)"}
      >
        {f}
        {#if f !== "all"}
          <span class="ml-1 opacity-70">
            {$usenetDownloads.filter((d) => d.status === f).length}
          </span>
        {/if}
      </button>
    {/each}
  </div>

  <!-- Download list -->
  {#if filtered.length === 0}
    <div class="flex flex-col items-center justify-center py-20">
      <Mascot size={80} />
      <p class="mt-4 text-sm" style="color: var(--text-secondary-color)">
        No Usenet downloads. Import an NZB file or configure a watch folder.
      </p>
    </div>
  {:else}
    <div class="grid gap-3">
      {#each filtered as download (download.id)}
        <div
          class="rounded-xl p-4"
          style="background: var(--surface-2-color); border: 1px solid var(--border-color)"
        >
          <div class="flex items-start justify-between gap-3">
            <div class="min-w-0 flex-1">
              <p class="font-semibold text-sm truncate">
                {download.filename || "NZB Import"}
              </p>
              <div class="flex gap-3 text-xs mt-1" style="color: var(--text-secondary-color)">
                {#if download.filesize}
                  <span>{formatBytes(download.filesize)}</span>
                {/if}
                <span class="capitalize">{download.status}</span>
              </div>
            </div>

            <div class="flex gap-1.5 shrink-0">
              {#if download.status === "downloading"}
                <button
                  onclick={() => handleAction(download.id, "pause")}
                  class="px-2.5 py-1.5 rounded-lg text-xs"
                  style="background: var(--surface-3-color); color: var(--text-secondary-color)"
                >
                  Pause
                </button>
              {:else if download.status === "paused" || download.status === "queued"}
                <button
                  onclick={() => handleAction(download.id, "resume")}
                  class="px-2.5 py-1.5 rounded-lg text-xs"
                  style="background: var(--surface-3-color); color: var(--text-secondary-color)"
                >
                  Resume
                </button>
              {/if}
              <button
                onclick={() => handleAction(download.id, "delete")}
                class="px-2.5 py-1.5 rounded-lg text-xs text-red-400 hover:bg-red-400/10"
              >
                Delete
              </button>
            </div>
          </div>

          <!-- Progress bar -->
          {#if download.status === "downloading" || download.status === "paused"}
            <div class="mt-3">
              <div
                class="h-1.5 rounded-full overflow-hidden"
                style="background: var(--surface-3-color)"
              >
                <div
                  class="h-full rounded-full transition-all"
                  style="background: var(--accent-color); width: {download.filesize ? (download.bytes_downloaded / download.filesize * 100) : 0}%"
                ></div>
              </div>
              <div class="flex justify-between mt-1 text-[10px]" style="color: var(--text-secondary-color)">
                <span>{formatBytes(download.bytes_downloaded)}{download.filesize ? ` / ${formatBytes(download.filesize)}` : ""}</span>
                {#if download.speed > 0}
                  <span>{formatSpeed(download.speed)}</span>
                {/if}
              </div>
            </div>
          {/if}

          <!-- Error message -->
          {#if download.error}
            <p class="mt-2 text-xs text-red-400 truncate">{download.error}</p>
          {/if}
        </div>
      {/each}
    </div>
  {/if}
</div>
