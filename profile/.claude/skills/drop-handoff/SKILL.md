---
name: drop-handoff
description: Write HANDOFF.md into the current workspace so a fresh Claude Code session can continue without reading this chat.
argument-hint: "Optional focus area for the next session"
---

Write a HANDOFF.md file in the current working directory. The file must be complete enough that a fresh Claude Code session can continue without reading this chat.

Include exactly these sections:

## Goal
What the session was trying to accomplish and the acceptance criteria.

## Current Branch / State
Current git branch, any uncommitted changes, and overall repo state.

## Files Changed
Bullet list: path — what changed and why.

## Decisions Made
Key choices made during this session and the reasoning behind each.

## Commands Run
Important shell commands executed (installs, migrations, test runs, etc.).

## What Failed
Errors, failed tests, or approaches that were tried and abandoned, with root cause if known.

## What Remains
Remaining tasks, in priority order.

## Exact Next Step
One concrete, actionable instruction the next agent should execute first.

---

Rules:
- Do not duplicate content already in PLAN.md, ADRs, or open GitHub issues — reference them by path or URL instead.
- Redact secrets, API keys, and passwords.
- Prefer absolute file paths over relative ones.
- If args were passed, treat them as the focus area for the next session and tailor the "Exact Next Step" section accordingly.
