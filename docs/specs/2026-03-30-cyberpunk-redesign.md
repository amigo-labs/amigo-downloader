# Cyberpunk Arcade Redesign + Audit Fixes

**Status**: Draft
**Date**: 2026-03-30
**Scope**: Complete visual redesign of web-ui + fix all 30 audit issues

## Overview

Transform the amigo-downloader web-ui from its current Tailwind-Slate aesthetic into a **Cyberpunk Arcade** design: neon-dominant colors, glassmorphism panels, pixel-art branding, and a distinctive retro-futuristic feel. Simultaneously fix all accessibility, performance, and theming issues from the UI audit.

Approach: **Redesign First, Fixes Integrated** — rebuild the theme system, then migrate each component with audit fixes baked in.

---

## 1. Visual Direction

### Aesthetic
**Cyberpunk Arcade** — Neon-dominant with glass panels and pixel-art branding elements. Dark backgrounds with luminous accents. CRT scan-line overlay for retro texture.

### Color Palette (4-color Neon System)

| Role | Color | Hex | Usage |
|------|-------|-----|-------|
| Primary | Electric Blue | `#00D4FF` | Active states, links, primary actions, progress bars |
| Success | Cyan | `#00FFD0` | Completed states, success toasts, positive indicators |
| Accent/Error | Hot Pink | `#FF2D78` | Errors, destructive actions, highlights, emphasis |
| Warning | Amber | `#FFB800` | Paused states, warnings, secondary accent |

### Accent System: Hybrid
The 4 semantic roles (primary, success, accent, warning) are fixed. The user can choose which neon color takes the **primary role** from the settings. The other 3 colors redistribute accordingly:

| Preset | Primary | Success | Accent/Error | Warning |
|--------|---------|---------|-------------|---------|
| **Electric** (default) | `#00D4FF` | `#00FFD0` | `#FF2D78` | `#FFB800` |
| **Hot** | `#FF2D78` | `#00FFD0` | `#00D4FF` | `#FFB800` |
| **Cyan** | `#00FFD0` | `#00D4FF` | `#FF2D78` | `#FFB800` |

Amber stays as warning in all presets (it's the most naturally "warning-like" color).

### Background Layers

| Token | Dark Mode | "Lights On" Mode |
|-------|-----------|------------------|
| `--bg-deep` | `#06080f` | `#14162a` |
| `--bg-surface` | `#0c0e18` | `#1a1d38` |
| `--bg-surface-2` | `#12152a` | `#222548` |
| `--bg-glass` | `rgba(primary, 0.08)` | `rgba(primary, 0.06)` |
| `--text-primary` | `#e8ecf4` | `#e8ecf4` |
| `--text-secondary` | `rgba(232,236,244, 0.5)` | `rgba(232,236,244, 0.45)` |

### Glassmorphism Tokens (Medium Intensity)

```css
--glass-blur: 16px;
--glass-opacity: 0.08;       /* background opacity tinted with primary */
--glass-border: 0.15;        /* border opacity tinted with primary */
--glass-shadow: 0 4px 20px rgba(0,0,0,0.3);
```

Cards and panels use: `background: rgba(primary, var(--glass-opacity)); backdrop-filter: blur(var(--glass-blur)); border: 1px solid rgba(primary, var(--glass-border));`

### Typography

| Role | Font | Weight | Fallback |
|------|------|--------|----------|
| UI / Headings | Rajdhani | 400-700 | sans-serif |
| Mono / Data | Share Tech Mono | 400 | monospace |

**Removed**: Inter (generic), Press Start 2P (barely used).

### Logo
Replace the inline SVG pixel-art mascot with `amigo-downloader.png` (cyberpunk mech robot head). Used in:
- Sidebar header (32x32px)
- About section in Settings
- PWA icons / favicon
- Empty states (larger, 64-80px)

---

## 2. Layout Architecture

### Overall Structure

```
┌──────────────────────────────────────────────────────────┐
│ [Sidebar]  │        [Main Content]       │ [Detail Panel]│
│ collapsible│  Header + Filters + List    │  collapsible  │
│  left      │                             │  right        │
├────────────┴─────────────────────────────┴───────────────┤
│                    [Footer Status Bar]                    │
└──────────────────────────────────────────────────────────┘
```

### Left Sidebar (Collapsible)

**Expanded** (220px):
- Logo (32px img) + "AMIGO" brand + version
- "+ ADD DOWNLOAD" button with Ctrl+N hint
- Navigation items with icons + labels + keyboard shortcuts
- Feedback link at bottom

**Collapsed** (56px):
- Logo only (centered)
- "+" button (icon only)
- Nav icons only (centered, tooltip on hover)
- Feedback icon only

**Collapse trigger**: Button at sidebar edge, persisted to localStorage.

**Navigation items** (with Usenet feature flag):
1. Downloads (always)
2. Plugins (always)
3. History (always)
4. Settings (always)

RSS Feeds moves into Settings → Usenet section. No separate nav item.

**Top neon accent line**: 2px gradient bar across the top of sidebar (`linear-gradient(90deg, transparent, primary, accent, transparent)`).

### Header

```
Downloads  [ALL | HTTP | USENET]                    [lights-toggle]
```

- Page title (h2, Rajdhani 700)
- **Segmented Control** for protocol filter: ALL | HTTP | USENET — only visible on Downloads page. Uses glass styling for the toggle container, neon highlight on active segment.
- Theme toggle button (Dark / Lights On)

### Main Content Area

- Filter chips below header: All, Downloading, Queued, Paused, Completed, Failed — with counts
- Download card list with drag handles
- Empty state with logo mascot (64px)

### Right Detail Panel (Master-Detail)

**Collapsed state** (default, nothing selected): Panel is hidden, content area takes full width.

**Expanded state** (download selected, desktop ~320px):
- **Header**: Filename, close button
- **File Info**: URL, protocol badge, filesize, created date
- **Chunk Visualization**: Detailed, larger version (per-chunk progress bars with labels)
- **Speed Graph**: Per-download speed history (sparkline, larger)
- **Connection Details**:
  - HTTP: chunk count, redirect chain, response headers
  - Usenet: server name, active connections, PAR2 status, unrar status
- **Actions**: Pause/Resume, Delete, Retry, Change Priority, Rename
- **Error Log**: Full error message + stack trace for failed downloads

**Mobile behavior** (< 768px): Panel slides in from right as full-height overlay with backdrop, close on swipe-right or close button.

**Transition**: Slide-in from right, 250ms cubic-bezier(0.16, 1, 0.3, 1).

### Footer Status Bar (36px)

Replaces the sidebar stats section. Spans full width below content.

Content (left to right):
- Sparkline mini-graph (120px wide, speed history)
- Separator
- Speed: `42.3 MB/s`
- Separator
- Active: `3` + pulse dot
- Separator
- Queued: `5`
- Separator
- Done: `12` (in success color)
- Right-aligned: Feedback link

Neon accent line on top (1px gradient, same as sidebar).

### Download Cards

Each card has:
- **Drag handle** (left edge, 28px, 6-dot grip pattern, `cursor: grab`)
- **Left accent bar** (2px, neon primary, only for active downloads, with glow)
- **Body**:
  - Header row: filename (with pulse dot if active) + status badge
  - URL (mono, truncated)
  - Chunk visualization (active) or progress bar (others)
  - Footer: size/progress/ETA left, speed + action buttons right

**Status badge colors**:
- `downloading`: Electric Blue
- `completed`: Cyan
- `failed`: Hot Pink
- `paused`: Amber
- `queued`: muted white/gray

**Glass border tinting**: Card border color shifts to match status color.

**Click to select**: Clicking a card opens the detail panel with that download's info. Selected card gets a brighter border highlight.

---

## 3. Animations & Motion

### CRT Scan Lines
Subtle repeating-linear-gradient overlay (`rgba(0,0,0,0.02)`, 2px/4px), pointer-events: none, z-index: 999.

### Page Transitions
- Page enter: `translateY(8px)` → `0`, opacity 0→1, 250ms, `cubic-bezier(0.16, 1, 0.3, 1)`
- Card enter: staggered via `--i` custom property, 50ms delay per card

### Active Download Pulse
- Status dot: `box-shadow` pulse on primary color, 2s ease-in-out infinite
- Chunk segments: subtle opacity pulse for in-progress chunks

### Progress Bar Shimmer
Active downloads: gradient shimmer animation (200% background-size, 2s linear infinite).

### Sidebar Collapse
- Width transition: 250ms `cubic-bezier(0.16, 1, 0.3, 1)`
- Labels fade out (opacity 0→1, 150ms)

### Detail Panel
- Slide in from right: `translateX(100%)` → `0`, 250ms
- Mobile: same + backdrop fade

### `prefers-reduced-motion`
```css
@media (prefers-reduced-motion: reduce) {
  *, *::before, *::after {
    animation-duration: 0.01ms !important;
    animation-iteration-count: 1 !important;
    transition-duration: 0.01ms !important;
  }
}
```

### Remove Global `*` Transition
Delete the current `* { transition-property: background-color, border-color; }` rule. Apply transitions only to specific elements that need them.

---

## 4. Audit Fixes (Integrated)

All 30 issues from the audit are addressed as part of the redesign. Grouped by category:

### Accessibility (Critical + High)

| ID | Fix |
|----|-----|
| C1 | Add `aria-label` to all icon-only buttons (Pause, Resume, Delete, theme toggle, hamburger) |
| C2 | Dialogs: add `role="dialog"`, `aria-modal="true"`, `aria-labelledby`, focus-trap, auto-focus first interactive element |
| H3 | Replace all `outline-none` with `outline-none focus-visible:ring-2 focus-visible:ring-[var(--neon-primary)]` |
| H4 | Toggle switches: add `role="switch"`, `aria-checked`, `aria-label` |
| H5 | Fix heading hierarchy: page title = h2, sections = h3 |
| H6 | Table (classic view): add `<caption class="sr-only">`, `scope="col"` on th |
| M8 | Filter buttons: wrap in `role="radiogroup"`, each button gets `role="radio"` + `aria-checked` |
| L2 | Add `aria-label="Navigation"` to sidebar, `aria-label="Main content"` to main |
| L6 | Add skip-to-content link (visually hidden, shown on focus) |
| L7 | Dynamic `<title>`: update document.title on page navigation |

### Performance

| ID | Fix |
|----|-----|
| C3 | Remove `* { transition }` rule, apply transitions per-element |
| H7 | CaptchaDialog: move `setInterval` into `onMount` with cleanup return |
| H8 | Sparkline: generate unique gradient ID per instance |
| M2 | Mobile: disable `backdrop-filter` and use opaque backgrounds instead |
| M3 | CaptchaDialog: defer `AudioContext` creation to user gesture |
| L1 | Remove Press Start 2P font load entirely (replaced by Rajdhani) |

### Theming

| ID | Fix |
|----|-----|
| M9 | History: replace `text-green-500` with `var(--neon-success)` |
| M10 | CaptchaDialog: replace `--card-bg` with `var(--bg-surface)` |

### Code Quality

| ID | Fix |
|----|-----|
| M5 | Replace `window.__amigo_report_crash` with Svelte store |
| M6 | Deduplicate `Download` interface — single definition in `stores.ts`, re-export from `api.ts` |
| M7 | Add `svelte:boundary` error boundaries around page components |
| M11 | Deduplicate Escape key handlers — single global handler in App.svelte |

### UX

| ID | Fix |
|----|-----|
| H1 | Increase touch targets to minimum 44x44px on all action buttons |
| M4 | Add skeleton loading states for History, Plugins, Settings pages |
| L3 | Toast container: add `right: calc(1rem + 8px)` offset for scrollbar |
| L5 | Feedback link: increase to 12px, opacity 0.6 |

### Responsive

| ID | Fix |
|----|-----|
| M12 | Sidebar auto-collapses to icon-only below 1024px (inherent in new collapsible design) |
| H2 | Add `prefers-reduced-motion: reduce` media query (see Section 3) |

---

## 5. Component Migration Map

Each existing component maps to the redesign:

| Current File | Changes |
|---|---|
| `app.css` | Complete rewrite: new CSS custom properties, glass tokens, neon palette, remove `*` transition, add scan-lines, add reduced-motion |
| `App.svelte` | New layout (sidebar+main+detail+footer), collapsible sidebar state, remove inline Icon snippet (extract to component), protocol filter state, selected download state, fix M5/M11 |
| `stores.ts` | Add: `sidebarCollapsed`, `selectedDownload`, `protocolFilter` stores. Fix M6. Remove duplicate Download type. |
| `api.ts` | Re-export Download from stores.ts. No other changes. |
| `main.ts` | Update font preconnects (Rajdhani + Share Tech Mono) |
| `Downloads.svelte` | Protocol filter integration, selected state on card click, fix M8 (radiogroup) |
| `DownloadCard.svelte` | Add drag handle, glass styling, neon status badges, click-to-select, fix C1/H1 |
| `DownloadRow.svelte` | Same fixes as DownloadCard for classic view, fix H6 |
| `ChunkViz.svelte` | Neon colors, larger variant for detail panel |
| `Sparkline.svelte` | Fix H8 (unique gradient ID), neon colors |
| `Mascot.svelte` | Replace with `<img>` referencing amigo-downloader.png |
| `AddDialog.svelte` | Glass styling, focus-trap, fix C2, neon buttons |
| `CaptchaDialog.svelte` | Fix M10 (--card-bg), fix H7 (interval cleanup), fix M3 (AudioContext), glass styling, fix C2 |
| `FeedbackDialog.svelte` | Glass styling, fix C2 |
| `DropZone.svelte` | Neon-styled drop overlay with logo |
| `Toasts.svelte` | Neon color bars, glass background, fix L3 |
| `History.svelte` | Fix M9, skeleton loading (M4), neon styling |
| `Plugins.svelte` | Skeleton loading (M4), neon styling, remove Press Start 2P usage |
| `Settings.svelte` | Neon styling, RSS feeds section under Usenet, remove Press Start 2P |
| `RssFeeds.svelte` | Move into Settings/Usenet section (no longer a page) |
| `UsenetDownloads.svelte` | Merged into Downloads via protocol filter |
| `UsenetServers.svelte` | Stays as Settings sub-component |
| `SettingsAppearance.svelte` | New accent picker (3 neon presets), Dark/Lights-On toggle, remove old 6-color picker |
| `SettingsFeatures.svelte` | Neon toggle switches, fix H4 |
| `SettingsUsenet.svelte` | Neon toggle switches, fix H4 |
| `SettingsDownloads.svelte` | Neon input styling, fix H3 |
| `SettingsWebhooks.svelte` | Neon styling |

### New Components

| Component | Purpose |
|---|---|
| `DetailPanel.svelte` | Right-side detail panel (master-detail) |
| `Footer.svelte` | Status bar with sparkline + stats |
| `Icon.svelte` | Extracted from App.svelte inline snippet, adds aria-label support |
| `SkeletonCard.svelte` | Loading skeleton for cards |

### Removed

| Item | Reason |
|---|---|
| `UsenetDownloads.svelte` (page) | Merged into Downloads via protocol segmented control |
| `RssFeeds.svelte` (page) | Moved into Settings as sub-section |
| Inline `Icon` snippet in App.svelte | Extracted to `Icon.svelte` |
| Press Start 2P font | Replaced by Rajdhani |
| Inter font | Replaced by Rajdhani |

---

## 6. File Changes Summary

### index.html
- Replace Google Fonts link: Rajdhani + Share Tech Mono instead of Inter + Press Start 2P
- Copy `amigo-downloader.png` to `web-ui/public/` for static serving
- Update favicon/PWA icons

### New Files
- `web-ui/src/components/DetailPanel.svelte`
- `web-ui/src/components/Footer.svelte`
- `web-ui/src/components/Icon.svelte`
- `web-ui/src/components/SkeletonCard.svelte`
- `web-ui/public/amigo-logo.png` (copy of amigo-downloader.png)

### Deleted Files
- `web-ui/src/pages/UsenetDownloads.svelte` (merged into Downloads)
- `web-ui/src/pages/RssFeeds.svelte` (moved to Settings sub-section)

---

## 7. Implementation Phases

### Phase 1: Foundation
- Rewrite `app.css` with new theme system (CSS custom properties, glass tokens, neon palette, scan-lines, reduced-motion)
- Update `index.html` (fonts, logo assets)
- Update `stores.ts` (new stores, fix M6)
- Extract `Icon.svelte` from App.svelte

### Phase 2: Shell
- Rebuild `App.svelte` (collapsible sidebar, new layout grid, footer area, protocol filter, selected download state)
- Create `Footer.svelte`
- Create `DetailPanel.svelte` (basic structure, responsive slide-over)
- Fix M5 (crash report store), M11 (escape dedup)

### Phase 3: Download Components
- Migrate `DownloadCard.svelte` (glass, drag handles, neon badges, click-to-select, fix C1/H1)
- Migrate `DownloadRow.svelte` (fix H6)
- Migrate `ChunkViz.svelte` (neon, detail variant)
- Migrate `Sparkline.svelte` (fix H8)
- Create `SkeletonCard.svelte`
- Merge UsenetDownloads into Downloads page (protocol filter)

### Phase 4: Dialogs & Overlays
- Migrate `AddDialog.svelte` (glass, focus-trap, fix C2)
- Migrate `CaptchaDialog.svelte` (fix M10, H7, M3, C2)
- Migrate `FeedbackDialog.svelte` (glass, fix C2)
- Migrate `DropZone.svelte` (neon overlay with logo)
- Migrate `Toasts.svelte` (neon, fix L3)

### Phase 5: Pages
- Migrate `Downloads.svelte` (protocol filter, selected state, fix M8)
- Migrate `History.svelte` (fix M9, M4)
- Migrate `Plugins.svelte` (fix M4, remove Press Start 2P)
- Migrate `Settings.svelte` + all sub-components (neon styling, RSS integration, fix H4, H3)
- Replace `Mascot.svelte` with logo image

### Phase 6: Polish
- Add error boundaries (M7)
- Add skip-to-content (L6)
- Dynamic page title (L7)
- Verify all audit fixes
- Test responsive breakpoints
- Test reduced-motion
- Test keyboard navigation end-to-end
- Test both theme modes (Dark + Lights On)
