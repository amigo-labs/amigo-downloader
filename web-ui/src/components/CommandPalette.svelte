<script lang="ts">
  import { fade } from "svelte/transition";
  import {
    currentPage, openAddPanel, theme, palette, neonIntensity,
    type Page, type ColorPalette,
  } from "../lib/stores";
  import { locale, tr } from "../lib/i18n";
  import { focusTrap } from "../lib/focusTrap";
  import { scaleFade, dur } from "../lib/motion";
  import Icon from "@amigo/ui/components/Icon.svelte";

  let { onclose, onshortcuts }: { onclose: () => void; onshortcuts: () => void } = $props();

  interface Command {
    id: string;
    group: string;
    label: string;
    icon?: string;
    swatch?: string;
    keywords: string;
    run: () => void;
  }

  const palettes: { id: ColorPalette; label: string; color: string }[] = [
    { id: "blue", label: "Blue", color: "#3b82f6" },
    { id: "teal", label: "Teal", color: "#14b8a6" },
    { id: "indigo", label: "Indigo", color: "#6366f1" },
    { id: "amber", label: "Amber", color: "#f59e0b" },
    { id: "violet", label: "Violet", color: "#8b5cf6" },
    { id: "rose", label: "Rose", color: "#f43f5e" },
  ];

  const intensities: { value: number; label: string }[] = [
    { value: 0, label: "Off" },
    { value: 0.25, label: "Low" },
    { value: 0.5, label: "Mid" },
    { value: 0.75, label: "High" },
    { value: 1, label: "Full" },
  ];

  const pages: { id: Page; key: string; icon: string }[] = [
    { id: "downloads", key: "nav.downloads", icon: "arrow-down" },
    { id: "plugins", key: "nav.plugins", icon: "puzzle" },
    { id: "history", key: "nav.history", icon: "clock" },
    { id: "settings", key: "nav.settings", icon: "gear" },
  ];

  function go(action: () => void) {
    action();
    onclose();
  }

  let commands = $derived<Command[]>([
    ...pages.map((p) => ({
      id: `nav-${p.id}`,
      group: tr($locale, "cmd.group_navigate"),
      label: tr($locale, p.key),
      icon: p.icon,
      keywords: `${p.id} ${tr($locale, p.key)}`,
      run: () => go(() => currentPage.set(p.id)),
    })),
    {
      id: "add",
      group: tr($locale, "cmd.group_actions"),
      label: tr($locale, "cmd.add_download"),
      icon: "plus",
      keywords: "add download new url link",
      run: () => go(() => openAddPanel()),
    },
    {
      id: "shortcuts",
      group: tr($locale, "cmd.group_actions"),
      label: tr($locale, "cmd.show_shortcuts"),
      icon: "info",
      keywords: "keyboard shortcuts help keys",
      run: () => go(() => onshortcuts()),
    },
    {
      id: "theme",
      group: tr($locale, "cmd.group_appearance"),
      label: tr($locale, "cmd.toggle_theme"),
      icon: "sun",
      keywords: "theme dark light mode appearance",
      run: () => go(() => theme.toggle()),
    },
    ...palettes.map((p) => ({
      id: `palette-${p.id}`,
      group: tr($locale, "cmd.group_appearance"),
      label: tr($locale, "cmd.set_palette", { name: p.label }),
      swatch: p.color,
      keywords: `palette color ${p.id} ${p.label} accent`,
      run: () => go(() => palette.set(p.id)),
    })),
    ...intensities.map((it) => ({
      id: `intensity-${it.value}`,
      group: tr($locale, "cmd.group_appearance"),
      label: tr($locale, "cmd.set_intensity", { name: it.label }),
      icon: "bolt",
      keywords: `neon intensity glow ${it.label}`,
      run: () => go(() => neonIntensity.set(it.value)),
    })),
  ]);

  let query = $state("");
  let selected = $state(0);

  // Subsequence fuzzy match: every char of the query appears in order.
  function matches(cmd: Command, q: string): boolean {
    if (!q) return true;
    const hay = `${cmd.label} ${cmd.keywords}`.toLowerCase();
    let i = 0;
    for (const ch of q.toLowerCase()) {
      i = hay.indexOf(ch, i);
      if (i === -1) return false;
      i++;
    }
    return true;
  }

  let results = $derived(commands.filter((c) => matches(c, query.trim())));

  // Group results, preserving group order of first appearance.
  let grouped = $derived.by(() => {
    const order: string[] = [];
    const map = new Map<string, Command[]>();
    for (const c of results) {
      if (!map.has(c.group)) { map.set(c.group, []); order.push(c.group); }
      map.get(c.group)!.push(c);
    }
    return order.map((g) => ({ group: g, items: map.get(g)! }));
  });

  // Flat list mirrors render order so arrow keys land on the right command.
  let flat = $derived(grouped.flatMap((g) => g.items));

  $effect(() => {
    query;
    selected = 0;
  });

  function clampSelect(next: number) {
    const n = flat.length;
    if (n === 0) return;
    selected = ((next % n) + n) % n;
    document.getElementById(`cmd-${selected}`)?.scrollIntoView({ block: "nearest" });
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") { e.preventDefault(); e.stopPropagation(); onclose(); }
    else if (e.key === "ArrowDown") { e.preventDefault(); clampSelect(selected + 1); }
    else if (e.key === "ArrowUp") { e.preventDefault(); clampSelect(selected - 1); }
    else if (e.key === "Enter") { e.preventDefault(); flat[selected]?.run(); }
  }
</script>

<div class="fixed inset-0 z-[110] flex items-start justify-center p-4 pt-[12vh]">
  <button
    class="fixed inset-0 bg-black/60"
    style="backdrop-filter: blur(2px)"
    onclick={onclose}
    aria-label={tr($locale, "common.close")}
    transition:fade={{ duration: dur(150) }}
  ></button>

  <div
    use:focusTrap
    role="dialog"
    aria-modal="true"
    aria-label={tr($locale, "cmd.hint_open")}
    tabindex="-1"
    class="relative z-10 w-full max-w-xl rounded-2xl overflow-hidden neon-card"
    style="background: var(--bg-surface)"
    transition:scaleFade={{ duration: dur(180), y: 8 }}
    onkeydown={onKeydown}
  >
    <!-- Search -->
    <div class="flex items-center gap-3 px-4 py-3 border-b" style="border-color: var(--border-color)">
      <Icon name="search" size={18} />
      <input
        type="text"
        bind:value={query}
        placeholder={tr($locale, "cmd.placeholder")}
        class="flex-1 bg-transparent text-sm outline-none"
        style="color: var(--text-primary)"
        aria-label={tr($locale, "cmd.placeholder")}
        role="combobox"
        aria-expanded="true"
        aria-controls="cmd-list"
      />
      <kbd
        class="px-1.5 py-0.5 rounded text-[10px] font-semibold shrink-0"
        style="font-family: var(--font-mono); background: var(--bg-deep); border: 1px solid var(--border-color); color: var(--text-secondary)"
      >ESC</kbd>
    </div>

    <!-- Results -->
    <div id="cmd-list" role="listbox" class="max-h-[50vh] overflow-y-auto py-2">
      {#if flat.length === 0}
        <p class="px-4 py-8 text-center text-sm" style="color: var(--text-secondary)">
          {tr($locale, "cmd.no_results")}
        </p>
      {:else}
        {#each grouped as section}
          <div class="px-3 pt-2 pb-1 text-[10px] font-semibold uppercase tracking-wider" style="color: var(--text-secondary)">
            {section.group}
          </div>
          {#each section.items as cmd}
            {@const idx = flat.indexOf(cmd)}
            <button
              id="cmd-{idx}"
              role="option"
              aria-selected={selected === idx}
              onclick={cmd.run}
              onmousemove={() => (selected = idx)}
              class="w-full flex items-center gap-3 px-3 py-2 mx-1 rounded-lg text-left text-sm"
              style={selected === idx
                ? "background: color-mix(in srgb, var(--neon-primary) 14%, transparent); color: var(--neon-primary)"
                : "color: var(--text-primary)"}
            >
              {#if cmd.swatch}
                <span class="w-4 h-4 rounded-full shrink-0" style="background: {cmd.swatch}"></span>
              {:else if cmd.icon}
                <Icon name={cmd.icon} size={16} />
              {/if}
              <span class="flex-1 truncate">{cmd.label}</span>
              {#if selected === idx}
                <kbd class="text-[10px] opacity-60" style="font-family: var(--font-mono)">↵</kbd>
              {/if}
            </button>
          {/each}
        {/each}
      {/if}
    </div>
  </div>
</div>
