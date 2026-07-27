<script lang="ts">
  import { addDownload, addBatch, importDlc, uploadNzb } from "../lib/api";
  import { addToast } from "../lib/toast";
  import { locale, tr } from "../lib/i18n";
  import { isFileDrag } from "../lib/dnd";

  let dragging = $state(false);
  let dragCounter = $state(0);

  // Only external file drags raise the overlay. Without this check an
  // internal queue-reorder drag put a full-viewport z-index:200 surface over
  // the list, so the row's own drop target never fired.
  function handleDragEnter(e: DragEvent) {
    if (!isFileDrag(e)) return;
    e.preventDefault();
    dragCounter++;
    dragging = true;
  }

  function handleDragLeave(e: DragEvent) {
    if (!dragging) return;
    // `relatedTarget` is null when the pointer leaves the window entirely;
    // any other value is a move between descendants and must not decrement.
    if (e.relatedTarget !== null) return;
    e.preventDefault();
    dragCounter = 0;
    dragging = false;
  }

  function handleDragOver(e: DragEvent) {
    if (!isFileDrag(e)) return;
    e.preventDefault();
  }

  async function handleDrop(e: DragEvent) {
    if (!isFileDrag(e)) return;
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
  <div
    class="fixed inset-0 flex items-center justify-center p-4 drop-overlay"
    style="z-index: var(--z-overlay)"
    aria-hidden="true"
  >
    <div
      class="rounded-3xl border-2 border-dashed p-8 sm:p-16 flex flex-col items-center gap-4 drop-bounce text-center"
      style="border-color: var(--accent); background: var(--bg-surface)"
    >
      <img src="/amigo-logo.png" alt="" width="64" height="64" class="rounded-lg opacity-60" />
      <p class="text-xl font-bold" style="color: var(--accent-ink)">{tr($locale, "drop.title")}</p>
      <p class="text-sm" style="color: var(--text-secondary)">{tr($locale, "drop.hint")}</p>
    </div>
  </div>
{/if}

<style>
  /* Reuses the shared page-enter/card-enter curves and the --ease-out-expo
     token instead of redeclaring the same cubic-bezier locally. */
  @keyframes drop-fade-in {
    from { opacity: 0; }
    to { opacity: 1; }
  }

  @keyframes drop-scale-in {
    from { opacity: 0; transform: scale(0.95); }
    to { opacity: 1; transform: scale(1); }
  }

  .drop-overlay {
    background: rgb(0 0 0 / 60%);
    animation: drop-fade-in var(--dur-base) var(--ease-out);
  }

  .drop-bounce {
    animation: drop-scale-in var(--dur-slow) var(--ease-out-expo);
  }
</style>
