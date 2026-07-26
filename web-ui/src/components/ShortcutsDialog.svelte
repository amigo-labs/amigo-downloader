<script lang="ts">
  import { locale, tr } from "../lib/i18n";
  import { bindings, formatChord } from "../lib/keymap";
  import { openLayer, layerState } from "../lib/overlays.svelte";
  import Dialog from "@amigo/ui/components/Dialog.svelte";

  let { onclose }: { onclose: () => void } = $props();

  const layer = layerState("shortcuts");
  $effect(() =>
    openLayer({ id: "shortcuts", kind: "modal", label: "Shortcuts", onDismiss: onclose }),
  );

  // Rendered from the live keymap registry rather than a hand-written list.
  // The old hard-coded array listed 5 of the app's shortcuts and had already
  // drifted -- it omitted Ctrl+Enter, the palette keys and the menu keys.
  // Several bindings share a description on purpose (the four digit keys are
  // one "switch page" shortcut), so collapse by description and show the
  // range rather than four identical rows.
  let rows = $derived.by(() => {
    const byDesc = new Map<string, string[]>();
    for (const b of $bindings) {
      const desc = tr($locale, b.descKey);
      byDesc.set(desc, [...(byDesc.get(desc) ?? []), formatChord(b.chord)]);
    }
    const out = [...byDesc].map(([desc, keys]) => ({
      desc,
      keys: keys.length > 2 ? `${keys[0]} – ${keys[keys.length - 1]}` : keys.join(" / "),
    }));
    out.push({ keys: "Esc", desc: tr($locale, "shortcuts.close") });
    return out;
  });
</script>

<Dialog
  title={tr($locale, "shortcuts.title")}
  size="sm"
  isTop={$layer.isTop}
  closeLabel={tr($locale, "common.close")}
  {onclose}
>
  <dl class="grid gap-1">
    {#each rows as row}
      <div class="flex items-center justify-between gap-4 py-1.5">
        <dt class="text-sm" style="color: var(--text-primary)">{row.desc}</dt>
        <dd class="m-0">
          <kbd
            class="px-2 py-0.5 rounded text-xs font-semibold whitespace-nowrap"
            style="font-family: var(--font-mono); background: var(--bg-surface-2);
                   border: 1px solid var(--border-color); color: var(--text-secondary)"
          >{row.keys}</kbd>
        </dd>
      </div>
    {/each}
  </dl>
</Dialog>
