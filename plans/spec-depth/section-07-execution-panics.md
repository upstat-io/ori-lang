---
section: "07"
title: "Program Execution and Runtime Panics (§23 + new)"
status: not-started
goal: "Expand §23 to ~300 lines and add a Runtime Panics subsection cataloguing all panic conditions"
inspired_by:
  - "Go spec 'Program initialization and execution' — zero value, package init, program init"
  - "Go spec 'Run-time panics' — consolidated list of panic conditions"
  - "Go spec 'Errors' — error interface as language concept"
depends_on: ["06"]
sections:
  - id: "07.1"
    title: "Module Initialization Order"
    status: not-started
  - id: "07.2"
    title: "Process Environment"
    status: not-started
  - id: "07.3"
    title: "Termination and Cleanup"
    status: not-started
  - id: "07.4"
    title: "Runtime Panic Catalogue"
    status: not-started
  - id: "07.5"
    title: "Panic Handler Semantics"
    status: not-started
  - id: "07.6"
    title: "Completion Checklist"
    status: not-started
---

# Section 07: Program Execution and Runtime Panics (§23 + new)

**Status:** Not Started
**Goal:** §23 is the thinnest core clause at 95 lines. It covers entry point signatures and basic init/termination but misses module init ordering, environment, cleanup, and runtime panic semantics. The panic catalogue is scattered across §8, §14, §17 — it needs consolidation. Target ~300+ lines in §23 plus a new panic subsection.

**Context:** Go's "Program initialization and execution" section defines the zero value, package initialization order, and program execution model. Separately, "Run-time panics" catalogues every condition that causes a panic. Ori's runtime panic info is scattered: integer overflow in §14.3.2, index bounds in §14.1.2, shift overflow in §14.3.2, division by zero in §14.3.2. A single catalogue prevents inconsistency and helps implementors.

**Reference implementations:**
- **Go** `ref/spec#Program_initialization_and_execution`: init order, zero value, goroutine model
- **Go** `ref/spec#Run-time_panics`: consolidated panic list
- **Rust** `reference/src/behavior-considered-undefined.md`: catalogue of UB/panic conditions

---

## 07.1 Module Initialization Order

**File:** `docs/ori_lang/v2026/spec/23-program-execution.md` — expand §23.2

- [ ] Formalize initialization algorithm:
  - Modules form a DAG via imports
  - A module is initialized after all modules it imports
  - Within a module: config variables are evaluated in dependency order
  - Circular module dependencies are a compile-time error
  - Initialization order is deterministic (topological sort, ties broken by import order)

- [ ] State: side effects during module initialization:
  - Config variable initializers may have side effects? Or must be pure?
  - If impure: order is guaranteed and observable
  - If pure: order is guaranteed but not observable (optimizer may reorder)

- [ ] State: all modules are initialized before `@main` runs (no lazy init)

---

## 07.2 Process Environment

**File:** `docs/ori_lang/v2026/spec/23-program-execution.md`

- [ ] `args` parameter:
  - Contains command-line arguments
  - Does NOT include the program name (first element is first user argument)
  - Empty list `[]` if no arguments
  - Each element is a `str`

- [ ] Exit code:
  - Range: 0–255 (values outside this range are truncated to low 8 bits)
  - Convention: 0 = success, non-zero = failure
  - `void` return → exit code 0
  - Panic termination → exit code 1

- [ ] Standard streams:
  - `print(msg:)` writes to stdout
  - Panic messages write to stderr
  - Stack traces write to stderr
  - No `stdin` built-in (use `std.io` capability)

---

## 07.3 Termination and Cleanup

**File:** `docs/ori_lang/v2026/spec/23-program-execution.md` — expand §23.3

- [ ] Normal termination:
  - `@main` returns → Drop impls run for live values → process exits
  - Drop order: reverse allocation order (LIFO)
  - All Drop impls shall complete before exit

- [ ] Panic termination:
  - Unhandled panic → error + trace to stderr → exit code 1
  - Drop impls for values in scope at panic point: are they run?
    - Decision needed: Ori runs Drop during unwind or not?
    - Check compiler behavior
    - If yes: "panic unwind" model (like Rust default)
    - If no: "panic abort" model (like Rust panic=abort)

- [ ] Concurrent termination:
  - If tasks are running when main exits: are they cancelled?
  - Nursery semantics: nursery ensures all child tasks complete
  - Orphan tasks (spawned without nursery): implementation-defined

---

## 07.4 Runtime Panic Catalogue

**File:** `docs/ori_lang/v2026/spec/23-program-execution.md` or new subsection

Consolidate every panic condition from across the spec into one table.

- [ ] Create consolidated table:

  | Condition | Source | Message Pattern |
  |-----------|--------|----------------|
  | Integer overflow (add/sub/mul/neg) | §14.3.2 | "integer overflow" |
  | Integer division by zero | §14.3.2 | "division by zero" |
  | Integer modulo by zero | §14.3.2 | "modulo by zero" |
  | `int.min / -1` | §14.3.2 | "integer overflow" |
  | `int.min % -1` | §14.3.2 | "integer overflow" |
  | Shift count negative | §14.3.2 | "negative shift count" |
  | Shift count ≥ bit width | §14.3.2 | "shift count exceeds bit width" |
  | Shift result overflow | §14.3.2 | "shift overflow" |
  | List index out of bounds | §14.1.2 | "index out of bounds" |
  | String index out of bounds | §14.1.2 | "index out of bounds" |
  | Unwrap on None | §9 Option | "unwrap called on None" |
  | Unwrap on Err | §9 Result | "unwrap called on Err" |
  | `panic(msg:)` | Built-in | User message |
  | `todo()` | Built-in | "not yet implemented" |
  | `unreachable()` | Built-in | "entered unreachable code" |
  | `assert` failure | Built-in | "assertion failed" |
  | `assert_eq` failure | Built-in | "assertion failed: actual ≠ expected" |
  | Range step is zero | §14.6.1 | "step cannot be zero" |
  | `as` conversion failure | §14.1.5 | "conversion out of range" |
  | Fixed-capacity list overflow | §8 Types | "list is full" |
  | Stack overflow | §23 | "stack overflow" |
  | Pre-condition failure | §15 Contracts | "pre-condition violated" |
  | Post-condition failure | §15 Contracts | "post-condition violated" |

- [ ] State: all panics produce a `PanicInfo` value with message, location, and stack trace
- [ ] State: panics are not recoverable (no `catch` for panics — use `Result` for recoverable errors)

---

## 07.5 Panic Handler Semantics

**File:** `docs/ori_lang/v2026/spec/23-program-execution.md`

- [ ] `@panic (info: PanicInfo) -> void` — optional, at most one per program
- [ ] When present: called instead of default handler on panic
- [ ] `print(msg:)` inside `@panic` writes to stderr (not stdout)
- [ ] Re-panic inside `@panic` → immediate termination (no recursive handler)
- [ ] Double panic: if a panic occurs during Drop → immediate termination
- [ ] Cross-reference to `PanicInfo` type definition

---

## 07.6 Completion Checklist

- [ ] Module init order algorithm specified (deterministic topological sort)
- [ ] Process environment: args, exit codes, std streams documented
- [ ] Normal termination with Drop ordering specified
- [ ] Panic termination behavior specified (unwind vs abort decision made)
- [ ] Runtime panic catalogue is complete — every panic condition in one table
- [ ] Panic handler semantics (re-panic, double-panic) specified
- [ ] Each panic condition cross-references its source clause
- [ ] All additions use ISO normative style

**Exit Criteria:** A reader can find every possible runtime panic in one table, and can determine from §23 alone what happens when a program starts, runs, and terminates in both normal and exceptional cases.
