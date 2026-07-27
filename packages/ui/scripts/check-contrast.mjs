#!/usr/bin/env node
/**
 * Contrast checker for the @amigo/ui design tokens.
 *
 * Parses `src/tokens.css` as text, resolves the cascade for every
 * (palette x theme) combination, evaluates `var()` / `color-mix()` /
 * `rgba()` itself, and asserts the WCAG 2.1 ratios declared in
 * `src/tokens.spec.json`.
 *
 * Why text-parsing rather than a headless browser: the token file is flat
 * by construction (only `:root`-rooted selectors carry custom properties),
 * so a scanner is exact here and keeps CI free of a browser dependency.
 *
 *   node scripts/check-contrast.mjs             # assert, exit 1 on failure
 *   node scripts/check-contrast.mjs --report    # markdown matrix to stdout
 */

import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const HERE = dirname(fileURLToPath(import.meta.url));
const TOKENS = join(HERE, "..", "src", "tokens.css");
const SPEC = join(HERE, "..", "src", "tokens.spec.json");

const PALETTES = ["blue", "teal", "indigo", "amber", "violet", "rose"];
const THEMES = ["dark", "light"];

/* ------------------------------------------------------------------ *
 * Colour maths
 * ------------------------------------------------------------------ */

const NAMED = {
  transparent: { r: 0, g: 0, b: 0, a: 0 },
  white: { r: 255, g: 255, b: 255, a: 1 },
  black: { r: 0, g: 0, b: 0, a: 1 },
};

/** Parse a resolved colour literal into {r,g,b,a}. Returns null if not one. */
function parseColor(input) {
  const v = String(input).trim().toLowerCase();
  if (NAMED[v]) return { ...NAMED[v] };

  if (v.startsWith("#")) {
    const h = v.slice(1);
    const expand = (s) => parseInt(s.length === 1 ? s + s : s, 16);
    if (h.length === 3 || h.length === 4) {
      return {
        r: expand(h[0]), g: expand(h[1]), b: expand(h[2]),
        a: h.length === 4 ? expand(h[3]) / 255 : 1,
      };
    }
    if (h.length === 6 || h.length === 8) {
      return {
        r: parseInt(h.slice(0, 2), 16),
        g: parseInt(h.slice(2, 4), 16),
        b: parseInt(h.slice(4, 6), 16),
        a: h.length === 8 ? parseInt(h.slice(6, 8), 16) / 255 : 1,
      };
    }
    return null;
  }

  const fn = v.match(/^rgba?\(([^)]+)\)$/);
  if (fn) {
    const parts = fn[1].split(/[\s,/]+/).filter(Boolean).map(Number);
    if (parts.length < 3 || parts.some(Number.isNaN)) return null;
    return { r: parts[0], g: parts[1], b: parts[2], a: parts.length > 3 ? parts[3] : 1 };
  }

  return null;
}

/** Composite a possibly-translucent colour over an opaque backdrop. */
function over(fg, bg) {
  if (fg.a >= 1) return { ...fg, a: 1 };
  return {
    r: fg.r * fg.a + bg.r * (1 - fg.a),
    g: fg.g * fg.a + bg.g * (1 - fg.a),
    b: fg.b * fg.a + bg.b * (1 - fg.a),
    a: 1,
  };
}

/** sRGB channel -> linear. */
function lin(c) {
  const s = c / 255;
  return s <= 0.04045 ? s / 12.92 : Math.pow((s + 0.055) / 1.055, 2.4);
}

function luminance({ r, g, b }) {
  return 0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b);
}

function contrast(fg, bg) {
  const a = luminance(fg) + 0.05;
  const b = luminance(bg) + 0.05;
  return a > b ? a / b : b / a;
}

function hex({ r, g, b }) {
  const p = (n) => Math.round(n).toString(16).padStart(2, "0");
  return `#${p(r)}${p(g)}${p(b)}`;
}

/* ------------------------------------------------------------------ *
 * Token file parsing
 * ------------------------------------------------------------------ */

/**
 * Extract every `:root...{ ... }` rule in source order.
 * The token file never nests custom properties inside another rule, so a
 * flat scan for brace-delimited blocks whose selector starts at `:root` is
 * exhaustive.
 */
function parseRules(source) {
  // Strip comments first — an inline `/* ... */` ahead of a declaration would
  // otherwise be glued onto the property name and silently drop the token.
  const css = source.replace(/\/\*[\s\S]*?\*\//g, "");
  const rules = [];
  const re = /(^|[}\s])(:root[^{}]*?)\{([^{}]*)\}/g;
  let m;
  while ((m = re.exec(css)) !== null) {
    const selector = m[2].trim();
    const decls = {};
    for (const d of m[3].split(";")) {
      const i = d.indexOf(":");
      if (i === -1) continue;
      const prop = d.slice(0, i).trim();
      if (!prop.startsWith("--")) continue;
      decls[prop] = d.slice(i + 1).trim();
    }
    if (Object.keys(decls).length) rules.push({ selector, decls });
  }
  return rules;
}

/** Does this selector apply to the given theme/palette? */
function applies(selector, theme, palette) {
  const classes = [...selector.matchAll(/\.([\w-]+)/g)].map((m) => m[1]);
  for (const c of classes) {
    if (c === "light") {
      if (theme !== "light") return false;
    } else if (c.startsWith("palette-")) {
      if (c !== `palette-${palette}`) return false;
    } else {
      return false; // unknown modifier — not part of the theme matrix
    }
  }
  return true;
}

/** Flatten the cascade into one {prop: rawValue} map. */
function resolveTheme(rules, theme, palette) {
  const applicable = rules
    .filter((r) => applies(r.selector, theme, palette))
    .map((r, i) => ({ ...r, spec: (r.selector.match(/\./g) || []).length, order: i }));

  applicable.sort((a, b) => (a.spec - b.spec) || (a.order - b.order));

  const out = {};
  for (const r of applicable) Object.assign(out, r.decls);
  return out;
}

/* ------------------------------------------------------------------ *
 * Value resolution: var() and color-mix()
 * ------------------------------------------------------------------ */

/** Split a comma-separated argument list, respecting nested parentheses. */
function splitArgs(s) {
  const out = [];
  let depth = 0, cur = "";
  for (const ch of s) {
    if (ch === "(") depth++;
    if (ch === ")") depth--;
    if (ch === "," && depth === 0) { out.push(cur.trim()); cur = ""; continue; }
    cur += ch;
  }
  if (cur.trim()) out.push(cur.trim());
  return out;
}

/** Find the body of the first `name(...)` call in `s`, or null. */
function callBody(s, name) {
  const start = s.indexOf(name + "(");
  if (start === -1) return null;
  let depth = 0;
  for (let i = start + name.length; i < s.length; i++) {
    if (s[i] === "(") depth++;
    else if (s[i] === ")") {
      depth--;
      if (depth === 0) return { start, end: i + 1, body: s.slice(start + name.length + 1, i) };
    }
  }
  return null;
}

class Unresolvable extends Error {}

/**
 * Resolve a token to a concrete colour. Throws Unresolvable rather than
 * silently skipping — a token that the checker cannot evaluate is a token
 * whose contrast nobody is verifying.
 */
function resolve(name, vars, seen = new Set()) {
  if (seen.has(name)) throw new Unresolvable(`circular reference at ${name}`);
  seen.add(name);
  const raw = vars[name];
  if (raw === undefined) throw new Unresolvable(`undefined token ${name}`);
  return evaluate(raw, vars, seen);
}

function evaluate(expr, vars, seen) {
  let s = String(expr).trim();

  // var(--x) / var(--x, fallback)
  for (let guard = 0; guard < 32; guard++) {
    const call = callBody(s, "var");
    if (!call) break;
    const args = splitArgs(call.body);
    const ref = args[0];
    let value;
    try {
      value = hexOf(resolve(ref, vars, new Set(seen)));
    } catch (e) {
      if (args.length > 1) value = args.slice(1).join(",");
      else throw e;
    }
    s = s.slice(0, call.start) + value + s.slice(call.end);
  }

  // color-mix(in srgb, A P%, B Q%)
  const mix = callBody(s, "color-mix");
  if (mix) {
    const args = splitArgs(mix.body);
    if (args.length < 3 || !/^in\s+srgb$/i.test(args[0])) {
      throw new Unresolvable(`unsupported color-mix: ${s}`);
    }
    const one = parseMixOperand(args[1], vars, seen);
    const two = parseMixOperand(args[2], vars, seen);
    let p1 = one.pct, p2 = two.pct;
    if (p1 === null && p2 === null) { p1 = 50; p2 = 50; }
    else if (p1 === null) p1 = 100 - p2;
    else if (p2 === null) p2 = 100 - p1;
    const total = p1 + p2;
    if (total === 0) throw new Unresolvable(`zero-weight color-mix: ${s}`);
    const w1 = p1 / total, w2 = p2 / total;
    // Premultiplied-alpha mix, per css-color-5.
    const a = one.color.a * w1 + two.color.a * w2;
    const ch = (k) =>
      a === 0 ? 0 : (one.color[k] * one.color.a * w1 + two.color[k] * two.color.a * w2) / a;
    return { r: ch("r"), g: ch("g"), b: ch("b"), a };
  }

  const c = parseColor(s);
  if (!c) throw new Unresolvable(`not a colour: "${expr}" -> "${s}"`);
  return c;
}

function parseMixOperand(arg, vars, seen) {
  const m = arg.match(/^(.*?)\s+([\d.]+)%$/);
  const colorPart = m ? m[1] : arg;
  const pct = m ? parseFloat(m[2]) : null;
  return { color: evaluate(colorPart, vars, seen), pct };
}

function hexOf(c) {
  if (c.a >= 1) return hex(c);
  const p = (n) => Math.round(n).toString(16).padStart(2, "0");
  return `#${p(c.r)}${p(c.g)}${p(c.b)}${p(c.a * 255)}`;
}

/* ------------------------------------------------------------------ *
 * Self-test of the maths (a broken checker must not pass everything)
 * ------------------------------------------------------------------ */

function selfTest() {
  const fixtures = [
    ["#000000", "#ffffff", 21.0],
    ["#777777", "#ffffff", 4.48],
    ["#767676", "#ffffff", 4.54],
    ["#3b82f6", "#ffffff", 3.68],
  ];
  for (const [fg, bg, expected] of fixtures) {
    const got = contrast(parseColor(fg), parseColor(bg));
    if (Math.abs(got - expected) > 0.02) {
      console.error(`contrast self-test failed: ${fg} on ${bg} = ${got.toFixed(2)}, expected ${expected}`);
      process.exit(2);
    }
  }
}

/* ------------------------------------------------------------------ *
 * Main
 * ------------------------------------------------------------------ */

function main() {
  selfTest();

  const report = process.argv.includes("--report");
  const css = readFileSync(TOKENS, "utf8");
  const spec = JSON.parse(readFileSync(SPEC, "utf8"));
  const rules = parseRules(css);

  const failures = [];
  const rows = [];
  let checks = 0;

  for (const theme of THEMES) {
    for (const palette of PALETTES) {
      const vars = resolveTheme(rules, theme, palette);

      for (const entry of spec) {
        let fg;
        try {
          fg = resolve(entry.fg, vars);
        } catch (e) {
          failures.push(`${theme}/${palette}: cannot resolve ${entry.fg} — ${e.message}`);
          continue;
        }

        for (const bgName of entry.bg) {
          let bg;
          try {
            bg = resolve(bgName, vars);
          } catch (e) {
            failures.push(`${theme}/${palette}: cannot resolve ${bgName} — ${e.message}`);
            continue;
          }

          // A translucent background is composited over the page backdrop.
          const page = resolve("--bg-deep", vars);
          const bgSolid = over(bg, page);
          const fgSolid = over(fg, bgSolid);
          const ratio = contrast(fgSolid, bgSolid);
          checks++;

          const ok = ratio >= entry.min - 0.005;
          rows.push({ theme, palette, fg: entry.fg, bg: bgName, ratio, min: entry.min, ok });
          if (!ok) {
            failures.push(
              `${theme}/${palette}: ${entry.fg} ${hex(fgSolid)} on ${bgName} ${hex(bgSolid)} ` +
              `= ${ratio.toFixed(2)} (need ${entry.min.toFixed(2)})`
            );
          }
        }
      }
    }
  }

  if (report) {
    console.log("| theme | palette | foreground | background | ratio | min | |");
    console.log("|---|---|---|---|---|---|---|");
    for (const r of rows) {
      console.log(
        `| ${r.theme} | ${r.palette} | \`${r.fg}\` | \`${r.bg}\` | ${r.ratio.toFixed(2)} | ${r.min} | ${r.ok ? "ok" : "FAIL"} |`
      );
    }
    console.log("");
  }

  if (failures.length) {
    console.error(`\ncontrast: ${failures.length} of ${checks} assertions failed\n`);
    for (const f of failures) console.error(`  ${f}`);
    console.error("");
    process.exit(1);
  }

  console.log(`contrast: ${checks} assertions passed (${PALETTES.length} palettes x ${THEMES.length} themes)`);
}

main();
