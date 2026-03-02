---
section: "06"
title: "Control Flow (§16)"
status: not-started
goal: "Expand §16 from 295 lines to ~1000+ lines with statement/expression distinction, terminating expressions, and detailed loop/match semantics"
inspired_by:
  - "Go spec 'Statements' — terminating statements, each form with full semantics"
  - "Rust reference 'Statements and expressions' — expression vs statement distinction"
depends_on: ["05"]
sections:
  - id: "06.1"
    title: "Statement vs Expression Distinction"
    status: not-started
  - id: "06.2"
    title: "Terminating Expressions"
    status: not-started
  - id: "06.3"
    title: "If-Then-Else Full Semantics"
    status: not-started
  - id: "06.4"
    title: "Match Expression Full Semantics"
    status: not-started
  - id: "06.5"
    title: "Loop Semantics"
    status: not-started
  - id: "06.6"
    title: "For Expression Full Semantics"
    status: not-started
  - id: "06.7"
    title: "Break, Continue, and Labels"
    status: not-started
  - id: "06.8"
    title: "Try Block Semantics"
    status: not-started
  - id: "06.9"
    title: "Completion Checklist"
    status: not-started
---

# Section 06: Control Flow (§16)

**Status:** Not Started
**Goal:** §16 is the most critically underspecified clause relative to its importance. At 295 lines, it covers `if`, loops, `break`/`continue`, and labels at a surface level. Go's equivalent (Statements) is ~1500 lines. For an expression-based language, this chapter should be one of the largest. Target ~1000+ lines.

**Context:** Ori is expression-based, so "control flow" is really "expression evaluation strategy." The distinction between §14 (Expressions) and §16 (Control Flow) is blurry — many constructs appear in both. This section should own the **evaluation strategy** for each form: when is a branch taken, what is the type of the overall expression, when does evaluation stop, what is the scope of bindings introduced.

The biggest structural gap: Ori has no concept of "terminating expressions" (Go's "terminating statements"). This is needed for: dead code detection, return-type checking ("does every path produce a value?"), and `Never` type propagation.

**Reference implementations:**
- **Go** `ref/spec#Statements`: Each statement form with complete semantics
- **Go** `ref/spec#Terminating_statements`: Precisely defines which forms guarantee termination
- **Rust** `reference/src/statements.md`: Statement evaluation and block-tail rules

---

## 06.1 Statement vs Expression Distinction

**File:** `docs/ori_lang/v2026/spec/16-control-flow.md`

Ori is expression-based, but some expressions are used in "statement position." Formalize this.

- [ ] Define "expression statement": an expression evaluated for its side effects, with the result discarded
  - `print(msg: "hello");` — the `;` makes it a statement
  - The discarded value shall have type `void` or the expression shall not be the last in a block

- [ ] Define "result expression": the last expression in a block (without `;`) whose value becomes the block's value

- [ ] State: these constructs are statements only (never produce values):
  - `let` bindings (type: `void`)
  - `use` imports (type: `void`)
  - Assignments `x = v` (type: `void`)
  - Compound assignments `x += v` (type: `void`)

- [ ] State: all other constructs are expressions that produce values
  - `if...then...else` produces a value
  - `match` produces a value
  - `for...yield` produces a value
  - `loop { break value }` produces a value
  - `for...do` produces `void`
  - `loop { break }` produces `void`

---

## 06.2 Terminating Expressions

**File:** `docs/ori_lang/v2026/spec/16-control-flow.md`

Define which expression forms are guaranteed to not produce a value (type `Never` / diverge).

- [ ] Define "terminating expression" — an expression whose evaluation is guaranteed to not complete normally:

  1. `panic(msg: ...)` — always terminates
  2. `todo()` — always terminates
  3. `unreachable()` — always terminates
  4. `break` / `break value` — exits enclosing loop
  5. `continue` — skips to next iteration
  6. A block `{ ... e }` where `e` is terminating
  7. `if c then t else e` where both `t` and `e` are terminating
  8. `match expr { arms }` where every arm is terminating
  9. `loop { body }` without any `break` (infinite loop)
  10. An expression followed by `?` where the Err/None branch diverges

- [ ] State: if the last expression in a function body is terminating, the function may have return type `Never`

- [ ] State: unreachable code after a terminating expression should produce a warning

- [ ] Cross-reference to `Never` type semantics in §8.1.1

---

## 06.3 If-Then-Else Full Semantics

**File:** `docs/ori_lang/v2026/spec/16-control-flow.md`

Currently §14.9 covers `if` but §16 should own the control flow semantics.

- [ ] Consolidate the evaluation rules (or clearly cross-reference §14.9):
  - Condition evaluated first; shall have type `bool`
  - Only one branch evaluated
  - With `else`: both branches shall have compatible types
  - Without `else`: then-branch shall have type `void` or `Never`
  - `else if` chains: semantically nested `if` in else branch

- [ ] Add: nested `if` depth — any limit? (Implementation-defined)
- [ ] Add: interaction with pattern matching in conditions (if-let equivalent?)
  - Decision needed: `if let Some(x) = opt then ...` — is this supported?
  - Check compiler

---

## 06.4 Match Expression Full Semantics

**File:** `docs/ori_lang/v2026/spec/16-control-flow.md`

Match is covered in §15 (Patterns) but needs control flow semantics here.

- [ ] Evaluation order:
  1. Scrutinee expression evaluated once
  2. Arms tested top-to-bottom
  3. First matching arm's body evaluated
  4. Guard (`if`) evaluated after pattern match succeeds

- [ ] Type rules:
  - All arm bodies shall have compatible types
  - `Never` arms are compatible with any type
  - The match expression type is the unified type of all arms

- [ ] Exhaustiveness:
  - The compiler shall verify that match is exhaustive
  - If not exhaustive: compile-time error
  - Guards break exhaustiveness (guarded arms don't contribute)
  - A catch-all `_` or binding pattern after guarded arms restores exhaustiveness

- [ ] Redundancy:
  - Unreachable arms after a catch-all: warning
  - Patterns that are subsets of earlier patterns: warning

---

## 06.5 Loop Semantics

**File:** `docs/ori_lang/v2026/spec/16-control-flow.md`

Expand §14.11 (currently in Expressions).

- [ ] Infinite loop detection:
  - `loop { body }` with no `break` — type is `Never`
  - `loop { body }` with `break value` — type is value type
  - `loop { body }` with `break` (no value) — type is `void`

- [ ] Loop body evaluation:
  - Body is a block expression
  - After body completes, re-enters body (unless `break`)
  - `continue` skips rest of body, re-enters

- [ ] Multiple break values: all break values shall have the same type

- [ ] Loop as the last expression in a function:
  ```ori
  @server () -> Never = loop { handle() };  // OK: Never
  @compute () -> int = loop { if done then break result };  // OK: int
  ```

---

## 06.6 For Expression Full Semantics

**File:** `docs/ori_lang/v2026/spec/16-control-flow.md`

Expand §14.10 with full iteration semantics.

- [ ] Iterator protocol:
  - `for x in source do body` desugars to:
    1. Call `source.iter()` to get an iterator
    2. Call `iterator.next()` repeatedly
    3. On `Some(value)`: bind `x = value`, evaluate body
    4. On `None`: stop
  - State the desugaring explicitly

- [ ] Guard semantics:
  - `for x in source if guard do body` — guard evaluated after binding
  - If guard is `false`, skip to next iteration (implicit `continue`)

- [ ] For-yield semantics:
  - `for x in source yield expr` — collect results into a list
  - Type: `[T]` where `T` = type of yield expression
  - Empty source → empty list
  - `break` — stop early, return accumulated values
  - `break value` — append value and stop
  - `continue` — skip this element (don't add to result)
  - `continue value` — substitute value for this element

- [ ] Nested for-yield:
  ```ori
  for x in xs
  for y in ys
  yield (x, y)
  ```
  - Equivalent to `xs.flat_map(x -> ys.map(y -> (x, y)))`

---

## 06.7 Break, Continue, and Labels

**File:** `docs/ori_lang/v2026/spec/16-control-flow.md`

Expand §16.3 with precise rules.

- [ ] `break` forms:
  | Form | Context | Effect |
  |------|---------|--------|
  | `break` | `loop { }` | Exit loop, loop type is `void` |
  | `break value` | `loop { }` | Exit loop, loop type is value type |
  | `break` | `for...do` | Exit loop |
  | `break` | `for...yield` | Stop, return accumulated |
  | `break value` | `for...yield` | Append value, stop, return |
  | `break:label` | Labeled loop | Exit labeled loop |
  | `break:label value` | Labeled loop | Exit with value |

- [ ] `continue` forms:
  | Form | Context | Effect |
  |------|---------|--------|
  | `continue` | `loop { }` | Skip to next iteration |
  | `continue` | `for...do` | Skip to next element |
  | `continue` | `for...yield` | Skip element (don't yield) |
  | `continue value` | `for...yield` | Substitute value |
  | `continue:label` | Labeled loop | Continue labeled loop |
  | `continue:label value` | Labeled yield | Substitute in labeled |

- [ ] Error cases:
  - `break` outside loop/for: compile error
  - `continue` outside loop/for: compile error
  - `break value` in `for...do`: compile error (E0860)
  - `continue value` in `loop`: compile error (E0861)
  - `break:label` with undefined label: compile error
  - Label shadowing: compile error

---

## 06.8 Try Block Semantics

**File:** `docs/ori_lang/v2026/spec/16-control-flow.md`

- [ ] `try { ... }` — error-propagating block:
  - `?` inside try propagates to the try boundary (not the enclosing function)
  - Type: `Result<T, E>` where `T` = block value type, `E` = error type from `?`
  - Without `?` inside: try block is semantically identical to a regular block

- [ ] Interaction with other control flow:
  - `break` inside try: exits the enclosing loop (passes through try)
  - `continue` inside try: skips in the enclosing loop
  - `return` (N/A — no return in Ori)

---

## 06.9 Completion Checklist

- [ ] Statement vs expression distinction formalized
- [ ] Terminating expressions defined (complete list)
- [ ] If-then-else evaluation rules complete
- [ ] Match evaluation rules complete (exhaustiveness, redundancy)
- [ ] Loop semantics complete (type inference from break values)
- [ ] For expression desugaring to iterator protocol explicit
- [ ] For-yield semantics with break/continue/value fully specified
- [ ] Break/continue form tables complete with all valid/invalid combinations
- [ ] Try block semantics defined
- [ ] Label scoping rules referenced from §11
- [ ] All error codes referenced
- [ ] All additions use ISO normative style

**Exit Criteria:** A type checker implementor can determine from §16 alone: whether code is reachable, what type a control flow expression produces, and what break/continue forms are valid in any context.
