---
name: spec-verify
description: Verify spec compliance and project consistency. Pass a spec name to check just that spec, or run without arguments for general consistency checks. Use "all" to run everything.
argument-hint: "[spec-name | all]"
---

# Spec Verification

## Mode Selection

Based on the argument, run in one of three modes:

- **`/spec-verify <name>`** — Fast: verify only spec `docs/specs/<name>.md` against its affected files
- **`/spec-verify`** (no argument) — Medium: run general consistency checks (no spec compliance)
- **`/spec-verify all`** — Full: all consistency checks + all specs. Use before releases.

---

## Mode 1: Single Spec Verification (`/spec-verify <name>`)

Read `docs/specs/$0.md` and verify only what that spec covers:

1. **Acceptance Criteria** — For each AC, check if the code satisfies it
   - Read the affected files listed in the spec
   - Verify the described behavior exists in the implementation
   - Report: PASS/FAIL per AC with evidence (file:line)

2. **Test Coverage** — Check the spec's test plan
   - Every test plan item should have a corresponding test function
   - Tests should actually test what the AC describes (not just exist)
   - Report: missing tests, tests that don't match their AC

3. **API Contract** — If spec defines endpoints/types
   - Verify Rust types exist as specified
   - Verify endpoints are registered in the router
   - Verify frontend API calls match
   - Report: mismatches between spec and implementation

4. **Affected Files** — Verify the spec's file list
   - Were all listed files actually modified?
   - Were files changed that aren't listed in the spec? (potential spec gap)

5. **Update the spec file** based on verification results:
   - Set `[x]` for acceptance criteria that are verified as met
   - Reset `[ ]` for acceptance criteria that are no longer met (regression)
   - Update the `status` field at the top of the spec:
     - `status: verified` — all ACs pass
     - `status: partial` — some ACs pass, some fail
     - `status: failing` — most/all ACs fail
     - `status: draft` — not yet implemented (no matching code found)

Output: Focused report for this spec + updated spec file.

---

## Mode 2: General Consistency (`/spec-verify` without argument)

Run these checks in parallel using Explore agents:

### 1. Type Consistency (Frontend <-> Backend)
Compare Rust API types in `crates/server/src/api.rs` with TypeScript types in `web-ui/src/lib/api.ts`:
- Every Rust response struct should have a matching TypeScript interface
- Field names and types must match (snake_case Rust -> camelCase TS via serde)
- Report: mismatches, missing types, extra types

### 2. API Completeness
- Scan `crates/server/src/api.rs` for all route definitions
- Compare against REST API section in `CLAUDE.md`
- Check that every endpoint has a corresponding frontend API call
- Report: undocumented endpoints, documented but unimplemented, frontend calls to missing endpoints

### 3. Test Coverage
- Scan `pub fn` and `pub async fn` in `crates/*/src/**/*.rs`
- Check for corresponding `#[test]` or `#[tokio::test]`
- Report: untested public functions grouped by crate

### 4. i18n Completeness
- Compare keys in `locales/en.json` vs `locales/de.json`
- Check i18n keys used in web-ui exist in locale files
- Report: missing keys, unused keys

### 5. Frontend <-> Backend API Sync
- Extract all API calls from `web-ui/src/lib/api.ts`
- Compare against route definitions in `crates/server/src/api.rs`
- Check WebSocket events in `crates/server/src/ws.rs` vs `web-ui/src/lib/stores.ts`
- Report: mismatched calls, missing endpoints

### 6. Documentation Freshness
- Compare CLAUDE.md "Repository-Struktur" against actual file tree
- Report: stale docs, missing entries, phantom entries

---

## Mode 3: Full Verification (`/spec-verify all`)

Run Mode 2 (general consistency) PLUS Mode 1 for every spec in `docs/specs/`:

```
For each *.md in docs/specs/:
  Run single spec verification
```

Compile into one unified report.

---

## Output Format

```markdown
# Verification Report

## Summary
| Check | Status | Issues |
|-------|--------|--------|
| ... | PASS/WARN/FAIL | N issues |

## Details

### [Check Name] — STATUS
- Issue 1: description + file:line
- **Action needed**: what to fix

## Priority Actions
1. [Most critical fix]
2. ...
```

## Rules
- **Read-only EXCEPT for spec files** — only update `[ ]`/`[x]` checkboxes and `status` field in `docs/specs/*.md`
- Be specific: include file paths and line numbers
- FAIL = broken/wrong, WARN = missing/incomplete, PASS = verified correct
- If a check cannot be performed, report as SKIP with reason
- Keep it fast: only read files that are relevant to the scope

## Context

Available specs:
!`ls /home/user/amigo-downloader/docs/specs/*.md 2>/dev/null || echo "No specs yet"`
