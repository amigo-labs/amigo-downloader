<script lang="ts">
  import { addToast } from "../lib/toast";

  let { captcha, onclose }: {
    captcha: {
      id: string;
      plugin_id: string;
      download_id: string;
      image_url: string;
      captcha_type: string;
    };
    onclose: () => void;
  } = $props();

  let answer = $state("");
  let submitting = $state(false);
  let elapsed = $state(0);
  const TIMEOUT = 300; // 5 minutes

  // Countdown timer
  const timer = setInterval(() => {
    elapsed++;
    if (elapsed >= TIMEOUT) {
      clearInterval(timer);
      addToast("error", "Captcha timed out");
      onclose();
    }
  }, 1000);

  // Play notification sound
  try {
    const ctx = new AudioContext();
    const osc = ctx.createOscillator();
    const gain = ctx.createGain();
    osc.connect(gain);
    gain.connect(ctx.destination);
    osc.frequency.value = 880;
    gain.gain.value = 0.1;
    osc.start();
    setTimeout(() => { osc.stop(); ctx.close(); }, 200);
  } catch { /* no audio */ }

  function remaining(): string {
    const secs = Math.max(0, TIMEOUT - elapsed);
    const m = Math.floor(secs / 60);
    const s = secs % 60;
    return `${m}:${s.toString().padStart(2, "0")}`;
  }

  async function submitAnswer() {
    if (!answer.trim() || submitting) return;
    submitting = true;
    try {
      const res = await fetch(`/api/v1/captcha/${captcha.id}/solve`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ answer: answer.trim() }),
      });
      if (res.ok) {
        addToast("success", "Captcha solved");
      } else {
        addToast("error", "Failed to submit captcha");
      }
    } catch {
      addToast("error", "Failed to submit captcha");
    }
    clearInterval(timer);
    onclose();
  }

  async function skip() {
    try {
      await fetch(`/api/v1/captcha/${captcha.id}/cancel`, { method: "POST" });
    } catch { /* ignore */ }
    clearInterval(timer);
    onclose();
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Enter" && answer.trim()) {
      e.preventDefault();
      submitAnswer();
    } else if (e.key === "Escape") {
      skip();
    }
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="fixed inset-0 z-[100] flex items-center justify-center bg-black/60 backdrop-blur-sm"
  onkeydown={handleKeydown}
>
  <div
    class="w-full max-w-md mx-4 rounded-2xl shadow-2xl overflow-hidden"
    style="background: var(--card-bg)"
  >
    <!-- Header -->
    <div class="flex items-center justify-between px-5 py-3 border-b" style="border-color: var(--border-color)">
      <div>
        <h3 class="font-bold text-base">Captcha Required</h3>
        <p class="text-xs mt-0.5" style="color: var(--text-secondary-color)">
          {captcha.plugin_id} &middot; {captcha.captcha_type}
        </p>
      </div>
      <div class="flex items-center gap-2">
        <!-- Countdown -->
        <span
          class="text-xs font-mono px-2 py-0.5 rounded"
          class:text-red-500={TIMEOUT - elapsed < 60}
          style="background: var(--surface-color)"
        >
          {remaining()}
        </span>
      </div>
    </div>

    <!-- Captcha Image -->
    <div class="p-5 flex flex-col items-center gap-4">
      <div class="w-full bg-white rounded-lg p-2 flex items-center justify-center min-h-[120px]">
        <img
          src={captcha.image_url}
          alt="Captcha"
          class="max-w-full max-h-48 object-contain"
          crossorigin="anonymous"
        />
      </div>

      <!-- Progress bar -->
      <div class="w-full h-1 rounded-full overflow-hidden" style="background: var(--border-color)">
        <div
          class="h-full rounded-full transition-all duration-1000"
          style="width: {((TIMEOUT - elapsed) / TIMEOUT) * 100}%; background: var(--accent-color)"
        ></div>
      </div>

      <!-- Input -->
      <input
        type="text"
        bind:value={answer}
        placeholder="Enter captcha text..."
        class="w-full px-4 py-3 rounded-lg text-center text-lg font-mono tracking-wider border-2 outline-none transition-colors"
        style="background: var(--surface-color); border-color: var(--border-color); color: var(--text-color)"
        onfocus={(e) => { (e.target as HTMLInputElement).style.borderColor = 'var(--accent-color)'; }}
        onblur={(e) => { (e.target as HTMLInputElement).style.borderColor = 'var(--border-color)'; }}
        autofocus
        disabled={submitting}
      />

      <!-- Buttons -->
      <div class="flex gap-3 w-full">
        <button
          onclick={skip}
          class="flex-1 py-2.5 rounded-lg text-sm font-medium border transition-colors"
          style="border-color: var(--border-color); color: var(--text-secondary-color)"
          disabled={submitting}
        >
          Skip
        </button>
        <button
          onclick={submitAnswer}
          class="flex-1 py-2.5 rounded-lg text-sm font-semibold text-white transition-all hover:brightness-110 active:scale-[0.98]"
          style="background: var(--accent-color)"
          disabled={!answer.trim() || submitting}
        >
          {submitting ? "Submitting..." : "Solve"}
        </button>
      </div>
    </div>
  </div>
</div>
