// Shared motion helpers — keep list/transition feel consistent and always
// honour the user's reduced-motion preference (mirrors the global CSS guard
// in tokens.css so JS-driven Svelte transitions are suppressed too).
import { cubicOut } from "svelte/easing";
import type { TransitionConfig } from "svelte/transition";

export function prefersReducedMotion(): boolean {
  return (
    typeof window !== "undefined" &&
    typeof window.matchMedia === "function" &&
    window.matchMedia("(prefers-reduced-motion: reduce)").matches
  );
}

/**
 * Shared `animate:flip` config for lists. Filter/sort/reorder changes glide
 * to their new position instead of jumping. Distance-aware duration, capped so
 * long moves still feel snappy. Collapses to 0 under reduced-motion.
 */
export const flipConfig = {
  duration: (len: number) => (prefersReducedMotion() ? 0 : Math.min(420, 160 + len * 0.4)),
  easing: cubicOut,
};

/** Combined scale + fade — the house transition for popovers, menus, cards. */
export function scaleFade(
  _node: Element,
  { duration = 200, start = 0.96, y = 0 }: { duration?: number; start?: number; y?: number } = {},
): TransitionConfig {
  if (prefersReducedMotion()) return { duration: 0 };
  return {
    duration,
    easing: cubicOut,
    css: (t) =>
      `opacity: ${t}; transform: translateY(${(1 - t) * y}px) scale(${start + (1 - start) * t});`,
  };
}

/** Reduced-motion-aware duration for `fly`/`slide` params. */
export function dur(ms: number): number {
  return prefersReducedMotion() ? 0 : ms;
}
