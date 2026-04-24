<script lang="ts">
  import ChunkViz from "@amigo/ui/components/ChunkViz.svelte";
  import { onMount } from "svelte";

  const commandParts = [
    { dim: false, text: "$ " },
    { dim: false, text: "amigo-dl " },
    { dim: true, text: "https://example.com/ubuntu-24.04-desktop.iso" },
  ];

  let typed = $state("");
  let typingDone = $state(false);
  let progress = $state(0);
  let bytes = $state(0);
  let speedMbps = $state(0);
  let eta = $state("--:--");
  let reducedMotion = false;

  const fullCommand = commandParts.map((p) => p.text).join("");
  const totalMB = 2800;

  onMount(() => {
    reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;

    if (reducedMotion) {
      typed = fullCommand;
      typingDone = true;
      progress = 64;
      bytes = Math.round((progress / 100) * totalMB);
      speedMbps = 112;
      eta = "00:09";
      return;
    }

    let i = 0;
    const typeTimer = setInterval(() => {
      i += 1;
      typed = fullCommand.slice(0, i);
      if (i >= fullCommand.length) {
        clearInterval(typeTimer);
        typingDone = true;
      }
    }, 35);

    return () => clearInterval(typeTimer);
  });

  $effect(() => {
    if (!typingDone || reducedMotion) return;
    const start = performance.now();
    const duration = 9000;
    let raf: number;

    const loop = (now: number) => {
      const t = ((now - start) % duration) / duration;
      const eased = 1 - Math.pow(1 - t, 2.3);
      progress = Math.round(eased * 100);
      bytes = Math.round(eased * totalMB);
      speedMbps = Math.round(80 + Math.sin(t * Math.PI * 4) * 22 + 14);
      const remaining = Math.max(0, Math.round((1 - eased) * 12));
      eta = `00:${String(remaining).padStart(2, "0")}`;
      raf = requestAnimationFrame(loop);
    };
    raf = requestAnimationFrame(loop);
    return () => cancelAnimationFrame(raf);
  });
</script>

<div class="terminal-window w-full max-w-xl">
  <div class="terminal-header">
    <span class="terminal-dot" style="background: #ef4444" aria-hidden="true"></span>
    <span class="terminal-dot" style="background: #eab308" aria-hidden="true"></span>
    <span class="terminal-dot" style="background: #22c55e" aria-hidden="true"></span>
    <span class="ml-2 text-xs terminal-dim">amigo-dl — zsh</span>
  </div>
  <div class="terminal-body">
    <div class="whitespace-pre-wrap break-all">
      <span class="terminal-prompt">$ </span><span class="font-semibold">amigo-dl </span><span class="terminal-dim">{typed.startsWith("$ amigo-dl ") ? typed.slice("$ amigo-dl ".length) : ""}</span>{!typingDone ? "▍" : ""}
    </div>

    {#if typingDone}
      <div class="mt-3 text-xs terminal-dim">resolving… 8 parallel chunks</div>
      <div class="mt-3">
        <ChunkViz chunks={8} {progress} active={progress < 100} size="detailed" />
      </div>
      <div class="mt-3 grid grid-cols-3 gap-3 text-xs" style="color: var(--text-secondary)">
        <div>
          <div class="stat-label mb-0.5">Progress</div>
          <div class="font-mono" style="color: var(--text-primary)">{progress}%</div>
        </div>
        <div>
          <div class="stat-label mb-0.5">Speed</div>
          <div class="font-mono" style="color: var(--text-primary)">{speedMbps} MB/s</div>
        </div>
        <div>
          <div class="stat-label mb-0.5">ETA</div>
          <div class="font-mono" style="color: var(--text-primary)">{eta}</div>
        </div>
      </div>
      <div class="mt-2 text-xs font-mono terminal-dim">
        {bytes} / {totalMB} MB
      </div>
    {/if}
  </div>
</div>
