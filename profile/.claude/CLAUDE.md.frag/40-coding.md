# Readable, Explicit Code

Write maintainable code that remains understandable after long periods without context. Prioritize clarity over cleverness and reduce cognitive load.

## Naming and Types

- Use names that communicate domain meaning and intent. Avoid vague names such as `data`, `item`, `value`, `result`, or `handler` when a specific name exists.
- Prefer concise names only when unambiguous in the immediate scope.
- Use the strictest practical type checking supported by the language, framework, and repository. Type public interfaces and important domain boundaries explicitly where supported.
- Make invalid states unrepresentable when it improves clarity. Prefer discriminated unions, enums, validated value objects, exhaustive matching, and constrained constructors over boolean combinations and defensive branches.
- Do not weaken types with `any`, unchecked casts, broad unions, or unnecessary nullable fields merely to compile.

## Control Flow

- Limit decision nesting (`if`, loops, and equivalent branches) to two levels. Use guard clauses, cohesive extraction, domain modeling, and exhaustive matching to keep the happy path flat.
- Do not use nested ternaries.
- Prefer `async`/`await` when supported and locally idiomatic. Do not wrap callback- or stream-based APIs solely to satisfy this preference.
- Limit function bodies to 30 logical lines, excluding blank lines, comments, signatures, and declarative data.
- Extract cohesive responsibilities, not thin pass-through helpers created solely to meet structural limits.
- If an algorithm cannot meet a structural limit without reducing readability, explain the exception before proceeding.

## Syntax and Comments

- Prefer `function` declarations for reusable top-level JavaScript or TypeScript functions unless a stronger local convention exists.
- Comments explain intent, constraints, or non-obvious decisions—not syntax.

## Workflow Boundaries

- Apply Chesterton's Fence: understand existing logic before modifying it.
- Refactors preserve identical inputs, outputs, side effects, and externally observable behavior unless a change is explicitly required.
