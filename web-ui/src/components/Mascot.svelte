<script lang="ts">
  // Pixel-art "Orb Bot" mascot — round head with headphones
  // Built on a 16x16 pixel grid, scaled up with crisp rendering
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
  <!-- Headphones band -->
  <rect x="4" y="1" width="8" height="1" fill="var(--text-secondary-color)" opacity="0.5" />
  <!-- Headphones left -->
  <rect x="2" y="2" width="2" height="3" fill="var(--accent-color)" />
  <rect x="2" y="2" width="2" height="1" fill="var(--accent-color)" opacity="0.7" class:headphone-pulse={animate} />
  <!-- Headphones right -->
  <rect x="12" y="2" width="2" height="3" fill="var(--accent-color)" />
  <rect x="12" y="2" width="2" height="1" fill="var(--accent-color)" opacity="0.7" class:headphone-pulse={animate} />

  <!-- Head (large sphere) -->
  <rect x="5" y="1" width="6" height="7" fill="var(--surface-color)" />
  <rect x="4" y="2" width="8" height="5" fill="var(--surface-color)" />

  <!-- Visor -->
  <rect x="5" y="3" width="6" height="3" fill="var(--text-color)" opacity="0.05" />

  <!-- Eyes (LED style) -->
  <rect x="5" y="3" width="2" height="2" fill="var(--accent-color)" class:eye-blink={animate} />
  <rect x="9" y="3" width="2" height="2" fill="var(--accent-color)" class:eye-blink={animate} />
  <!-- Eye inner -->
  <rect x="6" y="4" width="1" height="1" fill="white" />
  <rect x="9" y="4" width="1" height="1" fill="white" />

  <!-- Mouth (LED bar) -->
  <rect x="6" y="6" width="4" height="1" fill="var(--accent-color)" opacity="0.4" />

  <!-- Neck -->
  <rect x="7" y="8" width="2" height="1" fill="var(--text-secondary-color)" opacity="0.4" />

  <!-- Body -->
  <rect x="5" y="9" width="6" height="4" fill="var(--accent-color)" />
  <rect x="4" y="10" width="8" height="2" fill="var(--accent-color)" />

  <!-- Chest light -->
  <rect x="7" y="10" width="2" height="2" fill="var(--accent-color)" class:chest-glow={animate} />
  <rect x="7" y="10" width="1" height="1" fill="white" opacity="0.5" />

  <!-- Arms -->
  <rect x="2" y="9" width="2" height="1" fill="var(--accent-color)" />
  <rect x="1" y="10" width="2" height="3" fill="var(--accent-color)" />
  <rect x="12" y="9" width="2" height="1" fill="var(--accent-color)" />
  <rect x="13" y="10" width="2" height="3" fill="var(--accent-color)" />

  <!-- Waving hand -->
  <rect x="0" y="12" width="2" height="1" fill="var(--accent-color)" opacity="0.7" class:wave={animate} />
  <rect x="14" y="10" width="1" height="1" fill="var(--accent-color)" opacity="0.7" />

  <!-- Legs -->
  <rect x="5" y="13" width="2" height="2" fill="var(--text-color)" opacity="0.3" />
  <rect x="9" y="13" width="2" height="2" fill="var(--text-color)" opacity="0.3" />

  <!-- Feet -->
  <rect x="4" y="15" width="3" height="1" fill="var(--text-color)" opacity="0.2" />
  <rect x="9" y="15" width="3" height="1" fill="var(--text-color)" opacity="0.2" />
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
    0%, 100% { opacity: 0.5; }
    50% { opacity: 1; }
  }

  @keyframes wave {
    0%, 80%, 100% { transform: translateY(0); }
    40% { transform: translateY(-1px); }
    60% { transform: translateY(1px); }
  }

  .mascot-bounce {
    animation: bounce 2s ease-in-out infinite;
  }

  .eye-blink {
    animation: blink 4s ease-in-out infinite;
  }

  .chest-glow {
    animation: glow 1.5s ease-in-out infinite;
  }

  .headphone-pulse {
    animation: glow 2s ease-in-out infinite;
  }

  .wave {
    animation: wave 3s ease-in-out infinite;
  }
</style>
