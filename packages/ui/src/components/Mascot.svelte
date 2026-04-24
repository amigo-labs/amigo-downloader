<script lang="ts">
  let { size = 64, animate = false }: { size?: number; animate?: boolean } = $props();
</script>

<img
  src="/amigo-logo.png"
  alt="amigo-downloader"
  width={size}
  height={size}
  class="rounded-lg"
  class:mascot-glow={animate}
/>

<style>
  @keyframes glow-pulse {
    0%, 100% { filter: drop-shadow(0 0 4px color-mix(in srgb, var(--neon-primary) 30%, transparent)); }
    50% { filter: drop-shadow(0 0 12px color-mix(in srgb, var(--neon-primary) 50%, transparent)); }
  }

  .mascot-glow {
    animation: glow-pulse 2s ease-in-out infinite;
  }
</style>
