# UI Review 2026-07: Findings and Rebuild

> Implemented on branch `claude/ui-review-improvements-sjql5c`.
> Each finding below records where it was fixed, or why it was deferred.

## Context

The web UI (`web-ui/`, Svelte 5 + Tailwind v4, ~6.400 lines plus
`packages/ui`) grew through many incremental polish passes. It was
functionally dense but had drifted structurally: the design-system contract in
`.impeccable.md` ("WCAG AA across all themes", "responsive without
compromise") was not being met at several load-bearing points, three screens
sat entirely outside the system, there were no UI primitives — ~380 inline
`style=` attributes re-implemented the same four patterns by hand — and two
interactions were simply broken.

This review was produced by reading every component, page and token file, and
verifying each claim against the running configuration (including the server's
own CSP and the contrast maths). The rebuild landed as nine commits.

---

## A. Defects

| # | Finding | Fixed in |
|---|---|---|
| A1 | **Inter never loaded in the shipped server build.** `index.html` fetched it from `fonts.googleapis.com`, which the server's own CSP (`style-src 'self'`, `font-src 'self' data:`) blocks. Invisible under `vite dev`, which sets no CSP header, so the UI silently rendered in `system-ui` in every Docker/server install. Tauri separately allowlisted Google Fonts, fetching the font externally on each start. | `4d7d8c8` — self-host the latin subset (48 KB vs 224 KB for all subsets); the server CSP was already correct and needed no change |
| A2 | **Drag-to-reorder never worked.** `DropZone` bound `ondragenter/over/drop` to `<svelte:window>` with no source check, so a card drag raised the full-viewport `z-[200]` file-drop overlay, which then swallowed the events meant for the target row. `reorderQueue()` was never called; the only symptom was a flash of "Drop files here". | `1fce7cd` — `lib/dnd.ts`, three-way source discrimination |
| A3 | **CaptchaDialog was a keyboard trap.** No Escape, no backdrop dismiss, absent from the global Escape chain, and still applying a focus trap. Only exits were Solve, Skip, or waiting out the 300 s timer. | `1fce7cd` |
| A4 | **`SidePanel` mounted its children twice.** Both variants sat unconditionally in the DOM, hidden by CSS — duplicate DOM ids, duplicate state, doubled `onMount` fetches, and a focus trap running on a `display:none` node. | `1fce7cd` — one variant, chosen by `matchMedia` |
| A5 | **`PairingModal` used five CSS variables that do not exist** (`--surface`, `--border`, `--muted`, `--fg`, `--surface-2`), so it always rendered its hardcoded dark fallbacks and was unreadable in light mode. | `5ec52bf` |
| A6 | **Escape dismissed the wrong layer.** `$sidePanelMode` was checked before the dialogs, so Escape over a modal closed the panel behind it. `Ctrl+N` was unguarded entirely. | `1fce7cd` — `lib/overlays.svelte.ts` + `lib/keymap.ts` |
| A7 | **Settings swallowed load failures.** `catch { /* offline */ }` rendered "no servers configured" when the server was unreachable. | `5ec52bf` |
| A8 | **`DetailPanel` had no `{:else}`** — a download deleted elsewhere left a blank body under a populated header. | `ea86852` |
| A9 | **The Retry button had never worked.** The client sent `action: "retry"`; the server accepted only `pause`/`resume` and answered 400. | `c4cb317` |

## B. Accessibility

Contrast failed in **both** themes, not just light: `:root.light` overrode
backgrounds and text but never `--neon-*`, and those were used as text colours
throughout.

| Token as text | on light `#ffffff` | on dark `#252526` / `#2d2d2d` |
|---|---|---|
| `--neon-warning` `#eab308` | **1.85** | ok |
| `--neon-success` `#22c55e` | **2.30** | ok |
| `--neon-primary` `#3b82f6` | **3.68** | **4.16 / 3.74** |
| palette indigo `#6366f1` | — | **3.43 / 3.08** |
| palette violet `#8b5cf6` | — | **3.62 / 3.25** |
| palette rose `#f43f5e` | — | **4.17 / 3.75** |
| palette amber `#f59e0b` | **2.15** | ok |
| `--neon-accent` `#ef4444` | — | **4.07 / 3.66** |

`--text-secondary` — the colour of nearly all meta information — failed in both
themes (dark 4.15 / 3.73, light ≈ 3.70), and `::placeholder` added
`opacity: .6` on top of that.

Fixed in `4d7d8c8` by splitting each accent into three roles (`--accent`
decorative, `--accent-ink` for text, `--accent-solid` + `--on-accent` for
filled controls), keeping the 500 hues for glows so the Corporate Neon look is
unchanged. `packages/ui/scripts/check-contrast.mjs` now asserts 324 pairs
across 6 palettes × 2 themes in CI.

Other findings, all fixed:

- Batch selection was keyboard-unreachable (`<div onclick>` with no role,
  tabindex or aria-checked), and selection was a *mode* that made Space on a
  card open the detail panel instead of toggling. → `ea86852`
- The card root was `<div role="button">` wrapping an `<h3>` and five
  `<button>`s: invalid ARIA, no heading in the outline, accessible name = the
  entire card. → `ea86852`
- Toasts were never announced: `aria-live` sat on the node being inserted. →
  `1fce7cd`
- Two-step delete was invisible to screen readers — a static `aria-label`
  overrode the visible "Sure?". → `ea86852`
- File upload was mouse-only (`display:none` on the input). → `5ec52bf`
- Eleven form fields had sibling labels with no `for`. → `5ec52bf`
- The neon slider suppressed the global `*:focus-visible` and had no
  `aria-valuetext`. → `4d7d8c8`
- `ContextMenu` never restored focus and bound arrow keys at the window. →
  `1fce7cd`
- `ProgressRing` / `Sparkline` / `ChunkViz` / `.progress-bar` exposed nothing
  to assistive tech. → `ee41e07`
- Three `role="radiogroup"` / `role="tablist"` elements announced a keyboard
  contract they did not implement. → `ee41e07` (`SegmentedControl`)
- `<html lang>` stayed `en` in German. → `1fce7cd`

## C. Usability

Fixed: Shift/Ctrl-click ranges, Ctrl+A and Escape-to-clear (`ea86852`);
bulk actions in one request with progress, cancel and partial-failure
recovery (`c4cb317`); long-press context menu on touch, which the
`"ontouchstart" in window` guard had removed entirely — while also removing it
from Windows touchscreen laptops that have a right mouse button (`ea86852`);
one component for both list densities, ending the feature gap that made the
"density" toggle really a feature toggle (`ea86852`); reordering restricted to
queue order with an inline switch and a keyboard alternative (`ea86852`);
one confirmation policy for every destructive action (`5ec52bf`); History
given search, filters, real per-row status, re-download and clear (`4df456d`);
the plugin marketplace wired to backend endpoints that had shipped long ago
(`39924fd`).

## D. Mobile / tablet

The 768–1023 px dead zone (sidebar at `md`, side panel at `lg`) and the
sub-640 px loss of the HTTP/Usenet filter are fixed in `5ec52bf`: everything
structural switches at `lg`, and the protocol filter moves into the Downloads
toolbar below that. Also fixed: compact rows overflowing under ~480 px, toasts
covering and swallowing taps on the bottom nav, unguarded `grid-cols-2/3/4` in
settings forms, and dialogs without `dvh` or safe-area handling.

## E. Structure

`packages/ui` gained twelve primitives (`ee41e07`) that encode the
accessibility contract in their API — `IconButton.label` is a *required* prop,
`SegmentedControl` implements the real radiogroup pattern, `Dialog` owns focus
trap, Escape, scroll lock and `aria-modal`. `Mascot.svelte`, never imported,
was deleted.

Duplication removed: ~60 lines shared verbatim between the two row components
plus a third copy in `DetailPanel` now live in `lib/download.ts` and
`lib/downloadActions.svelte.ts` (`ea86852`). Six hand-rolled overlays now share
one layer manager where z-index is derived from stack position (`1fce7cd`).

## Verification

```bash
pnpm install
pnpm --filter @amigo/ui check     # 324 contrast assertions + compile every primitive
node scripts/ui-lint.mjs          # hard bans + inline-style ratchet
pnpm --filter amigo-web-ui check  # svelte-check: 0 errors, 0 warnings
pnpm --filter amigo-web-ui test   # 39 unit tests
pnpm build:web-ui
cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings
```

Manual checks must run against a real server (`cargo run -p amigo-server`,
UI on `:1516`) — **not** `vite dev`, which sets no CSP and therefore hides A1.

## Deferred

- **List virtualisation.** Still renders every row. Worth doing above ~150
  items; the hot/cold store split it depends on is not in place yet.
- **WebSocket coalescing.** Progress ticks still re-render the list, and the
  unconditional `loadData()` at the end of the WS handler still triggers three
  HTTP round-trips per progress-adjacent event.
- **`App.svelte` decomposition.** Down from 752 lines but still the boot gate,
  router, sidebar, header, WS controller and bandwidth form in one file.
- **The remaining 218 inline styles.** Ratcheted by `scripts/ui-lint.mjs`, to
  be migrated to primitives file by file.
- **A Playwright + axe-core smoke suite.** Unit tests and the static gates are
  in; nothing yet drives a real browser.
