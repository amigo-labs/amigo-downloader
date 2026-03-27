<script lang="ts">
  // Pixel-art "Amigo" mascot — a friendly download robot
  // Built on an 16x16 pixel grid, scaled up with crisp rendering
  let { size = 64, animate = false }: { size?: number; animate?: boolean } = $props();
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 16 16"
  fill="none"
  xmlns="http://www.w3.org/2000/svg"
  class="pixel-logo"
  class:mascot-bounce={animate}
  style="image-rendering: pixelated"
>
  <!-- Body (rounded robot shape) -->
  <rect x="4" y="3" width="8" height="9" rx="0" fill="var(--accent-color)" />
  <rect x="3" y="4" width="10" height="7" rx="0" fill="var(--accent-color)" />

  <!-- Screen/Face area -->
  <rect x="5" y="4" width="6" height="5" fill="var(--surface-color)" />

  <!-- Eyes -->
  <rect x="6" y="5" width="1" height="2" fill="var(--accent-color)" class:eye-blink={animate} />
  <rect x="9" y="5" width="1" height="2" fill="var(--accent-color)" class:eye-blink={animate} />

  <!-- Smile -->
  <rect x="6" y="7" width="1" height="1" fill="var(--accent-color)" opacity="0.5" />
  <rect x="7" y="8" width="2" height="1" fill="var(--accent-color)" opacity="0.5" />
  <rect x="9" y="7" width="1" height="1" fill="var(--accent-color)" opacity="0.5" />

  <!-- Antenna -->
  <rect x="7" y="1" width="2" height="2" fill="var(--accent-color)" />
  <rect x="7" y="0" width="2" height="1" fill="var(--accent-color)" class:antenna-glow={animate} />

  <!-- Arms -->
  <rect x="2" y="5" width="1" height="4" fill="var(--accent-color)" />
  <rect x="13" y="5" width="1" height="4" fill="var(--accent-color)" />

  <!-- Hands -->
  <rect x="1" y="8" width="2" height="2" fill="var(--accent-color)" />
  <rect x="13" y="8" width="2" height="2" fill="var(--accent-color)" />

  <!-- Download arrow on chest -->
  <rect x="7" y="9" width="2" height="2" fill="var(--surface-color)" opacity="0.8" />
  <rect x="7" y="10" width="2" height="1" fill="var(--surface-color)" />
  <rect x="6" y="10" width="4" height="1" fill="var(--surface-color)" opacity="0.6" />

  <!-- Feet -->
  <rect x="4" y="12" width="3" height="2" fill="var(--accent-color)" />
  <rect x="9" y="12" width="3" height="2" fill="var(--accent-color)" />

  <!-- Highlight/shine -->
  <rect x="5" y="4" width="1" height="1" fill="white" opacity="0.3" />
</svg>

<style>
  @keyframes bounce {
    0%, 100% { transform: translateY(0); }
    50% { transform: translateY(-4px); }
  }

  @keyframes blink {
    0%, 90%, 100% { opacity: 1; }
    95% { opacity: 0; }
  }

  @keyframes glow {
    0%, 100% { opacity: 0.6; }
    50% { opacity: 1; }
  }

  .mascot-bounce {
    animation: bounce 2s ease-in-out infinite;
  }

  .eye-blink {
    animation: blink 4s ease-in-out infinite;
  }

  .antenna-glow {
    animation: glow 1.5s ease-in-out infinite;
  }
</style>
