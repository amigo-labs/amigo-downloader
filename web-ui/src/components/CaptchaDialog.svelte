<script lang="ts">
  import { onMount } from "svelte";
  import { addToast } from "../lib/toast";
  import { locale, tr } from "../lib/i18n";
  import { openLayer, layerState } from "../lib/overlays.svelte";
  import Dialog from "@amigo/ui/components/Dialog.svelte";
  import Button from "@amigo/ui/components/Button.svelte";
  import ProgressBar from "@amigo/ui/components/ProgressBar.svelte";

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
  let imageFailed = $state(false);
  let timerRef: ReturnType<typeof setInterval> | undefined;
  const TIMEOUT = 300;

  // Registered as "critical" but still dismissible: this dialog used to have
  // no Escape, no backdrop click and no entry in the global Escape chain, so
  // together with its focus trap it was a genuine keyboard trap. Dismissing
  // it now cancels the captcha server-side, which is what Skip already did.
  const layer = layerState("captcha");
  $effect(() =>
    openLayer({
      id: "captcha",
      kind: "critical",
      label: "Captcha",
      dismissible: true,
      onDismiss: () => void skip(),
    }),
  );

  onMount(() => {
    timerRef = setInterval(() => {
      elapsed++;
      if (elapsed >= TIMEOUT) {
        if (timerRef) clearInterval(timerRef);
        addToast("error", tr($locale, "captcha.expired"));
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

  // Deferred to a user gesture so the AudioContext isn't blocked by autoplay.
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
        credentials: "same-origin",
        body: JSON.stringify({ answer: answer.trim() }),
      });
      addToast(
        res.ok ? "success" : "error",
        tr($locale, res.ok ? "captcha.solved" : "captcha.failed"),
      );
    } catch {
      addToast("error", tr($locale, "captcha.failed"));
    }
    if (timerRef) clearInterval(timerRef);
    onclose();
  }

  async function skip() {
    try {
      await fetch(`/api/v1/captcha/${captcha.id}/cancel`, {
        method: "POST",
        credentials: "same-origin",
      });
    } catch { /* ignore */ }
    if (timerRef) clearInterval(timerRef);
    onclose();
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === "Enter" && answer.trim()) {
      e.preventDefault();
      submitAnswer();
    }
  }
</script>

<Dialog
  title={tr($locale, "captcha.title")}
  description="{captcha.plugin_id} · {captcha.captcha_type}"
  size="sm"
  layer="modal"
  isTop={$layer.isTop}
  closeLabel={tr($locale, "captcha.skip")}
  onclose={skip}
>
  {#snippet header()}
    <span
      class="shrink-0 text-xs px-2 py-0.5 rounded"
      style="font-family: var(--font-mono); background: var(--bg-surface-2);
             color: {TIMEOUT - elapsed < 60 ? 'var(--danger-ink)' : 'var(--text-secondary)'}"
      aria-live="off"
    >
      {remaining()}
    </span>
  {/snippet}

  <div class="flex flex-col items-center gap-4" {onkeydown} role="none">
    <!-- The captcha art is generally dark-on-white, so the plate stays white
         in both themes; that is the image's background, not the app's, and
         the text on it must be dark regardless of the active theme. -->
    <!-- ui-lint-disable-next-line no-raw-hex -->
    <div class="w-full rounded-lg p-2 flex items-center justify-center min-h-[120px]" style="background: #ffffff">
      {#if imageFailed}
        <!-- ui-lint-disable-next-line no-raw-hex -->
        <p class="text-sm" style="color: #b91c1c">{tr($locale, "captcha.image_failed")}</p>
      {:else}
        <img
          src={captcha.image_url}
          alt={tr($locale, "captcha.image_alt")}
          class="max-w-full max-h-48 object-contain"
          crossorigin="anonymous"
          onerror={() => (imageFailed = true)}
        />
      {/if}
    </div>

    <ProgressBar
      value={TIMEOUT - elapsed}
      max={TIMEOUT}
      tone={TIMEOUT - elapsed < 60 ? "danger" : "accent"}
      label={tr($locale, "captcha.time_left")}
      valueText={remaining()}
    />

    <input
      type="text"
      bind:value={answer}
      placeholder={tr($locale, "captcha.enter")}
      aria-label={tr($locale, "captcha.enter")}
      class="w-full px-4 py-3 rounded-lg text-center text-lg tracking-wider border"
      style="font-family: var(--font-mono); background: var(--bg-surface-2);
             border-color: var(--border-color); color: var(--text-primary)"
      disabled={submitting}
      data-autofocus
    />
  </div>

  {#snippet footer()}
    <Button variant="outline" tone="accent" onclick={skip} disabled={submitting}>
      {tr($locale, "captcha.skip")}
    </Button>
    <Button
      variant="solid"
      onclick={submitAnswer}
      loading={submitting}
      disabled={!answer.trim()}
    >
      {tr($locale, "captcha.solve")}
    </Button>
  {/snippet}
</Dialog>
