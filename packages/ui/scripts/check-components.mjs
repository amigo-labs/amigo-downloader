#!/usr/bin/env node
/**
 * Compile every primitive and fail on any non-CSS warning.
 *
 * The primitives live in a package that nothing type-checks directly, so
 * without this they could sit broken until the first consumer imports them.
 * Unused-CSS warnings are ignored: a component's styles legitimately cover
 * variants the compiler cannot see used from inside the file.
 */
import { compile } from "svelte/compiler";
import { readFileSync, readdirSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const dir = join(dirname(fileURLToPath(import.meta.url)), "..", "src", "components");
let bad = 0;
let total = 0;

for (const file of readdirSync(dir).filter((f) => f.endsWith(".svelte")).sort()) {
  total++;
  try {
    const { warnings } = compile(readFileSync(join(dir, file), "utf8"), {
      filename: file,
      generate: "client",
    });
    const real = warnings.filter((w) => !w.code.startsWith("css_unused"));
    if (real.length) {
      bad++;
      console.error(`WARN ${file}`);
      for (const w of real) console.error(`  ${w.code}: ${w.message}`);
    }
  } catch (e) {
    bad++;
    console.error(`FAIL ${file}: ${e.message}`);
  }
}

if (bad) {
  console.error(`\ncomponents: ${bad} of ${total} have problems`);
  process.exit(1);
}
console.log(`components: ${total} primitives compile clean`);
