---
name: spec-feature
description: Implement a feature based on an existing spec from docs/specs/. Use this AFTER /spec has been used to create the spec. Walks through types, tests, backend, frontend, verification, and commit — step by step with user approval at each gate.
argument-hint: "<feature-name>"
---

# Spec-Driven Feature Implementation

Implement **$ARGUMENTS** based on the existing spec. This skill is Step 2 in the workflow — the spec must already exist.

## Prerequisites

1. **Read the spec** from `docs/specs/$0.md`
2. If no spec exists, STOP and tell the user to run `/spec $ARGUMENTS` first
3. Present a quick summary of the spec's acceptance criteria to confirm we're on the same page

## Implementation Steps

### Step 1: TYPES
Define the data contract from the spec.

- Create/modify Rust structs with `serde` derives in the appropriate crate
- If API changes: define request/response types in server crate
- If DB changes: add migration logic in storage.rs
- If WebSocket events: define event types in ws.rs
- Cross-check every type against the spec's "API Contract" section
- **GATE**: Show user the type definitions, confirm they match the spec

### Step 2: TESTS (TDD-light)
Write test stubs before implementation.

- Create one test function per acceptance criterion from the spec
- Map each test to its AC: `// AC-1: <description>`
- Unit tests: inline `#[cfg(test)]` modules in the relevant source files
- Integration tests: in `tests/integration/` if API endpoints are involved
- Tests should compile but can use `todo!()` for assertions that need implementation
- Cross-check against spec's "Test Plan" section — every item must have a test
- **GATE**: Show user the test list, confirm coverage matches the spec

### Step 3: BACKEND
Implement the backend logic.

- Follow the spec exactly — no scope creep, no "improvements" beyond spec
- Implement in this order: storage → business logic → API handler
- Make tests pass as you implement
- Run `cargo clippy` and `cargo test` after implementation
- **GATE**: Show user which tests pass/fail, ask for approval

### Step 4: FRONTEND (if applicable)
Implement UI changes per the spec's "UI Changes" section.

- Update TypeScript types in `web-ui/src/lib/api.ts` to match Rust types
- Add API calls for new endpoints
- Update/create Svelte components
- Update stores if WebSocket events were added
- Add i18n strings to both `locales/en.json` and `locales/de.json`
- Run `npm run check` in web-ui/
- **GATE**: Describe UI changes to user, confirm they match spec

### Step 5: VERIFY
Run the full consistency check.

- Invoke the `/spec-verify` skill logic
- Address any FAIL items before proceeding
- WARN items: present to user for decision
- **GATE**: Show verification report, ask if issues should be fixed

### Step 6: COMMIT
Create a clean commit.

- Stage all changed files
- Write a conventional commit message: `feat: <description>`
- Reference the spec: `Spec: docs/specs/$0.md`
- Mark acceptance criteria as checked (`[x]`) in the spec file
- **GATE**: Show commit message and changed files list, ask for confirmation

## Rules

- **Spec is the source of truth** — implement exactly what's specified, nothing more
- **NEVER skip a step** — each step builds on the previous
- **NEVER proceed without user approval** at each gate
- **If the spec is wrong**, STOP implementation. Update the spec first, get approval, then continue
- **Tests must pass** before moving to the next step
- If the feature is backend-only, skip Step 4
- If the feature is frontend-only, adjust Step 3 accordingly
- Check off each acceptance criterion in the spec as it's implemented

## Context

The spec to implement:
!`cat /home/user/amigo-downloader/docs/specs/$0.md 2>/dev/null || echo "NO SPEC FOUND — run /spec $0 first!"`

Current branch:
!`git branch --show-current`

Recent commits (for commit message style):
!`git log --oneline -5 2>/dev/null || echo "No commits yet"`
