#!/usr/bin/env node
/**
 * UI lint: hard bans plus a ratchet budget for inline styles.
 *
 * The repo has no ESLint, so this is a standalone Node script. Its job is to
 * stop the specific regressions this codebase already suffered once:
 *
 *   - z-index picked by hand at each call site, which produced two overlays
 *     tied at 100 and a modal above the skip link,
 *   - decorative accent tokens used as a text colour, which is what put light
 *     mode at 1.85:1,
 *   - icon-only buttons with no accessible name,
 *   - radiogroups that announce a keyboard contract they do not implement,
 *   - and the slow drift back to inline styles instead of primitives.
 *
 * The inline-style budget only ever ratchets down: a file may not gain new
 * `style="` attributes, and a file that loses some must record the new number.
 */

import { readFileSync, writeFileSync, readdirSync, statSync } from "node:fs";
import { join, relative } from "node:path";

const ROOT = process.cwd();
const BUDGET_PATH = join(ROOT, "scripts", "ui-budget.json");
const SCAN_DIRS = ["web-ui/src", "packages/ui/src"];
const UPDATE = process.argv.includes("--update");

const DISABLE = /ui-lint-disable-next-line\s+([\w-]+)/;

/** @type {{id: string, test: RegExp | ((line: string) => boolean), message: string}[]} */
const RULES = [
  {
    id: "no-manual-z-index",
    // Tailwind arbitrary z utilities, or a raw z-index of 10 or more. Single
    // digits are local stacking inside a component's own context (an overlay
    // above its own siblings), which is not global layering.
    test: (line) => {
      if (/\bz-\[/.test(line)) return true;
      const m = /\bz-index:\s*(\d+)/.exec(line);
      return !!m && Number(m[1]) >= 10;
    },
    message:
      "z-index belongs to the --z-* scale in tokens.css. Use `z-index: var(--z-modal)` or a layer from lib/overlays.",
  },
  {
    id: "no-decorative-token-as-text",
    test: /(?<![-\w])color:\s*var\(--(?:accent|success|warning|danger|neon-primary|neon-success|neon-warning|neon-accent)\)/,
    message:
      "Decorative accent tokens fail WCAG AA as text. Use the -ink variant (--accent-ink, --danger-ink, ...).",
  },
  {
    id: "no-raw-hex",
    test: /(?:^|[\s;"'{])(?:color|background|background-color|border-color|fill|stroke):\s*#[0-9a-fA-F]{3,8}\b/,
    message: "Raw hex colours bypass theming. Use a token from @amigo/ui/tokens.css.",
  },
  {
    id: "no-legacy-radius",
    test: /\brounded-2xl\b/,
    message: "rounded-2xl duplicates the --radius-lg role. Use rounded-xl or var(--radius-lg).",
  },
  {
    id: "no-undefined-tokens",
    test: /var\(--(?:surface|surface-2|muted|fg|bg|border|accent),/,
    message:
      "These token names do not exist; the fallback silently wins. The real names are --bg-surface, --bg-surface-2, --text-secondary, --text-primary, --bg-deep, --border-color.",
  },
];

function walk(dir) {
  const out = [];
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry);
    if (statSync(full).isDirectory()) out.push(...walk(full));
    else if (/\.(svelte|css|ts)$/.test(entry) && !entry.endsWith(".d.ts")) out.push(full);
  }
  return out;
}

/** Icon-only buttons must carry an accessible name. */
function checkIconButtons(text, file) {
  const problems = [];
  const re = /<button\b([^>]*)>([\s\S]*?)<\/button>/g;
  let m;
  while ((m = re.exec(text)) !== null) {
    const [, attrs, body] = m;
    const onlyIcon = /^\s*(\{#if[\s\S]*?\})?\s*<Icon\b[^>]*\/>\s*(\{\/if\})?\s*$/.test(body);
    if (!onlyIcon) continue;
    if (/aria-label|aria-labelledby/.test(attrs)) continue;
    problems.push({
      line: text.slice(0, m.index).split("\n").length,
      id: "icon-button-needs-label",
      message:
        "An icon-only <button> has no accessible name. Use <IconButton>, whose `label` prop is required.",
    });
  }
  return problems;
}

/** A radiogroup that never handles arrow keys is an unfulfilled contract. */
function checkRadiogroups(text) {
  const problems = [];
  const re = /role=["']radiogroup["']/g;
  let m;
  while ((m = re.exec(text)) !== null) {
    // Whole-file check: SegmentedControl's key handler lives in <script>,
    // above the markup.
    if (/onkeydown/.test(text)) continue;
    problems.push({
      line: text.slice(0, m.index).split("\n").length,
      id: "radiogroup-needs-keys",
      message:
        "role=radiogroup promises Arrow/Home/End navigation. Use <SegmentedControl> or add onkeydown.",
    });
  }
  return problems;
}

const files = SCAN_DIRS.flatMap((d) => walk(join(ROOT, d))).sort();
const failures = [];
const counts = {};

for (const file of files) {
  const rel = relative(ROOT, file).replaceAll("\\", "/");
  const text = readFileSync(file, "utf8");
  const lines = text.split("\n");

  // tokens.css is where the scale is *defined*.
  const isTokenFile = rel.endsWith("packages/ui/src/tokens.css");

  lines.forEach((line, i) => {
    const trimmed = line.trim();
    // Comment lines describe the rules; they do not break them.
    if (trimmed.startsWith("//") || trimmed.startsWith("*") || trimmed.startsWith("/*")) return;
    const prev = lines[i - 1] ?? "";
    const disabled = DISABLE.exec(prev)?.[1];
    for (const rule of RULES) {
      if (isTokenFile && (rule.id === "no-manual-z-index" || rule.id === "no-raw-hex" || rule.id === "no-decorative-token-as-text")) continue;
      if (disabled === rule.id) continue;
      const hit = typeof rule.test === "function" ? rule.test(line) : rule.test.test(line);
      if (hit) {
        failures.push(`${rel}:${i + 1}  [${rule.id}] ${rule.message}\n    ${line.trim()}`);
      }
    }
  });

  if (rel.endsWith(".svelte")) {
    for (const p of [...checkIconButtons(text, rel), ...checkRadiogroups(text)]) {
      const prev = lines[p.line - 2] ?? "";
      if (DISABLE.exec(prev)?.[1] === p.id) continue;
      failures.push(`${rel}:${p.line}  [${p.id}] ${p.message}`);
    }
    const inline = (text.match(/style="/g) ?? []).length;
    if (inline > 0) counts[rel] = inline;
  }
}

// ---- inline-style ratchet -------------------------------------------------
let budget = { total: 0, perFile: {} };
try {
  budget = JSON.parse(readFileSync(BUDGET_PATH, "utf8"));
} catch {
  if (!UPDATE) {
    console.error(`missing ${relative(ROOT, BUDGET_PATH)} — run with --update to create it`);
    process.exit(1);
  }
}

const total = Object.values(counts).reduce((a, b) => a + b, 0);

if (UPDATE) {
  writeFileSync(BUDGET_PATH, JSON.stringify({ total, perFile: counts }, null, 2) + "\n");
  console.log(`ui-lint: budget updated — ${total} inline styles across ${Object.keys(counts).length} files`);
} else {
  for (const [file, n] of Object.entries(counts)) {
    const allowed = budget.perFile[file];
    if (allowed === undefined) {
      failures.push(
        `${file}  [inline-style-budget] new file with ${n} inline style attributes. ` +
        `Build it from @amigo/ui primitives, or run \`node scripts/ui-lint.mjs --update\` if this is intentional.`,
      );
    } else if (n > allowed) {
      failures.push(
        `${file}  [inline-style-budget] inline styles went ${allowed} -> ${n}. ` +
        `The budget only ratchets down.`,
      );
    }
  }
  if (total < budget.total) {
    console.log(
      `ui-lint: inline styles ${budget.total} -> ${total}. Run \`node scripts/ui-lint.mjs --update\` to lock the improvement in.`,
    );
  }
}

if (failures.length) {
  console.error(`\nui-lint: ${failures.length} problem(s)\n`);
  for (const f of failures) console.error(`  ${f}`);
  console.error("");
  process.exit(1);
}

console.log(`ui-lint: clean (${files.length} files, ${total} inline styles remaining)`);
