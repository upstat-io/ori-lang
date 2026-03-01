---
title: "Codegen Verification"
description: "Ori Compiler Design — In-Pipeline LLVM IR Verification"
order: 1004
section: "LLVM Backend"
---

# Codegen Verification

## Overview

The `compiler/ori_llvm/src/verify/` module provides in-pipeline static analysis
that walks in-memory LLVM IR via inkwell to detect RC lifecycle bugs, COW
sequencing violations, ABI mismatches, and safety check density anomalies. Unlike
tools that parse textual IR with regex, the audit operates on live inkwell IR
objects -- the same `Module`, `FunctionValue`, and `InstructionValue` types used
during codegen. This eliminates parsing fragility and runs during compilation
itself, catching issues before the module reaches optimization or JIT execution.

The audit is gated behind the `ORI_AUDIT_CODEGEN=1` environment variable. When
disabled (the default), no data structures are allocated, no IR is walked, and no
functions are called. The gating uses `dbg_do!` in the AOT path (`oric`) and a
raw `std::env::var` check in the JIT path (`ori_llvm`), since `ori_llvm` cannot
depend on `oric`. Zero overhead in both release and ungated debug builds.

The module entry point is `audit_module()`, which reads `AuditOptions` from the
environment and dispatches to four independent check modules:

```
rc_balance::check_module(module, options, &mut report);
cow_rules::check_module(module, options, &mut report);
abi_check::check_module(module, options, &mut report);
safety_checks::check_module(module, options, &mut report);
```

Each module iterates over every function in the LLVM module (skipping
declarations without bodies), applies the function filter if set, and appends
findings to the shared `AuditReport`.

## Environment Variables

Three environment variables control the audit. Their string constants are defined
as `ENV_AUDIT_CODEGEN`, `ENV_AUDIT_STRICT`, and `ENV_AUDIT_FUNCTION` in
`verify/mod.rs`, and mirrored in `oric/src/debug_flags.rs`. A compile-time
`const` assertion in `debug_flags.rs` verifies the two sets of constants remain
in sync -- if either side renames a flag, the build fails.

| Variable              | Default | Effect                                                                |
|-----------------------|---------|-----------------------------------------------------------------------|
| `ORI_AUDIT_CODEGEN`   | unset   | Set to `1` to enable the audit pipeline. Any value other than `0`.    |
| `ORI_AUDIT_STRICT`    | unset   | Set to `1` for pessimistic mode (see Strict Mode below).              |
| `ORI_AUDIT_FUNCTION`  | unset   | Substring filter: only audit functions whose LLVM name contains this. |

These compose freely. For example:

```bash
ORI_AUDIT_CODEGEN=1 ORI_AUDIT_STRICT=1 ORI_AUDIT_FUNCTION=process ori build file.ori
```

audits only functions whose name contains `process`, in strict mode.

## What It Checks

### 1. RC Balance (`rc_balance.rs`)

Forward-walks each function's instructions tracking pointer states through the RC
lifecycle. Every call to `ori_rc_alloc` records the result SSA name as `Live`.
Calls to `ori_rc_dec` transition the pointer to `Decremented`. COW function calls
(detected via the `ori_*_cow` naming convention) transition the pointer to
`CowConsumed`. At function exit, any pointer still in `Live` state is flagged.

State machine:

```
ori_rc_alloc    ->  Live
COW function    ->  CowConsumed  (or Decremented in strict mode)
ori_rc_dec      ->  Decremented
```

Findings:

| Situation                                    | Finding Kind      | Severity |
|----------------------------------------------|-------------------|----------|
| Pointer `Live` at function exit              | `RcLeak`          | Warning (Error in strict mode) |
| `ori_rc_dec` on `CowConsumed` pointer        | `RcDecAfterCow`   | Warning  |
| `ori_rc_dec` on already `Decremented` pointer | `RcDoubleDec`    | Error (strict mode only) |

The tracker uses an `FxHashMap<String, PtrState>` keyed by SSA name. This is a
linear walk, not full CFG dataflow -- it handles straight-line RC patterns (95%+
of codegen output) but may miss conditional RC paths. By design it produces false
negatives (misses), never false positives.

### 2. COW Sequencing (`cow_rules.rs`)

Validates three sequencing rules for Copy-on-Write operations:

**Rule 1 -- No reuse after COW.** After `ori_list_push_cow(data_ptr, ...)`, any
subsequent use of `data_ptr` (in a call argument or non-call instruction operand)
other than `ori_rc_dec` is a `CowInputReusedAfterCall` error. The COW function
may have reallocated, invalidating the old pointer.

**Rule 2 -- COW output extraction.** Structurally impossible to violate because
COW functions return `void` and write to an output alloca. No check needed.

**Rule 3 -- No dec before COW.** If `ori_rc_dec(ptr)` fires before
`ori_list_push_cow(ptr, ...)`, the COW function receives a potentially freed
pointer. Produces a `CowInputDecBeforeCall` error.

The module tracks two sets per function: `cow_consumed` (pointers passed to COW
functions) and `decremented` (pointers passed to `ori_rc_dec`). It walks all
instructions and checks call/non-call operands against these sets.

### 3. ABI Conformance (`abi_check.rs`)

Three independent sub-checks on calling conventions:

**A. Large aggregate loads.** Detects `load %StructType, ptr` where the struct
exceeds 16 bytes. These trigger LLVM FastISel bugs in JIT mode -- the correct
pattern is per-field `struct_gep` + `load` + `insert_value`. The size is computed
conservatively by `estimated_type_size()` which sums field sizes without padding
(real structs are only larger). Findings are `LargeAggregateLoad` warnings.

**B. Runtime arg count mismatch.** Checks calls to `ori_*` runtime functions
against the `RT_FUNCTIONS` table in `codegen/runtime_decl/runtime_functions.rs`,
the single source of truth for runtime function signatures. If a call has a
different operand count than the declared parameter count, it produces a
`RuntimeArgCountMismatch` error. The operand count subtracts one because LLVM
includes the callee itself as the last operand of call/invoke instructions.

**C. Nounwind + invoke conflict.** Functions marked `Nounwind` in `RT_FUNCTIONS`
should be called with `call`, not `invoke` (which generates unnecessary landing
pads). Produces a `NounwindCalledWithInvoke` warning.

### 4. Safety Check Density (`safety_checks.rs`)

Analyzes how many panic/assert checks appear relative to total instruction count.
This is purely informational (all findings are `Note` severity).

The analysis uses a two-pass approach per function:

1. **Pass 1**: Identify "panic blocks" -- basic blocks containing calls to safety
   functions (`ori_panic`, `ori_assert*`, or any runtime function with `Attr::Cold`).
2. **Pass 2**: Count safety calls and conditional branches targeting panic blocks.
   Compute density as `check_count / instruction_count * 100`.

Findings:

| Finding Kind           | Meaning                                          |
|------------------------|--------------------------------------------------|
| `SafetyCheckCall`      | A direct call to a panic/assert runtime function  |
| `SafetyCheckBranch`    | A conditional branch targeting a panic block       |
| `SafetyCheckSummary`   | Per-function density: N checks in M instructions   |

Unconditional branches to panic blocks are not counted -- they represent
inevitable panics, not guard checks.

## Strict Mode

When `ORI_AUDIT_STRICT=1` is set, the audit makes pessimistic assumptions:

1. **COW treated as freeing.** COW consumption transitions the pointer directly
   to `Decremented` instead of `CowConsumed`. Any subsequent `ori_rc_dec` is a
   definite `RcDoubleDec` error, not a `RcDecAfterCow` warning.

2. **Parameters tracked as RC-managed.** Function pointer parameters are recorded
   as `Live` at entry. If a function receives a pointer but never decrements it,
   strict mode flags an `RcLeak` error. Normal mode ignores parameters (they may
   be borrowed, not owned).

3. **Leaks elevated to errors.** RC leaks that are `Warning` in normal mode
   become `Error` in strict mode.

Strict mode may produce false positives by design. It is intended for focused
investigation of suspected RC bugs, not routine CI use.

## Function Filtering

Setting `ORI_AUDIT_FUNCTION=name` restricts the audit to functions whose LLVM
name contains the substring `name`. The filter is applied via
`should_audit_fn()` (in `rc_balance.rs`, reused by all four modules). Functions
that do not match are silently skipped. This is useful for large programs where
auditing every function would produce too much output.

## Reporting

All findings are collected in an `AuditReport`:

```rust
pub struct AuditReport {
    pub findings: Vec<AuditFinding>,
}
```

Each finding contains:

```rust
pub struct AuditFinding {
    pub function_name: String,  // LLVM function where the finding was detected
    pub severity: Severity,     // Error, Warning, or Note
    pub kind: FindingKind,      // Structured discriminant for filtering/testing
    pub description: String,    // Human-readable explanation
}
```

All strings are owned (`String`, not `&str`) because the report must outlive
inkwell's LLVM context.

`AuditReport::emit_to_stderr()` prints each finding in the format:

```
codegen audit: {severity}: [{function_name}] {description}
```

followed by a summary line:

```
codegen audit summary: N error(s), M warning(s), K note(s)
```

### Exit Behavior

The audit **can** fail the build. In both the AOT path (`codegen_pipeline.rs`)
and the JIT path (`evaluator/compile.rs`), if `audit_report.has_errors()` returns
true after `emit_to_stderr()`, compilation aborts with an error message reporting
the error count. Warnings and notes do not block compilation.

In the AOT path, the audit runs inside `dbg_do!` (zero cost in release builds).
In the JIT path, the audit runs behind a raw `audit_requested()` check (the
`ori_llvm` crate cannot depend on `oric` for the `dbg_do!` macro).

## The `codegen-audit.sh` Diagnostic Script

The shell script `diagnostics/codegen-audit.sh` wraps the in-pipeline audit with
a convenient CLI:

```bash
diagnostics/codegen-audit.sh [options] <file.ori>
```

**Options:**

| Flag                  | Effect                                         |
|-----------------------|------------------------------------------------|
| `--strict`            | Sets `ORI_AUDIT_STRICT=1`                       |
| `--function <name>`   | Sets `ORI_AUDIT_FUNCTION=<name>`                |
| `--color` / `--no-color` | Force or disable colored output (default: auto) |
| `-h` / `--help`       | Show usage                                      |

The script invokes `ori build` with `ORI_AUDIT_CODEGEN=1` and captures stderr.
It then parses lines matching `^codegen audit:` from the build output, colorizes
them (errors in red, warnings in yellow, summary in bold), and determines the
exit code from the summary line.

**Exit codes:**

| Code | Meaning                                    |
|------|--------------------------------------------|
| `0`  | Clean -- no findings                        |
| `1`  | Findings detected (errors or warnings)      |
| `2`  | Usage error or compilation failure           |

The script cleans up any compiled binary produced by `ori build` after the audit
completes.

## Limitations

The audit uses a **linear forward scan** per function, not full CFG dataflow
analysis. This is a deliberate trade-off of precision for speed and simplicity.

**Handles well (~95% of RC patterns):**
- Straight-line RC sequences: alloc, use, dec
- Simple COW patterns: uniqueness check, branch, cow_fn or copy
- Drop function bodies (linear cleanup sequences)

**May miss (false negatives):**
- Conditional RC operations where only one branch decrements
- Loop-carried RC state (accumulating RC values in a loop)
- Inter-procedural RC flow (value passed to another function that handles dec)
- Pointer aliasing through GEP/extractvalue chains (only SSA names are tracked)

**Never produces:**
- False positives for Error-severity findings in normal mode. Every Error
  finding represents a real violation of the RC protocol. Strict mode may produce
  false positives by design as it makes pessimistic assumptions.

## Extending the Audit

To add a new audit check:

1. Create a new module in `compiler/ori_llvm/src/verify/` (e.g., `phi_check.rs`)
2. Implement `pub fn check_module(module: &Module, options: &AuditOptions, report: &mut AuditReport)`
3. Add new `FindingKind` variant(s) to `report.rs`
4. Wire into `verify/mod.rs` by calling the new check in `audit_module_with_options()`
5. Add unit tests in `verify/tests.rs` using synthetic inkwell IR
6. Export the module in `verify/mod.rs`

Each module is independent -- adding a new check does not affect existing modules.
The shared `AuditReport` and `AuditOptions` provide a uniform interface across
all checks.
