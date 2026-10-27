#!/usr/bin/env node
/**
 * Version consistency: keeps the release pipeline's assumptions true.
 *
 * `release-please-config.json` uses the `simple` release type and bumps the
 * version by hand through `extra-files`, because the `rust` release type
 * cannot handle this repo: it writes a literal `version` into every member's
 * `[package]` table and into the root `Cargo.toml`, while this workspace is
 * virtual and every crate inherits `version.workspace = true`.
 *
 * That trade is only safe while the invariants below hold, and none of them
 * is enforced by cargo or by release-please itself:
 *
 *   - `[workspace.package].version` is the single source of truth,
 *   - every workspace member inherits it instead of pinning its own version
 *     (a pinned one would silently stay behind at every release),
 *   - the `amigo-*` entries in `Cargo.lock` match it, because CI builds with
 *     `--locked` and a stale lockfile fails the release PR,
 *   - the JSON files that carry the same version agree with it, and
 *   - every one of those files is actually listed in `extra-files`.
 */

import { readFileSync } from "node:fs";
import { join } from "node:path";

const ROOT = process.cwd();
const read = (p) => readFileSync(join(ROOT, p), "utf8").replace(/\r\n/g, "\n");

/** Body of a top-level TOML table, excluding the header line. */
function section(toml, header) {
  const start = toml.indexOf(`[${header}]\n`);
  if (start === -1) return null;
  const body = toml.slice(start + header.length + 3);
  const end = body.search(/^\[/m);
  return end === -1 ? body : body.slice(0, end);
}

const errors = [];
const fail = (msg) => errors.push(msg);

const rootToml = read("Cargo.toml");

const workspacePackage = section(rootToml, "workspace.package");
const expected = workspacePackage && /^version\s*=\s*"([^"]+)"/m.exec(workspacePackage)?.[1];
if (!expected) {
  console.error("Cargo.toml: no version in [workspace.package] — nothing to check against");
  process.exit(1);
}

// Every member inherits the workspace version.
const membersBlock = /members\s*=\s*\[([^\]]*)\]/.exec(section(rootToml, "workspace") ?? "");
const members = [...(membersBlock?.[1] ?? "").matchAll(/"([^"]+)"/g)].map((m) => m[1]);
if (members.length === 0) fail("Cargo.toml: could not read [workspace].members");

const crateNames = new Set();
for (const member of members) {
  const manifest = read(`${member}/Cargo.toml`);
  const pkg = section(manifest, "package");
  if (!pkg) {
    fail(`${member}/Cargo.toml: no [package] table`);
    continue;
  }
  crateNames.add(/^name\s*=\s*"([^"]+)"/m.exec(pkg)?.[1] ?? member);
  if (!/^version\s*=\s*\{?\s*workspace\s*[=.]/m.test(pkg) && !/^version\.workspace\s*=\s*true/m.test(pkg)) {
    fail(`${member}/Cargo.toml: must inherit the version (version.workspace = true), not pin its own`);
  }
}

// Cargo.lock carries a copy of every workspace member's version.
const lockedCrates = new Set();
for (const block of read("Cargo.lock").split("[[package]]").slice(1)) {
  const name = /^name\s*=\s*"([^"]+)"/m.exec(block)?.[1];
  if (!name?.startsWith("amigo-")) continue;
  lockedCrates.add(name);
  const version = /^version\s*=\s*"([^"]+)"/m.exec(block)?.[1];
  if (version !== expected) fail(`Cargo.lock: ${name} is ${version}, expected ${expected}`);
}
for (const name of crateNames) {
  if (!lockedCrates.has(name)) fail(`Cargo.lock: no entry for workspace member ${name}`);
}

// JSON files that restate the same version.
const JSON_FILES = ["web-ui/package.json", "tauri/tauri.conf.json"];
for (const path of JSON_FILES) {
  const version = JSON.parse(read(path)).version;
  if (version !== expected) fail(`${path}: version is ${version}, expected ${expected}`);
}

// release-please has to know about every file checked above.
const configured = new Set(
  JSON.parse(read("release-please-config.json")).packages["."]["extra-files"].map((f) =>
    typeof f === "string" ? f : f.path,
  ),
);
for (const path of ["Cargo.toml", "Cargo.lock", ...JSON_FILES]) {
  if (!configured.has(path)) fail(`release-please-config.json: ${path} is missing from extra-files`);
}

if (errors.length > 0) {
  console.error(`Version consistency failed (source of truth: ${expected})\n`);
  for (const error of errors) console.error(`  ${error}`);
  process.exit(1);
}

console.log(`Version consistency OK: ${expected} across Cargo.toml, Cargo.lock, ${JSON_FILES.join(", ")}`);
