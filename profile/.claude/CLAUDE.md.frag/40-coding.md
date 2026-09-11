# Agent Directives: Readability & Minimal-Depth Code Chains

## 1. Project Intent
* **Goal**: Write highly maintainable, flat, explicit code. 
* **Core Philosophy**: Prioritize clarity over cleverness; eliminate cognitive load.

## 2. Code-Chain Constraints
* **Max Nesting**: Strictly enforce a maximum depth of 2 structural blocks (`if`, `loops`).
* **Guard Clauses**: Use early returns to keep the happy path completely flat.
* **No Nested Ternaries**: Do not use inline nested conditional statements.
* **No Callback Pyramids**: Linearize asynchronous logic using strict async/await syntax.
* **Rule of 30**: Break any function exceeding 30 lines into isolated, single-concern units.

## 3. Formatting & Readability Guidelines
* **Explicit Naming**: Choose descriptive, self-documenting variable and function names.
* **Function Syntax**: Prefer standard `function` declarations over arrow functions for top-level code.
* **Type Annotations**: Always enforce strict, explicit return types for all public interfaces.
* **Comments**: Remove noise; document *why* a constraint exists, never *what* the syntax does.

## 4. Workflow Boundaries
* **Chesterton’s Fence**: Analyze existing logic before modifying it; do not perform drive-by refactors.
* **Behavior Preservation**: Refactorings must preserve identical inputs, outputs, and side-effects.

