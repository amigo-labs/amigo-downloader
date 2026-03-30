<script lang="ts">
  import { onMount } from "svelte";
  import { addToast } from "../lib/toast";
  import Icon from "./Icon.svelte";

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
  let timerRef: ReturnType<typeof setInterval> | undefined;
  const TIMEOUT = 300;

  // Fix H7: move setInterval into onMount with cleanup
  onMount(() => {
    timerRef = setInterval(() => {
      elapsed++;
      if (elapsed >= TIMEOUT) {
        if (timerRef) clearInterval(timerRef);
        addToast("error", "Captcha timed out");
        onclose();
      }
    }, 1000);

    return () => {
      if (timerRef) clearInterval(timerRef);
    };
  });

  function remaining(): string {
    const secs = Math.max(0, TIMEOUT - elapsed);
    const m = Math.floor(secs / 60);
    const s = secs % 60;
    return `${m}:${s.toString().padStart(2, "0")}`;
  }

  // Fix M3: defer AudioContext to user gesture
  let soundPlayed = false;
  function playNotificationSound() {
    if (soundPlayed) return;
    soundPlayed = true;
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
  }

  async function submitAnswer() {
    if (!answer.trim() || submitting) return;
    playNotificationSound();
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
    if (timerRef) clearInterval(timerRef);
    onclose();
  }

  async function skip() {
    try {
      await fetch(`/api/v1/captcha/${captcha.id}/cancel`, { method: "POST" });
    } catch { /* ignore */ }
    if (timerRef) clearInterval(timerRef);
    onclose();
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Enter" && answer.trim()) {
      e.preventDefault();
      submitAnswer();
    }
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="fixed inset-0 z-[100] flex items-center justify-center bg-black/70"
  onkeydown={handleKeydown}
>
  <!-- Dialog (audit C2) -->
  <div
    role="dialog"
    aria-modal="true"
    aria-labelledby="captcha-title"
    class="w-full max-w-md mx-4 rounded-2xl shadow-2xl overflow-hidden"
    style="background: var(--bg-surface); border: 1px solid var(--border-color)"
  >
    <!-- Header -->
    <div class="flex items-center justify-between px-5 py-3 border-b" style="border-color: var(--border-color)">
      <div>
        <h3 id="captcha-title" class="font-bold text-base" style="color: var(--text-primary)">Captcha Required</h3>
        <p class="text-xs mt-0.5" style="color: var(--text-secondary)">
          {captcha.plugin_id} &middot; {captcha.captcha_type}
        </p>
      </div>
      <span
        class="text-xs px-2 py-0.5 rounded"
        class:neon-text-accent={TIMEOUT - elapsed < 60}
        style="font-family: 'Share Tech Mono', monospace; background: var(--bg-surface-2); color: var(--text-secondary)"
      >
        {remaining()}
      </span>
    </div>

    <div class="p-5 flex flex-col items-center gap-4">
      <!-- Captcha Image -->
      <div class="w-full bg-white rounded-lg p-2 flex items-center justify-center min-h-[120px]">
        <img
          src={captcha.image_url}
          alt="Captcha"
          class="max-w-full max-h-48 object-contain"
          crossorigin="anonymous"
        />
      </div>

      <!-- Progress bar -->
      <div class="w-full h-1 rounded-full overflow-hidden" style="background: rgba(255,255,255,0.06)">
        <div
          class="h-full rounded-full transition-all duration-1000"
          style="width: {((TIMEOUT - elapsed) / TIMEOUT) * 100}%; background: var(--neon-primary)"
        ></div>
      </div>

      <!-- Input -->
      <input
        type="text"
        bind:value={answer}
        placeholder="Enter captcha text..."
        class="w-full px-4 py-3 rounded-lg text-center text-lg tracking-wider border"
        style="font-family: 'Share Tech Mono', monospace; background: var(--bg-surface-2); border-color: var(--border-color); color: var(--text-primary)"
        autofocus
        disabled={submitting}
      />

      <!-- Buttons -->
      <div class="flex gap-3 w-full">
        <button
          onclick={skip}
          class="flex-1 py-2.5 rounded-lg text-sm font-medium border transition-colors min-h-[44px]"
          style="border-color: var(--border-color); color: var(--text-secondary)"
          disabled={submitting}
        >
          Skip
        </button>
        <button
          onclick={submitAnswer}
          class="flex-1 py-2.5 rounded-lg text-sm font-semibold transition-colors disabled:opacity-40 min-h-[44px]"
          style="background: var(--neon-primary); color: var(--bg-deep)"
          disabled={!answer.trim() || submitting}
        >
          {submitting ? "Submitting..." : "Solve"}
        </button>
      </div>
    </div>
  </div>
</div>
