<script lang="ts">
  import { addDownload, addBatch, importDlc, uploadNzb } from "../lib/api";
  import { addToast } from "../lib/toast";
  import Mascot from "./Mascot.svelte";

  let dragging = $state(false);
  let dragCounter = $state(0);

  function handleDragEnter(e: DragEvent) {
    e.preventDefault();
    dragCounter++;
    dragging = true;
  }

  function handleDragLeave(e: DragEvent) {
    e.preventDefault();
    dragCounter--;
    if (dragCounter <= 0) {
      dragging = false;
      dragCounter = 0;
    }
  }

  function handleDragOver(e: DragEvent) {
    e.preventDefault();
  }

  async function handleDrop(e: DragEvent) {
    e.preventDefault();
    dragging = false;
    dragCounter = 0;

    // Check for files
    const files = e.dataTransfer?.files;
    if (files && files.length > 0) {
      for (const file of files) {
        const ext = file.name.split(".").pop()?.toLowerCase();
        try {
          if (ext === "dlc") {
            await importDlc(file);
            addToast("success", "DLC imported", file.name);
          } else if (ext === "nzb") {
            const text = await file.text();
            await uploadNzb(text);
            addToast("success", "NZB imported", file.name);
          } else {
            // Treat as text with URLs
            const text = await file.text();
            const urls = text.split("\n").map((u) => u.trim()).filter((u) => u.startsWith("http"));
            if (urls.length > 0) {
              await addBatch(urls);
              addToast("success", `Added ${urls.length} URLs`, file.name);
            }
          }
        } catch {
          addToast("error", "Import failed", file.name);
        }
      }
      return;
    }

    // Check for dropped text/URLs
    const text = e.dataTransfer?.getData("text/plain");
    if (text) {
      const urls = text.split("\n").map((u) => u.trim()).filter((u) => u.startsWith("http"));
      if (urls.length === 1) {
        await addDownload(urls[0]);
        addToast("success", "Download added", urls[0]);
      } else if (urls.length > 1) {
        await addBatch(urls);
        addToast("success", `Added ${urls.length} downloads`);
      }
    }
  }
</script>

<svelte:window
  ondragenter={handleDragEnter}
  ondragleave={handleDragLeave}
  ondragover={handleDragOver}
  ondrop={handleDrop}
/>

{#if dragging}
  <div class="fixed inset-0 z-[200] flex items-center justify-center drop-overlay">
    <div
      class="rounded-3xl border-4 border-dashed p-16 flex flex-col items-center gap-4 drop-bounce"
      style="border-color: var(--accent-color); background: color-mix(in srgb, var(--surface-color) 95%, transparent)"
    >
      <Mascot size={96} animate={true} />
      <p class="text-xl font-bold" style="color: var(--accent-color)">Drop it like it's hot!</p>
      <p class="text-sm" style="color: var(--text-secondary-color)">URLs, NZB, DLC, or text files</p>
    </div>
  </div>
{/if}

<style>
  @keyframes fade-in {
    from { opacity: 0; }
    to { opacity: 1; }
  }

  @keyframes scale-in {
    from { opacity: 0; transform: scale(0.9); }
    to { opacity: 1; transform: scale(1); }
  }

  .drop-overlay {
    backdrop-filter: blur(8px);
    background: rgba(0, 0, 0, 0.4);
    animation: fade-in 0.2s ease-out;
  }

  .drop-bounce {
    animation: scale-in 0.3s cubic-bezier(0.16, 1, 0.3, 1);
  }
</style>
