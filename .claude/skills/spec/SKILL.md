---
name: spec
description: Collaboratively create or update a feature spec. Creates a new spec if none exists, or updates an existing one when requirements change. Use for new features and significant changes — not for bugfixes.
argument-hint: "<feature-name>"
---

# Spec: Create or Update

You are helping the user define **$ARGUMENTS**. This is collaborative — research, propose, refine.

## Step 1: Check if spec exists

Check `docs/specs/$0.md`:
- **Exists** → Read it, ask the user what should change, then update it (keep existing ACs, add/modify/remove as needed, reset status to `draft` for changed ACs)
- **New** → Continue with Step 2

## Step 2: Research + Ask

Do both in parallel:

**Research** (Explore agents):
- Find existing code related to this feature area
- Identify reusable patterns, types, utilities
- Check for conflicts with existing specs in `docs/specs/`

**Then present findings and ask questions** in small batches (1-4 at a time via AskUserQuestion):

- **What & Why** — What should happen? What's out of scope?
- **Data & API** — New endpoints? DB changes? Error handling?
- **UI** (if applicable) — Which page? User flow? Loading/error states?
- **Edge Cases** — Propose specific scenarios, let user confirm/deny

**Propose, don't just ask**: "I'd use the existing `RetryConfig` here — works for you?" is better than "What retry mechanism?"

Keep asking until no open questions remain.

## Step 3: Write + Review

Write the spec to `docs/specs/$0.md`, then present a concise summary (not the full markdown). Ask if anything is missing or wrong. Iterate until approved.

### Spec Template

```markdown
# Feature: <name>

> status: draft

## Summary
What, why, key design decision.

## Acceptance Criteria
- [ ] AC-1: <specific, testable statement>
- [ ] AC-2: ...

## API Contract

### Rust Types
\`\`\`rust
// Concrete serde structs with derives
\`\`\`

### REST Endpoints
| Method | Path | Request Body | Response | Status Codes |
|--------|------|-------------|----------|-------------|

### WebSocket Events (if applicable)
| Event | Payload | When |
|-------|---------|------|

## Data Model Changes
SQLite schema changes, new tables/columns.

## UI Changes (if applicable)
Affected pages/components, user flow, loading/error/empty states.

## Affected Files
- `crates/.../file.rs` — what changes
- `web-ui/src/.../File.svelte` — what changes

## Edge Cases
| Scenario | Expected Behavior |
|----------|------------------|

## Test Plan
- [ ] Unit: ...
- [ ] Integration: ...
- [ ] E2E (if UI): ...

## Out of Scope
What this does NOT include.
```

**Omit empty sections.** Backend-only? Drop UI Changes. No new endpoints? Drop API Contract.

## Rules
- Every AC must be testable — not "should be fast" but "responds within 200ms for files < 1GB"
- API types must be concrete Rust structs, not pseudocode
- At least 3 edge cases
- Test plan must cover every AC
- When updating an existing spec: preserve history, add a `## Changelog` section at the bottom

## Context

!`head -20 /home/user/amigo-downloader/CLAUDE.md`

Existing specs:
!`ls /home/user/amigo-downloader/docs/specs/*.md 2>/dev/null || echo "No specs yet"`

Existing API:
!`grep -n "\.get\|\.post\|\.patch\|\.delete\|\.route" /home/user/amigo-downloader/crates/server/src/api.rs 2>/dev/null | head -30`
