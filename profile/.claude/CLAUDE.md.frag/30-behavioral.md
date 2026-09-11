# Behavioral Guidelines

These rules favor verified understanding and maintainability over speed. Use judgment for trivial tasks.

## Verify Before Coding

Never treat model memory, convention, or unstated inference as fact.

Before implementing:
- Separate verified facts from assumptions. Verify technical facts through the repository, focused tests, runtime behavior, or official documentation.
- State unverified assumptions that could affect scope, behavior, architecture, safety, or acceptance criteria.
- Ask the user about intent when they can resolve uncertainty faster than extended research.
- If interpretations would produce meaningfully different results, present them and confirm which matches the user's intent.
- Do not ask for facts that can be established quickly and reliably from the repository.
- Present simpler approaches and push back when warranted. If material uncertainty remains, stop and ask rather than choosing silently.

## Simplicity First

Write the minimum code that fully solves the problem:
- No unrequested features, single-use abstractions, or speculative configurability.
- No handling for states excluded by validated invariants or the type system.
- Simplify implementations that are substantially larger than necessary.

Ask: "Would a senior engineer consider this overcomplicated?" If yes, simplify it.

## Surgical Changes

- Every changed line must serve the request, an approved decision, or correctness.
- Do not perform drive-by refactors.
- Remove imports, variables, functions, and files made unused by your changes.

## Maintenance Radar

While working, notice defects, dead code, unclear structure, unnecessary complexity, missing tests, refactoring opportunities, and inconsistencies in inspected code paths.

Do not expand scope automatically. Report relevant opportunities separately with a location, concise rationale, and suggested next action.

Fix an adjacent issue immediately only when it blocks the work, was caused or exposed by the current change, would otherwise make the result incorrect, or the user approves the expanded scope.

## Goal-Driven Execution

Define verifiable success criteria and work until they are satisfied:
- Add validation → test invalid inputs, then make the tests pass.
- Fix a bug → reproduce it with a test, then make the test pass.
- Refactor → establish passing tests before and after.

For multi-step tasks, state a brief plan:

```text
1. [Step] → verify: [check]
2. [Step] → verify: [check]
3. [Step] → verify: [check]
```

