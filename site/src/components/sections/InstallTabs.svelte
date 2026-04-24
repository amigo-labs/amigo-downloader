<script lang="ts">
  type Tab = {
    id: string;
    label: string;
    hint: string;
    lines: string[];
  };

  const tabs: Tab[] = [
    {
      id: "docker",
      label: "Docker",
      hint: "Recommended for self-hosting",
      lines: [
        "curl -O https://raw.githubusercontent.com/amigo-labs/amigo-downloader/main/docker/docker-compose.yml",
        "docker compose up -d",
      ],
    },
    {
      id: "shell",
      label: "Shell",
      hint: "Linux · macOS",
      lines: [
        "curl -fsSL https://raw.githubusercontent.com/amigo-labs/amigo-downloader/main/scripts/install.sh | bash",
      ],
    },
    {
      id: "cargo",
      label: "Cargo",
      hint: "Build from source",
      lines: [
        "cargo install --git https://github.com/amigo-labs/amigo-downloader amigo-cli amigo-server",
      ],
    },
    {
      id: "tauri",
      label: "Desktop",
      hint: "Tauri · macOS · Windows · Linux",
      lines: ["# Download the latest release:",
        "open https://github.com/amigo-labs/amigo-downloader/releases/latest",
      ],
    },
  ];

  let active = $state(tabs[0].id);
  let copiedId = $state<string | null>(null);
  let copyTimer: ReturnType<typeof setTimeout> | null = null;

  const current = $derived(tabs.find((t) => t.id === active) ?? tabs[0]);

  async function copy(id: string, text: string) {
    try {
      await navigator.clipboard.writeText(text);
      copiedId = id;
      if (copyTimer) clearTimeout(copyTimer);
      copyTimer = setTimeout(() => {
        copiedId = null;
      }, 1600);
    } catch {
      // clipboard unavailable — select text as fallback
    }
  }
</script>

<div class="feature-card" style="background: var(--bg-surface)">
  <div role="tablist" aria-label="Install methods" class="flex flex-wrap gap-1 mb-4">
    {#each tabs as tab}
      <button
        type="button"
        role="tab"
        aria-selected={active === tab.id}
        id={`tab-${tab.id}`}
        aria-controls={`panel-${tab.id}`}
        onclick={() => (active = tab.id)}
        class="px-3 py-1.5 rounded-md text-sm font-medium transition-colors"
        style={
          active === tab.id
            ? "background: color-mix(in srgb, var(--neon-primary) 18%, transparent); color: var(--neon-primary); box-shadow: inset 0 0 0 1px var(--neon-border)"
            : "color: var(--text-secondary)"
        }
      >
        {tab.label}
      </button>
    {/each}
  </div>

  <div
    role="tabpanel"
    id={`panel-${current.id}`}
    aria-labelledby={`tab-${current.id}`}
  >
    <div class="text-xs mb-3" style="color: var(--text-secondary)">
      {current.hint}
    </div>

    <div class="space-y-2">
      {#each current.lines as line}
        {@const isComment = line.startsWith("#")}
        <div
          class="flex items-start gap-2 font-mono text-xs rounded-lg p-3"
          style="background: var(--bg-deep); border: 1px solid var(--border-color); color: {isComment ? 'var(--text-secondary)' : 'var(--text-primary)'}"
        >
          <span aria-hidden="true" style="color: var(--neon-primary)">
            {isComment ? "·" : "$"}
          </span>
          <code class="flex-1 break-all">{line.replace(/^#\s*/, "")}</code>
          {#if !isComment}
            <button
              type="button"
              onclick={() => copy(`${current.id}-${line}`, line)}
              class="icon-btn px-2 py-0.5 rounded text-[11px] font-semibold"
              style={
                copiedId === `${current.id}-${line}`
                  ? "color: var(--neon-success)"
                  : "color: var(--text-secondary)"
              }
              aria-label={`Copy command: ${line}`}
            >
              {copiedId === `${current.id}-${line}` ? "copied" : "copy"}
            </button>
          {/if}
        </div>
      {/each}
    </div>
  </div>
</div>
