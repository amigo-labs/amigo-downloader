---
name: spec-fix
description: Lightweight bugfix workflow. No full spec needed — just find the root cause, fix it, add a regression test, and commit. Use for bugs, not for new features.
argument-hint: "<bug description or issue number>"
---

# Bugfix: $ARGUMENTS

## Step 1: FIND — Root Cause Analysis

1. **Reproduce**: Understand the bug from the description. Search the codebase for related code.
2. **Trace**: Follow the code path to find the root cause. Use Explore agents if the scope is unclear.
3. **Present findings** to user:
   - "The bug is in `file.rs:123` — here's what happens: ..."
   - "Root cause: ..."
   - "Fix approach: ..."

4. **Check specs**: Search `docs/specs/` for specs that cover the affected code. If the fix changes behavior defined in a spec, flag it: "This fix affects spec X — AC-N needs to be updated."

Ask for confirmation before fixing.

## Step 2: FIX — Code + Regression Test + Spec Update

1. **Write the regression test first** — it should fail before the fix and pass after
2. **Fix the bug** — minimal change, don't refactor surrounding code
3. Run `cargo clippy && cargo test` (and `npm run check` if frontend)

4. **Update affected specs** — if the fix changes behavior covered by a spec, update the relevant ACs to match the corrected behavior

Show the user: diff summary + test results + spec changes (if any).

## Step 3: COMMIT

- Conventional commit: `fix: <what was broken>`
- Reference issue if provided: `Fixes #<number>`

Ask for confirmation before committing.

## Rules
- **Minimal fix** — don't refactor, don't improve, don't clean up
- **Always add a regression test** — a fix without a test is incomplete
- **No new spec needed** — but if the fix changes spec'd behavior, update the affected spec
- If the bug reveals a spec gap (untested edge case), add it as a new AC to the relevant spec
- If the fix is bigger than ~50 lines or touches 5+ files, it's probably not a bugfix — suggest `/spec` instead

## Context

Branch: !`git branch --show-current`
Recent commits: !`git log --oneline -5 2>/dev/null`
