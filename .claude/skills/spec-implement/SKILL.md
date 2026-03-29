---
name: spec-implement
description: Implement a feature based on an existing spec from docs/specs/. Reads the spec, implements in 3 gated phases (contract, code, verify+commit). Use after /spec.
argument-hint: "<feature-name>"
---

# Implement Spec: $ARGUMENTS

## Prerequisites

1. Read `docs/specs/$0.md` — if missing, STOP and tell user to run `/spec $0` first
2. Present the acceptance criteria as a quick checklist to confirm scope

## Phase 1: CONTRACT — Types + Test Stubs

**Types:**
- Create/modify Rust structs with `serde` derives per the spec's API Contract
- If DB changes: add migration logic
- If WebSocket events: define event types

**Test Stubs:**
- One test per acceptance criterion, labeled `// AC-1: <description>`
- Tests compile but use `todo!()` for unimplemented assertions
- Unit tests inline, integration tests in `tests/integration/`

**GATE**: Show types + test list, confirm they match the spec.

## Phase 2: CODE — Backend + Frontend

**Backend:**
- Implement in order: storage → business logic → API handler
- Follow spec exactly — no scope creep
- Make tests pass as you go

**Frontend (if applicable):**
- Update TS types in `web-ui/src/lib/api.ts` to match Rust types
- Add API calls, update components, update stores
- Add i18n strings to all locale files

Run `cargo clippy && cargo test` and `npm run check` (if frontend).

**GATE**: Show test results + summary of changes.

## Phase 3: VERIFY + COMMIT

**Verify** (inline, not a separate skill call):
- Check each AC against the code — is it implemented?
- Check test plan — does every AC have a passing test?
- Check types match between Rust and TypeScript
- Update spec file: `[x]` for passing ACs, set `status: verified` (or `partial`)

**Commit:**
- Stage all changed files (including updated spec)
- Conventional commit: `feat: <description>` with `Spec: docs/specs/$0.md`

**GATE**: Show verification result + commit message, confirm.

## Rules

- **Spec is law** — implement what's specified, nothing more
- **If the spec is wrong**: STOP, update spec with user (`/spec $0` handles updates), then resume
- **Tests must pass** before Phase 3
- Skip frontend steps if backend-only (and vice versa)
- Don't gold-plate — the spec defines "done"

## Context

The spec:
!`cat /home/user/amigo-downloader/docs/specs/$0.md 2>/dev/null || echo "NO SPEC FOUND — run /spec $0 first!"`

Branch: !`git branch --show-current`
Recent commits: !`git log --oneline -5 2>/dev/null`
