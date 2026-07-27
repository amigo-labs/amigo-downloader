// Media-query stores.
//
// Chrome used to switch at two different widths: the sidebar and bottom nav
// flipped at `md` (768px) while the side panel flipped at `lg` (1024px), so
// 768-1023px showed a desktop sidebar *and* an overlay side panel. Everything
// structural now switches at one breakpoint, DESKTOP.
//
// These are stores rather than CSS classes because the side panel must render
// exactly one variant: hiding the other with `hidden lg:flex` still mounted
// its children, which duplicated DOM ids and ran every onMount twice.

import { readable, type Readable } from "svelte/store";

/** Single structural breakpoint: below this the app is in "compact" chrome. */
export const DESKTOP = "(min-width: 1024px)";
/** Secondary, for progressive disclosure of header stats only. */
export const WIDE = "(min-width: 1280px)";
/** Phones, where multi-column forms and dense rows must collapse. */
export const PHONE = "(max-width: 639px)";

export function mediaQuery(query: string): Readable<boolean> {
  return readable(
    typeof window !== "undefined" ? window.matchMedia(query).matches : false,
    (set) => {
      if (typeof window === "undefined") return;
      const mql = window.matchMedia(query);
      const update = () => set(mql.matches);
      update();
      mql.addEventListener("change", update);
      return () => mql.removeEventListener("change", update);
    },
  );
}

export const isDesktop = mediaQuery(DESKTOP);
export const isWide = mediaQuery(WIDE);
export const isPhone = mediaQuery(PHONE);
