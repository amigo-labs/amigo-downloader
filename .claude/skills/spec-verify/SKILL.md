---
name: spec-verify
description: Run a comprehensive consistency check across the entire project. Verifies type sync between frontend and backend, test coverage, i18n completeness, documentation accuracy, and spec compliance. Use after implementing a feature or before a release.
argument-hint: "[scope: all|types|tests|i18n|api|docs|specs]"
---

# Consistency Verification Check

Run a comprehensive consistency check across the amigo-downloader project. Scope: **$ARGUMENTS** (default: all).

## Checks to Perform

Launch parallel Explore agents for independent checks, then compile a unified report.

### 1. Type Consistency (Frontend <-> Backend)

Compare Rust API types in `crates/server/src/api.rs` with TypeScript types in `web-ui/src/lib/api.ts`:
- Every Rust response struct should have a matching TypeScript interface
- Field names and types must match (snake_case Rust -> camelCase TS via serde)
- Enum variants must be in sync
- Report: mismatches, missing types, extra types

### 2. API Completeness

Compare implemented endpoints vs documented endpoints:
- Scan `crates/server/src/api.rs` for all route definitions
- Compare against REST API section in `CLAUDE.md`
- Check that every endpoint has a corresponding frontend API call in `web-ui/src/lib/api.ts`
- Report: undocumented endpoints, documented but unimplemented endpoints, frontend calls to non-existent endpoints

### 3. Test Coverage

Identify public functions without tests:
- Scan all `pub fn` and `pub async fn` in `crates/*/src/**/*.rs`
- Check for corresponding `#[test]` or `#[tokio::test]` in the same file or in `tests/`
- Pay special attention to: coordinator, chunk, bandwidth, queue, retry, protocol backends
- Report: untested public functions grouped by crate, coverage percentage estimate

### 4. i18n Completeness

Compare locale files:
- Parse `locales/en.json` and `locales/de.json`
- Find keys present in one but missing in the other
- Check that all i18n keys used in web-ui (`$t(...)` or equivalent) exist in locale files
- Report: missing keys per locale, unused keys, keys used in code but not in locale files

### 5. Frontend <-> Backend API Sync

Verify frontend API calls match actual backend:
- Extract all fetch/API calls from `web-ui/src/lib/api.ts`
- Extract all route definitions from `crates/server/src/api.rs`
- Verify HTTP methods, paths, and expected response shapes match
- Check WebSocket event names in `crates/server/src/ws.rs` vs `web-ui/src/lib/stores.ts`
- Report: mismatched calls, unused endpoints, frontend calls to missing endpoints

### 6. Spec Compliance

If `docs/specs/` contains specs:
- For each spec, check acceptance criteria against actual implementation
- Verify that listed files were actually modified
- Check that test plan items have corresponding test implementations
- Report: unmet acceptance criteria, missing tests from test plan

### 7. Documentation Freshness

Verify CLAUDE.md accuracy:
- Compare "Repository-Struktur" section against actual file tree
- Check that listed files actually exist
- Check for files that exist but aren't listed
- Verify tech stack versions match Cargo.toml/package.json
- Report: stale docs, missing entries, phantom entries

## Output Format

Present results as a structured report:

```markdown
# Verification Report

## Summary
| Check | Status | Issues |
|-------|--------|--------|
| Type Consistency | PASS/WARN/FAIL | N issues |
| API Completeness | ... | ... |
| ... | ... | ... |

## Details

### [Check Name] — STATUS
- Issue 1: description + file:line
- Issue 2: ...
- **Action needed**: what to fix

## Priority Actions
1. [Most critical fix]
2. [Next most critical]
...
```

## Rules
- Do NOT make any changes — this is a read-only audit
- Be specific: include file paths and line numbers
- Distinguish between FAIL (broken/wrong) and WARN (missing/incomplete)
- PASS means verified correct, not just "didn't check"
- If a check cannot be performed (e.g., no specs exist), report as SKIP with reason
