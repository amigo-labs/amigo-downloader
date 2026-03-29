---
name: spec-feature
description: Orchestrate the complete spec-driven development workflow for a new feature. Walks through spec, types, tests, implementation, verification, and commit — step by step with user approval at each gate.
argument-hint: "<feature-name>"
---

# Spec-Driven Feature Workflow

Implement **$ARGUMENTS** using the complete spec-driven development process. Each step requires user approval before proceeding.

## Workflow Steps

### Step 1: SPEC
Write the feature specification first.

- Invoke the `/spec` skill logic (follow the same process as the spec skill for `$ARGUMENTS`)
- Create `docs/specs/$0.md` with full spec including acceptance criteria, API contract, test plan
- **GATE**: Present spec summary to user, ask for approval before proceeding

### Step 2: TYPES
Define the data contract.

- Create/modify Rust structs with `serde` derives in the appropriate crate
- If API changes: define request/response types in server crate
- If DB changes: add migration logic in storage.rs
- If WebSocket events: define event types in ws.rs
- **GATE**: Show user the type definitions, confirm they match the spec

### Step 3: TESTS (TDD-light)
Write test stubs before implementation.

- Create test functions for each acceptance criterion from the spec
- Unit tests: inline `#[cfg(test)]` modules in the relevant source files
- Integration tests: in `tests/integration/` if API endpoints are involved
- Tests should compile but can use `todo!()` for assertions that need implementation
- **GATE**: Show user the test list, confirm coverage is adequate

### Step 4: BACKEND
Implement the backend logic.

- Follow the spec exactly — no scope creep
- Implement in this order: types -> storage -> business logic -> API handler
- Make tests pass as you implement
- Run `cargo clippy` and `cargo test` after implementation
- **GATE**: Show user which tests pass/fail, ask for approval

### Step 5: FRONTEND (if applicable)
Implement UI changes.

- Update TypeScript types in `web-ui/src/lib/api.ts` to match Rust types
- Add API calls for new endpoints
- Update/create Svelte components
- Update stores if WebSocket events were added
- Add i18n strings to both `locales/en.json` and `locales/de.json`
- Run `npm run check` in web-ui/
- **GATE**: Describe UI changes to user, confirm they match spec

### Step 6: VERIFY
Run the consistency check.

- Invoke the `/spec-verify` skill logic (follow the same process as the spec-verify skill)
- Address any FAIL items before proceeding
- WARN items: present to user for decision
- **GATE**: Show verification report, ask if issues should be fixed

### Step 7: COMMIT
Create a clean commit.

- Stage all changed files
- Write a conventional commit message: `feat: <description>`
- Reference the spec: `Spec: docs/specs/$0.md`
- Mark acceptance criteria as checked in the spec file
- **GATE**: Show commit message and changed files list, ask for confirmation

## Rules

- **NEVER skip a step** — each step builds on the previous
- **NEVER proceed without user approval** at each gate
- **Stick to the spec** — if implementation reveals the spec is wrong, go back and update it first
- **One feature at a time** — do not bundle unrelated changes
- **Tests must pass** before moving to the next step
- If the feature is backend-only, skip Step 5
- If the feature is frontend-only, adjust Step 4 to be minimal (just type definitions)

## Context

Current branch:
!`git branch --show-current`

Recent commits (for commit message style):
!`git log --oneline -5 2>/dev/null || echo "No commits yet"`

Existing specs:
!`ls /home/user/amigo-downloader/docs/specs/ 2>/dev/null || echo "No specs yet"`
