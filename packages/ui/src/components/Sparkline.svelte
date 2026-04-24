<script lang="ts">
  // Mini speed graph — last 30 data points
  // Fix H8: unique gradient ID per instance
  let { values = [], width = 120, height = 32, color = "var(--neon-primary)" }:
    { values?: number[]; width?: number; height?: number; color?: string } = $props();

  const gradientId = `spark-fill-${crypto.randomUUID().slice(0, 8)}`;

  let points = $derived(() => {
    if (values.length === 0) return "";
    const max = Math.max(...values, 1);
    const step = width / Math.max(values.length - 1, 1);
    return values
      .map((v, i) => `${i * step},${height - (v / max) * (height - 2) - 1}`)
      .join(" ");
  });

  let fillPoints = $derived(() => {
    if (values.length === 0) return "";
    const max = Math.max(...values, 1);
    const step = width / Math.max(values.length - 1, 1);
    const line = values
      .map((v, i) => `${i * step},${height - (v / max) * (height - 2) - 1}`)
      .join(" ");
    return `0,${height} ${line} ${(values.length - 1) * step},${height}`;
  });
</script>

<svg {width} {height} class="overflow-visible">
  <defs>
    <linearGradient id={gradientId} x1="0" y1="0" x2="0" y2="1">
      <stop offset="0%" stop-color={color} stop-opacity="0.2" />
      <stop offset="100%" stop-color={color} stop-opacity="0" />
    </linearGradient>
  </defs>

  {#if values.length > 1}
    <polygon points={fillPoints()} fill="url(#{gradientId})" />
    <polyline
      points={points()}
      fill="none"
      stroke={color}
      stroke-width="1.5"
      stroke-linecap="round"
      stroke-linejoin="round"
    />
    {@const max = Math.max(...values, 1)}
    {@const lastVal = values[values.length - 1]}
    {@const lastX = (values.length - 1) * (width / Math.max(values.length - 1, 1))}
    {@const lastY = height - (lastVal / max) * (height - 2) - 1}
    <circle cx={lastX} cy={lastY} r="2" fill={color} class="sparkline-dot" />
  {/if}
</svg>

<style>
  @keyframes pulse-dot {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.4; }
  }

  .sparkline-dot {
    animation: pulse-dot 1.5s ease-in-out infinite;
  }
</style>
