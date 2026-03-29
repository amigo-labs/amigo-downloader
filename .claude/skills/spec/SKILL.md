---
name: spec
description: Interactively develop a structured feature spec together with the user. Explores the codebase, asks targeted questions, and iteratively builds a complete spec with all necessary changes identified. Use this whenever a new feature is planned.
argument-hint: "<feature-name>"
---

# Spec-Driven Development: Collaborative Spec Discovery

You are helping the user develop a complete feature specification for **$ARGUMENTS**. This is a COLLABORATIVE process — you research, ask, propose, and refine together until every detail is nailed down.

## Process: 3 Phases

### Phase 1: DISCOVER — Understand what's needed

1. **Research the codebase** using Explore agents in parallel:
   - Find all code related to the feature area (existing implementations, types, API endpoints, UI components)
   - Identify existing patterns, utilities, and abstractions that can be reused
   - Check for potential conflicts or dependencies

2. **Present your findings** to the user:
   - "Here's what already exists related to this feature: ..."
   - "These files/components will likely be affected: ..."
   - "I see these potential approaches: ..."

3. **Ask targeted questions** using AskUserQuestion to fill gaps. Cover these dimensions one by one — do NOT dump all questions at once, instead ask in logical groups of 1-4 questions:

   **Scope & Behavior:**
   - What exactly should happen? Walk me through the user flow step by step
   - What should NOT be included? (explicit boundaries)

   **Data & API:**
   - What data is needed? Where does it come from?
   - Do we need new API endpoints? New DB fields?
   - How should errors be communicated to the user?

   **UI (if applicable):**
   - Which page/view does this live on?
   - What does the user see/click/interact with?
   - What happens during loading/error/empty states?

   **Edge Cases** (propose specific scenarios for the user to confirm/deny):
   - "What should happen if [specific scenario]?"
   - "Should we handle [edge case] now or is that out of scope?"

   **Integration:**
   - Does this affect WebSocket events?
   - Does this need i18n strings?
   - Does this interact with the plugin system?

4. **Iterate**: After each answer, dig deeper. If the user says "it should support X", ask HOW specifically. Keep asking until there are no open questions.

### Phase 2: PROPOSE — Draft the spec

Once all questions are answered, write the spec to `docs/specs/$0.md`:

```markdown
# Feature: <name>

> status: draft

## Summary
One paragraph: what, why, and key design decision.

## Acceptance Criteria
Testable, numbered. Each must be verifiable by a test.
- [ ] AC-1: <specific, testable statement>
- [ ] AC-2: ...

## API Contract

### Rust Types
\`\`\`rust
// Concrete serde structs — not pseudocode
\`\`\`

### REST Endpoints
| Method | Path | Request Body | Response | Status Codes |
|--------|------|-------------|----------|-------------|

### WebSocket Events
| Event | Payload | When |
|-------|---------|------|

## Data Model Changes
- DB migrations (SQLite schema changes)
- New tables/columns with types

## UI Changes
- Affected pages/components
- New components needed
- User interaction flow (step by step)
- Loading/error/empty states

## Affected Files
Grouped by layer:
### Backend
- `crates/.../file.rs` — what changes
### Frontend
- `web-ui/src/.../File.svelte` — what changes
### Other
- `locales/*.json` — new keys
- `CLAUDE.md` — doc updates if needed

## Edge Cases & Error Handling
| Scenario | Expected Behavior |
|----------|------------------|
| ... | ... |

## Test Plan
### Unit Tests
- [ ] ...
### Integration Tests
- [ ] ...
### E2E Tests (if UI)
- [ ] ...

## Dependencies
- External crates/packages needed
- Other features this depends on

## Out of Scope
- What this feature explicitly does NOT include
```

### Phase 3: REVIEW — Refine together

1. **Present a summary** of the spec (not the full markdown — a concise overview):
   - Key acceptance criteria
   - Files that will change
   - Anything that surprised you or seems risky

2. **Ask for approval** using AskUserQuestion:
   - "Is there anything missing or wrong?"
   - "Should I adjust scope on any of these points?"

3. **Iterate** until the user confirms the spec is complete.

4. **Save** the final spec to `docs/specs/$0.md`.

## Rules

- **NEVER write the spec before understanding the feature** — Phase 1 comes first
- **Ask questions in small batches** (1-4 at a time), not a wall of 15 questions
- **Be specific in your questions** — "Should cancelled downloads be deletable from history?" is better than "How should history work?"
- **Propose, don't just ask** — "I'd suggest we use the existing `RetryConfig` struct here. Does that work?" is better than "What retry mechanism should we use?"
- **Every acceptance criterion MUST be testable** — no vague statements
- **API types must be concrete Rust structs** with serde derives, not pseudocode
- **Edge cases section must have at least 3 scenarios** with expected behavior
- **Test plan must cover every acceptance criterion**
- **Check existing `docs/specs/`** for related specs to avoid contradictions
- If a feature touches both frontend and backend, the API contract section is MANDATORY

## Context

Current project architecture and conventions:
!`head -20 /home/user/amigo-downloader/CLAUDE.md`

Existing specs:
!`ls /home/user/amigo-downloader/docs/specs/ 2>/dev/null || echo "No specs yet"`

Existing API endpoints in server:
!`grep -n "fn\|Router\|route\|.get\|.post\|.patch\|.delete" /home/user/amigo-downloader/crates/server/src/api.rs 2>/dev/null | head -40`

Existing Rust types (core):
!`grep -n "pub struct\|pub enum" /home/user/amigo-downloader/crates/core/src/lib.rs 2>/dev/null | head -30`

Existing frontend API functions:
!`grep -n "export.*function\|export.*async\|export.*const" /home/user/amigo-downloader/web-ui/src/lib/api.ts 2>/dev/null | head -30`
