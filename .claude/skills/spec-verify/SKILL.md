---
name: spec-verify
description: Verify a spec against the codebase and update its status. Without argument runs a general project consistency check. Use "all" for full audit before releases.
argument-hint: "[spec-name | all]"
---

# Spec Verification

## Mode

- **`/spec-verify <name>`** — Verify one spec, update its status (fast)
- **`/spec-verify`** — General project consistency check (medium)
- **`/spec-verify all`** — Both of the above for every spec (slow, for releases)

---

## Single Spec (`/spec-verify <name>`)

Read `docs/specs/$0.md` and check:

1. **Acceptance Criteria** — For each AC, verify the code satisfies it. Report PASS/FAIL with evidence (file:line).
2. **Test Coverage** — Every AC should have a corresponding passing test.
3. **API Contract** — If spec defines types/endpoints: verify they exist and match.

Then **update the spec file**:
- `[x]` for ACs that pass, `[ ]` for ACs that fail (regression)
- Update `status`: `verified` (all pass) / `partial` (some pass) / `failing` (most fail) / `draft` (not implemented)

Output: Short report + updated spec.

---

## General Consistency (`/spec-verify` without argument)

Run in parallel:

1. **Type Sync** — Rust API types vs TypeScript types in `web-ui/src/lib/api.ts`
2. **API Sync** — Route definitions vs CLAUDE.md docs vs frontend API calls
3. **Test Coverage** — Public functions without tests, grouped by crate
4. **i18n** — Missing keys between `locales/en.json` and `locales/de.json`

Output: Summary table (PASS/WARN/FAIL per check) + action items.

---

## Full (`/spec-verify all`)

Run general consistency + verify every spec in `docs/specs/`.

---

## Output Format

```markdown
# Verification Report

## Summary
| Check | Status | Issues |
|-------|--------|--------|

## Details
### [Check] — STATUS
- file:line — description
- **Action**: what to fix
```

## Rules
- Read-only except for spec status updates (`[ ]`/`[x]`, `status` field)
- FAIL = broken, WARN = missing/incomplete, PASS = verified
- Be specific: file paths + line numbers
- Skip checks that can't be performed (report as SKIP)

## Context

Available specs:
!`ls /home/user/amigo-downloader/docs/specs/*.md 2>/dev/null || echo "No specs yet"`
