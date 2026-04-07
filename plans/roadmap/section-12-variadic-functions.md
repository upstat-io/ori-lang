---
section: 12
title: Variadic Functions
status: in-progress
last_verified: "2026-03-29"
reviewed: true
tier: 4
goal: Enable functions with variable number of arguments
spec:
  - spec/10-declarations.md
  - spec/26-ffi.md
sections:
  - id: "12.1"
    title: Homogeneous Variadics
    status: partial
  - id: "12.2"
    title: Minimum Argument Count
    status: not-started
  - id: "12.3"
    title: Trait Bounds on Variadics
    status: not-started
  - id: "12.4"
    title: C Variadic Interop
    status: partial
  - id: "12.5"
    title: Variadic in Patterns
    status: deferred
---

# Section 12: Variadic Functions

**Goal**: Enable functions with variable number of arguments

**Criticality**: Medium — API design flexibility, required for C interop

**Dependencies**: Section 11 (FFI — C variadic interop in 12.4), Section 3 (Traits — trait bounds on variadics in 12.3)

**Proposal**: `proposals/approved/variadic-functions-proposal.md`

### Sync Points: Variadic Parameter Type (multi-crate sync required)

Adding variadic parameter support requires updates across these crates:

1. **`ori_ir`** — `Param.is_variadic`, `CallArg.is_spread`, `ExternItem.is_c_variadic`, `ListElement::Spread`, `MapElement::Spread`, `StructLitField::Spread` all defined (COMPLETE, verified 2026-03-29)
2. **`ori_parse`** — Parse `...T` in parameter position, `...expr` spread in call position, C variadic `...` in extern blocks; incremental copier handles `is_variadic` (COMPLETE, verified 2026-03-29)
3. **`ori_types`** — Convert `...T` to `[T]` internally, validate spread type compatibility, enforce "last param only" rule, reject spread in non-variadic calls (GAP-12-003: `is_variadic` never read by typeck)
4. **`ori_eval`** — Collect variadic args into list at call site, expand spread operator (GAP-12-004: `is_spread` on `CallArg` never handled)
5. **`ori_llvm`** — Codegen for variadic arg collection (stack allocation or heap list), C variadic ABI (`va_list`) for extern functions (GAP-12-005: no variadic codegen)
6. **`ori_canon`** — Collection spread desugaring exists (`ori_canon/src/desugar/spread.rs`) but no variadic-specific call-site desugaring
7. **`ori_fmt`** — `is_c_variadic` handled for extern blocks; `is_variadic` on regular params NOT handled (GAP-12-001); `is_spread` in call args NOT handled (GAP-12-002)

---

## Design Decisions

| Question | Decision | Rationale |
|----------|----------|-----------|
| Type safety | Homogeneous only | Type safety over flexibility |
| Syntax | `...T` parameter | Clear, matches common convention |
| C variadics | Separate syntax | Different semantics require distinction |
| Minimum args | Configurable | Allow `printf`-style (min 1) or `sum`-style (min 0) |

---

## Reference Implementation

### Rust

Rust doesn't have variadic functions in the language; uses macros instead.
```
~/projects/reference_repos/lang_repos/rust/library/core/src/fmt/mod.rs    # format! handles variadics via macro
```

### Go

```
~/projects/reference_repos/lang_repos/golang/src/go/types/signature.go    # Variadic signature handling
~/projects/reference_repos/lang_repos/golang/src/cmd/compile/internal/types2/signature.go
```

### Swift

```
# Swift has variadic generics (parameter packs: each T, repeat each T)
~/projects/reference_repos/lang_repos/swift/lib/Sema/CSSimplify.cpp       # Pack expansion type checking
~/projects/reference_repos/lang_repos/swift/include/swift/AST/Types.h     # PackExpansionType
```

---

## 12.1 Homogeneous Variadics

**Spec section**: `spec/10-declarations.md § Variadic Parameters`

### Syntax

```ori
// Variadic parameter (receives as list)
@sum (numbers: ...int) -> int = {
    numbers.fold(initial: 0, op: (acc, n) -> acc + n)
}

// Usage
sum(1, 2, 3)        // 6
sum(1)              // 1
sum()               // 0

// With required parameters before
@printf (format: str, args: ...Printable) -> str = ...

printf("Hello")                    // "Hello"
printf("{} + {} = {}", 1, 2, 3)   // "1 + 2 = 3"

// Spread operator to pass list as varargs
let nums = [1, 2, 3]
sum(...nums)        // 6
sum(0, ...nums, 4)  // 10
```

### Grammar

```ebnf
Parameter        = Identifier ':' Type | VariadicParameter ;
VariadicParameter = Identifier ':' '...' Type ;
SpreadExpr       = '...' Expression ;
```

### Type Rules

- Variadic parameter must be last
- Only one variadic parameter allowed
- Parameter type `...T` becomes `[T]` inside function
- Spread `...list` requires `list: [T]` where `T` matches variadic

### Implementation

- [x] **Spec**: Variadic parameter syntax (verified 2026-03-29)
  - [x] Parameter syntax `...T` — `spec/10-declarations.md` Section 10.1.3
  - [x] Spread operator `...expr` — `spec/10-declarations.md`
  - [x] Type rules — constraints, type inference rules, function type representation
  - [x] Grammar — `grammar.ebnf` lines 304-305: `variadic_param`, `spread_arg` productions

- [x] **Lexer**: `DotDotDot` token (verified 2026-03-29)
  - [x] Three-dot token — `ori_lexer_core` raw scanner, `ori_lexer` cooker
  - [x] Distinguish from range `..` — separate `DotDot` vs `DotDotDot` tokens

- [x] **IR (AST)**: Variadic AST nodes (verified 2026-03-29)
  - [x] `Param.is_variadic: bool` at `ori_ir/src/ast/items/function.rs:97`
  - [x] `CallArg.is_spread` at `ori_ir/src/ast/collections.rs:143`
  - [x] `ListElement::Spread`, `MapElement::Spread`, `StructLitField::Spread`

- [x] **Parser**: Parse variadic parameters (verified 2026-03-29)
  - [x] In function signatures — `ori_parse/src/grammar/item/function/mod.rs:489-494`
  - [x] Spread in call expressions — `ori_parse/src/grammar/expr/postfix.rs:392-423`
  - [x] `is_variadic` copied in `ori_parse/src/incremental/copier.rs:861`

- [ ] **Type checker**: Variadic type rules (GAP-12-003: `is_variadic` never read in `ori_types`)
  - [ ] Convert `...T` to `[T]` internally
  - [ ] Check spread type compatibility
  - [ ] Infer element type
  - [ ] Reject spread in non-variadic calls (GAP-12-007: spec requires error, not implemented)

- [ ] **Evaluator**: Handle variadic calls (GAP-12-004: `is_spread` on `CallArg` never handled)
  - [ ] Collect args into list
  - [ ] Handle spread expansion
  - [ ] Mixed literal and spread

- [ ] **Formatter**: Emit variadic/spread syntax (GAP-12-001, GAP-12-002)
  - [ ] Emit `...` before type for `is_variadic` params — round-tripping would lose variadic marker
  - [ ] Emit `...` before spread args in calls — round-tripping would lose spread syntax

- [ ] **ori_canon**: Call-site variadic spread desugaring
  - [ ] Update `ori_canon/src/desugar/spread.rs` for variadic call-site spread (collection spread exists)

- [ ] **LLVM Support**: LLVM codegen for homogeneous variadics (GAP-12-005)
- [ ] **LLVM Rust Tests**: `ori_llvm/tests/variadic_tests.rs` — homogeneous variadics codegen
- [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Test**: `tests/spec/functions/variadic.ori`
  - [ ] Basic variadic function
  - [ ] With required parameters
  - [ ] Spread operator
  - [ ] Empty variadic call

HYGIENE: `tests/spec/declarations/functions.ori` lines 92-120 contain commented-out variadic test code. These should be converted to `#skip` tests or removed with a plan item tracking them.

- [ ] **Subsection close-out (12.1)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-12.1 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 12.1: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 12.2 Minimum Argument Count

**Spec section**: `spec/10-declarations.md § Variadic Constraints`

### Syntax

```ori
// Require at least one argument
@max (first: int, rest: ...int) -> int = {
    rest.fold(initial: first, op: (a, b) -> if a > b then a else b)
}

max(1, 2, 3)   // 3
max(5)         // 5
max()          // Error: max requires at least 1 argument

// Explicit minimum (alternative syntax consideration)
@format (fmt: str, args: ...Printable) -> str = ...
// Minimum 1 (fmt) is implicit from required param
```

### Implementation

- [ ] **Spec**: Minimum argument rules
  - [ ] Required params before variadic
  - [ ] Error messages

- [ ] **Type checker**: Validate minimum args
  - [ ] Count required params
  - [ ] Error if insufficient

- [ ] **Diagnostics**: Clear error messages
  - [ ] "expected at least N arguments, got M"
  - [ ] Show required vs optional

- [ ] **LLVM Support**: LLVM codegen for minimum argument validation
- [ ] **LLVM Rust Tests**: `ori_llvm/tests/variadic_tests.rs` — minimum argument count codegen
- [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Test**: `tests/spec/functions/variadic_min.ori`
  - [ ] Minimum 1 with required param
  - [ ] Minimum 0 (variadic only)
  - [ ] Error cases

- [ ] **Subsection close-out (12.2)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-12.2 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 12.2: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 12.3 Trait Bounds on Variadics

**Spec section**: `spec/10-declarations.md § Variadic Trait Bounds`

### Syntax

```ori
// Generic variadic with trait bound
@print_all<T: Printable> (items: ...T) -> void = {
    for item in items do print(item.to_str())
}

print_all(1, 2, 3)              // OK: all int
print_all("a", "b", "c")        // OK: all str
print_all(1, "a")               // Error: mixed types

// With explicit heterogeneous (trait object)
@print_any (items: ...Printable) -> void = {
    for item in items do print(item.to_str())
}

print_any(1, "hello", true)     // OK: all Printable
```

### Implementation

- [ ] **Spec**: Trait bounds on variadics
  - [ ] Homogeneous generics
  - [ ] Trait object variadics
  - [ ] Error messages

- [ ] **Type checker**: Bound validation
  - [ ] All args satisfy bound
  - [ ] Infer common type
  - [ ] Trait object boxing

- [ ] **LLVM Support**: LLVM codegen for trait bounds on variadics
- [ ] **LLVM Rust Tests**: `ori_llvm/tests/variadic_tests.rs` — variadic trait bounds codegen
- [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Test**: `tests/spec/functions/variadic_bounds.ori`
  - [ ] Generic variadic
  - [ ] Trait object variadic
  - [ ] Bound violations

- [ ] **Subsection close-out (12.3)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-12.3 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 12.3: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 12.4 C Variadic Interop

**Spec section**: `spec/26-ffi.md § C Variadics`

### Syntax

```ori
// Declare C variadic function
extern "c" {
    @printf (format: CPtr, ...) -> c_int  // C-style variadic
}

// Call with any types (unsafe, no type checking)
unsafe { printf("Number: %d, String: %s\n".as_c_str(), 42, "hello".as_c_str()) }
```

### Distinction from Ori Variadics

| Feature | Ori `...T` | C `...` |
|---------|--------------|---------|
| Type safety | Homogeneous, checked | Unchecked |
| Context | Safe code | Unsafe only |
| Implementation | List | va_list ABI |

### Implementation

- [x] **Spec**: C variadic syntax (verified 2026-03-29)
  - [x] Extern function with `...` — `spec/26-ffi.md` Section 26.4.11
  - [x] No type after `...` — `grammar.ebnf` lines 214-216: `c_variadic = "," "..."`
  - [x] Unsafe requirement

- [x] **IR (AST)**: `ExternItem.is_c_variadic: bool` at `ori_ir/src/ast/items/extern_def.rs:47` (verified 2026-03-29)

- [x] **Parser**: Parse C variadics (verified 2026-03-29)
  - [x] `...` without type in extern — `ori_parse/src/grammar/item/extern_def.rs`
  - [x] Distinguish from Ori variadics

- [x] **Formatter**: `ori_fmt/src/declarations/extern_def.rs:55-60` handles `is_c_variadic` (verified 2026-03-29)

- [x] **Debug output**: `oric/src/commands/debug.rs:57` displays variadic in debug (verified 2026-03-29)

- [ ] **Type checker**: C variadic rules (GAP-12-006: no unsafe enforcement for C variadic callers)
  - [ ] Must be in extern block
  - [ ] Caller must be unsafe
  - [ ] No type inference

- [ ] **Codegen**: va_list ABI
  - [ ] Platform-specific ABI
  - [ ] Argument passing conventions

- [ ] **LLVM Support**: LLVM codegen for C variadic interop
- [ ] **LLVM Rust Tests**: `ori_llvm/tests/variadic_tests.rs` — C variadic interop codegen
- [ ] **AOT Tests**: No AOT coverage yet

- [x] **Parser Tests**: `test_extern_c_variadic` and `test_extern_c_variadic_no_params` in `oric/tests/phases/parse/extern_def.rs:97-112` -- 2 tests, both pass (verified 2026-03-29) WEAK TESTS — parse-only, no codegen/end-to-end

- [ ] **Test**: `tests/spec/ffi/c_variadics.ori`
  - [ ] printf call
  - [ ] Mixed argument types
  - [ ] Requires unsafe

- [ ] **Subsection close-out (12.4)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-12.4 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 12.4: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 12.5 Variadic in Patterns

**Spec section**: `spec/15-patterns.md § Variadic Patterns`

### Consideration

```ori
// Should variadic work in patterns?
@process_commands (commands: ...(str, int)) -> void = {
    for (name, priority) in commands do
        print(`Command: {name}, Priority: {priority}`)
}

process_commands(("init", 1), ("run", 2), ("cleanup", 3))
```

### Decision

Defer to future consideration. Current section focuses on function parameters only.

- [ ] **Subsection close-out (12.5)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-12.5 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 12.5: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## Section Completion Checklist

- [ ] All implementation items above have checkboxes marked `[x]`
- [x] Spec updated: `spec/10-declarations.md` variadic section complete (verified 2026-03-29)
- [ ] CLAUDE.md updated with variadic syntax
- [ ] Homogeneous variadics work (typeck/eval/codegen not started)
- [ ] Spread operator works (typeck/eval/codegen not started)
- [ ] C variadic interop works (after Section 11)
- [ ] Commented-out test code in `tests/spec/declarations/functions.ori` lines 92-120 cleaned up
- [ ] All tests pass: `./test-all.sh`
- [ ] `/tpr-review` passed — independent Codex review found no critical or major issues (or all findings triaged)
- [ ] `/impl-hygiene-review` passed — implementation hygiene review clean (phase boundaries, SSOT, algorithmic DRY, naming). MUST run AFTER `/tpr-review` is clean.
- [ ] `/improve-tooling` retrospective completed — MANDATORY at section close, after both reviews are clean. Reflect on the section's debugging journey (which `diagnostics/` scripts you ran, which command sequences you repeated, where you added ad-hoc `dbg!`/`tracing` calls, where output was hard to interpret) and identify any tool/log/diagnostic improvement that would have made this section materially easier OR that would help the next section touching this area. Implement every accepted improvement NOW (zero deferral) and commit each via SEPARATE `/commit-push`. The retrospective is mandatory even when nothing felt painful — that is exactly when blind spots accumulate. See `.claude/skills/improve-tooling/SKILL.md` "Retrospective Mode" for the full protocol.

**Exit Criteria**: Can implement `format()` and call C's `printf()`

---

## Verification Summary (2026-03-29)

**Overall completion**: ~20% — parse/IR infrastructure exists but no typeck/eval/codegen/tests

| Subsection | Plan Status | Actual Status | Test Classification |
|------------|-------------|---------------|---------------------|
| 12.1 Homogeneous Variadics | partial | PARTIAL (parse/IR only) | NEEDS TESTS |
| 12.2 Minimum Argument Count | not-started | NOT STARTED | NEEDS TESTS |
| 12.3 Trait Bounds on Variadics | not-started | NOT STARTED | NEEDS TESTS |
| 12.4 C Variadic Interop | partial | PARTIAL (parse only) | WEAK TESTS (parse-only, 2 tests) |
| 12.5 Variadic in Patterns | deferred | NOT APPLICABLE | NOT APPLICABLE |

### Gap Registry

| Gap ID | Description | Phases | Severity |
|--------|-------------|--------|----------|
| GAP-12-001 | `ori_fmt` does not handle `is_variadic` on function params — round-trip loses variadic | Parse -> Fmt | Major |
| GAP-12-002 | `ori_fmt` does not handle `is_spread` in call args — round-trip loses spread | Parse -> Fmt | Major |
| GAP-12-003 | Type checker never reads `is_variadic` — params parsed but not type-checked as variadic | Parse -> Typeck | Critical |
| GAP-12-004 | Evaluator never handles `is_spread` on `CallArg` — spread parsed but not evaluated | Parse -> Eval | Critical |
| GAP-12-005 | No LLVM codegen for variadic arg collection or spread in calls | Typeck -> Codegen | Critical |
| GAP-12-006 | No unsafe enforcement for C variadic callers | Typeck | Major |
| GAP-12-007 | Spread in non-variadic calls not rejected (spec says it should be an error) | Typeck | Major |

### Hygiene Issues

- HYGIENE VIOLATION: `tests/spec/declarations/functions.ori` lines 92-120 contain commented-out variadic test code. Should be `#skip` tests or removed with plan item tracking.

---

## Example: Format Function

```ori
// Ori's format function (like Python's format or Rust's format!)
@format (template: str, args: ...Printable) -> str = {
    let mut result = ""
    let mut arg_index = 0
    let mut i = 0

    loop {
        if i >= template.len() then break result

        if template[i] == "{" && i + 1 < template.len() && template[i + 1] == "}" then {
            if arg_index >= args.len() then
                panic("Not enough arguments for format string")
            result = result + args[arg_index].to_str()
            arg_index = arg_index + 1
            i = i + 2
        }
        else {
            result = result + template[i]
            i = i + 1
        }
    }
}

// Usage
let msg = format("{} + {} = {}", 1, 2, 3)  // "1 + 2 = 3"
```
