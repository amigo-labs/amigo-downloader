// Svelte action: trap Tab focus inside a dialog/overlay and restore focus to
// the previously focused element on teardown. Apply with `use:focusTrap` on the
// dialog container. Keeps keyboard users from tabbing into the page behind a
// modal and returns them to where they were when it closes (WCAG 2.4.3).
//
// `active` exists because overlays stack: only the topmost layer may hold the
// trap, otherwise two traps fight over Tab and focus ping-pongs between them.
// A trap that is mounted but inactive does nothing at all — it neither steals
// focus on mount nor restores it on destroy.

const FOCUSABLE =
  'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

export interface FocusTrapOptions {
  /** Whether this trap is the active one. Defaults to true. */
  active?: boolean;
}

export function focusTrap(node: HTMLElement, options: FocusTrapOptions = {}) {
  let active = options.active ?? true;
  const previouslyFocused = document.activeElement as HTMLElement | null;
  let claimedFocus = false;

  function focusable(): HTMLElement[] {
    return Array.from(node.querySelectorAll<HTMLElement>(FOCUSABLE)).filter(
      (el) => el.offsetParent !== null || el === document.activeElement,
    );
  }

  function onKeydown(e: KeyboardEvent) {
    if (!active || e.key !== "Tab") return;
    const items = focusable();
    if (items.length === 0) {
      e.preventDefault();
      return;
    }
    const first = items[0];
    const last = items[items.length - 1];
    const current = document.activeElement as HTMLElement | null;
    if (e.shiftKey && (current === first || !node.contains(current))) {
      e.preventDefault();
      last.focus();
    } else if (!e.shiftKey && current === last) {
      e.preventDefault();
      first.focus();
    }
  }

  function claim() {
    if (claimedFocus || !active) return;
    claimedFocus = true;
    // Honour an explicit [data-autofocus] target if the dialog names one.
    queueMicrotask(() => {
      if (!active) return;
      const target =
        node.querySelector<HTMLElement>("[data-autofocus]") ?? focusable()[0] ?? node;
      target.focus();
    });
  }

  claim();
  node.addEventListener("keydown", onKeydown);

  return {
    update(next: FocusTrapOptions = {}) {
      active = next.active ?? true;
      claim();
    },
    destroy() {
      node.removeEventListener("keydown", onKeydown);
      if (claimedFocus) previouslyFocused?.focus?.();
    },
  };
}

/**
 * Reference-counted body scroll lock.
 *
 * Refcounting matters because overlays stack: a dialog opened from inside a
 * side panel must not unlock the page when only it closes.
 */
let lockCount = 0;
let savedOverflow = "";
let savedPaddingRight = "";

export function lockScroll(): () => void {
  if (typeof document === "undefined") return () => {};

  if (lockCount === 0) {
    const body = document.body;
    savedOverflow = body.style.overflow;
    savedPaddingRight = body.style.paddingRight;
    // Compensate for the disappearing scrollbar so the layout doesn't jump.
    const gap = window.innerWidth - document.documentElement.clientWidth;
    if (gap > 0) body.style.paddingRight = `${gap}px`;
    body.style.overflow = "hidden";
  }
  lockCount++;

  let released = false;
  return () => {
    if (released) return;
    released = true;
    lockCount = Math.max(0, lockCount - 1);
    if (lockCount === 0) {
      document.body.style.overflow = savedOverflow;
      document.body.style.paddingRight = savedPaddingRight;
    }
  };
}
