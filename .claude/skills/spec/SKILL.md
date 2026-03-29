---
name: spec
description: Create a structured feature spec before implementation. Use this whenever a new feature is planned to ensure testable requirements, API contracts, and edge cases are defined upfront.
argument-hint: "<feature-name>"
---

# Spec-Driven Development: Write Feature Spec

You are writing a feature specification for **$ARGUMENTS**. This spec will serve as the single source of truth for implementation.

## Process

1. **Research first**: Explore the codebase to understand existing patterns, related code, and potential conflicts. Read CLAUDE.md for architecture context.

2. **Ask clarifying questions**: Use AskUserQuestion to resolve any ambiguity about the feature before writing the spec.

3. **Write the spec** to `docs/specs/$0.md` using this template:

```markdown
# Feature: <name>

## Summary
One paragraph describing what this feature does and why.

## Acceptance Criteria
Testable, numbered requirements. Each criterion must be verifiable by a test.
- [ ] AC-1: <specific, testable statement>
- [ ] AC-2: ...

## API Contract

### Rust Types
```rust
// serde structs for request/response
```

### REST Endpoints
| Method | Path | Request Body | Response | Status Codes |
|--------|------|-------------|----------|-------------|

### WebSocket Events
| Event | Payload | When |
|-------|---------|------|

## Data Model Changes
- DB migrations needed (SQLite schema changes)
- New tables/columns

## UI Changes
- Which pages/components are affected
- New components needed
- User interaction flow

## Affected Files
- List of files that need modification
- New files to create

## Edge Cases & Error Handling
- What happens when X fails?
- Invalid input scenarios
- Concurrent access scenarios
- Network failure scenarios

## Test Plan
### Unit Tests
- [ ] Test case 1: ...
### Integration Tests
- [ ] Test case 1: ...
### E2E Tests (if UI involved)
- [ ] Test case 1: ...

## Dependencies
- External crates/packages needed
- Other features this depends on

## Out of Scope
- What this feature explicitly does NOT include
```

4. **Review with user**: Present the spec summary and ask for approval before marking it ready.

## Rules
- Every acceptance criterion MUST be testable — no vague statements like "should be fast" or "nice UI"
- API types must be concrete Rust structs with serde derives, not pseudocode
- Edge cases section must have at least 3 scenarios
- Test plan must cover every acceptance criterion
- If the feature touches both frontend and backend, the API contract section is MANDATORY
- Check existing `docs/specs/` for related specs to avoid contradictions

## Context

Current project architecture and conventions:
!`head -20 /home/user/amigo-downloader/CLAUDE.md`

Existing specs:
!`ls /home/user/amigo-downloader/docs/specs/ 2>/dev/null || echo "No specs yet"`

Existing API endpoints in server:
!`grep -n "fn\|Router\|route\|.get\|.post\|.patch\|.delete" /home/user/amigo-downloader/crates/server/src/api.rs 2>/dev/null | head -40`
