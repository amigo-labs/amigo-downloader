---
name: spec-extract
description: Reverse-engineer a spec from existing code. Explores the codebase for a given area/feature and generates a spec that documents what's already implemented. Use to create specs for legacy code or to establish a baseline.
argument-hint: "<area or feature name>"
---

# Extract Spec from Code

Reverse-engineer a spec for **$ARGUMENTS** by reading the existing implementation.

## Process

### 1. Explore

Use Explore agents to find all code related to `$ARGUMENTS`:
- Rust: types, functions, API endpoints, DB queries, tests
- Frontend: components, API calls, stores, pages
- Config: locale keys, CLAUDE.md references

### 2. Reconstruct

From the code, derive:
- **What it does** — summarize the feature from the implementation
- **Acceptance Criteria** — reverse-engineer testable ACs from what the code actually does (not what it should do)
- **API Contract** — extract actual Rust types and endpoint definitions
- **Data Model** — extract actual DB schema from migrations/storage code
- **Edge Cases** — find error handling, match arms, guard clauses → these are the handled edge cases
- **Test Coverage** — find existing tests, note what's tested vs what's not

### 3. Present + Refine

Show the user what you found:
- "Here's what the code does for $ARGUMENTS"
- "These ACs are covered by tests: ..."
- "These have no tests: ..."
- "I found these gaps/inconsistencies: ..."

Ask: "Should I save this as-is, or do you want to adjust the spec to include planned improvements?"

### 4. Save

Write to `docs/specs/$0.md` using the standard spec template. Set status based on test coverage:
- `status: verified` — all ACs have passing tests
- `status: partial` — some ACs untested
- `status: draft` — mostly untested

Mark ACs as `[x]` if code exists and tests pass, `[ ]` if untested.

## Rules
- Document what IS, not what SHOULD BE — this is a snapshot of current state
- If the user wants to add planned improvements, add them as new `[ ]` ACs and set status to `partial`
- Reuse existing types verbatim — copy the actual struct definitions, don't paraphrase
- Note any code smells or inconsistencies in a `## Notes` section (but don't judge)

## Context

!`head -20 /home/user/amigo-downloader/CLAUDE.md`

Existing specs (avoid duplicates):
!`ls /home/user/amigo-downloader/docs/specs/*.md 2>/dev/null || echo "No specs yet"`
