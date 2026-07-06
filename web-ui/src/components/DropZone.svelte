<script lang="ts">
  import { addDownload, addBatch, importDlc, uploadNzb } from "../lib/api";
  import { addToast } from "../lib/toast";
  import { locale, tr } from "../lib/i18n";

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

    const files = e.dataTransfer?.files;
    if (files && files.length > 0) {
      for (const file of files) {
        const ext = file.name.split(".").pop()?.toLowerCase();
        try {
          if (ext === "dlc") {
            await importDlc(file);
            addToast("success", tr($locale, "drop.dlc_imported"), file.name);
          } else if (ext === "nzb") {
            const text = await file.text();
            await uploadNzb(text);
            addToast("success", tr($locale, "drop.nzb_imported"), file.name);
          } else {
            const text = await file.text();
            const urls = text.split("\n").map((u) => u.trim()).filter((u) => u.startsWith("http"));
            if (urls.length > 0) {
              await addBatch(urls);
              addToast("success", tr($locale, "drop.urls_added", { count: urls.length }), file.name);
            }
          }
        } catch {
          addToast("error", tr($locale, "drop.import_failed"), file.name);
        }
      }
      return;
    }

    const text = e.dataTransfer?.getData("text/plain");
    if (text) {
      const urls = text.split("\n").map((u) => u.trim()).filter((u) => u.startsWith("http"));
      try {
        if (urls.length === 1) {
          await addDownload(urls[0]);
          addToast("success", tr($locale, "add.added"), urls[0]);
        } else if (urls.length > 1) {
          await addBatch(urls);
          addToast("success", tr($locale, "add.added_many", { count: urls.length }));
        }
      } catch {
        addToast("error", tr($locale, "add.failed"));
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
      class="rounded-3xl border-2 border-dashed p-16 flex flex-col items-center gap-4 drop-bounce"
      style="border-color: var(--neon-primary); background: var(--bg-surface)"
    >
      <img src="/amigo-logo.png" alt="" width="64" height="64" class="rounded-lg opacity-60" />
      <p class="text-xl font-bold" style="color: var(--neon-primary)">{tr($locale, "drop.title")}</p>
      <p class="text-sm" style="color: var(--text-secondary)">{tr($locale, "drop.hint")}</p>
    </div>
  </div>
{/if}

<style>
  @keyframes fade-in {
    from { opacity: 0; }
    to { opacity: 1; }
  }

  @keyframes scale-in {
    from { opacity: 0; transform: scale(0.95); }
    to { opacity: 1; transform: scale(1); }
  }

  .drop-overlay {
    background: rgba(0, 0, 0, 0.6);
    animation: fade-in 0.2s ease-out;
  }

  .drop-bounce {
    animation: scale-in 0.3s cubic-bezier(0.16, 1, 0.3, 1);
  }
</style>
