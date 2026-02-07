---
section: "15"
title: Diagnostics & Error Reporting
status: not-started
goal: Structured codegen diagnostics with E4xxx (ori_arc) and E5xxx (ori_llvm) error codes, following the Problem/Reporting pattern
sections:
  - id: "15.1"
    title: Existing Diagnostic Infrastructure
    status: not-started
  - id: "15.2"
    title: Codegen Error Codes
    status: not-started
  - id: "15.3"
    title: CodegenProblem Type
    status: not-started
  - id: "15.4"
    title: Cross-Section Integration
    status: not-started
  - id: "15.5"
    title: Error Recovery
    status: not-started
---

# Section 15: Diagnostics & Error Reporting

**Status:** Not Started
**Goal:** Structured codegen error reporting with dedicated error code ranges (E4xxx for `ori_arc`, E5xxx for `ori_llvm`), following the existing Problem/Reporting 1:1 coupling pattern. Replaces panics with proper error accumulation and source-located diagnostics.

**Reference compilers:**
- **Rust** `compiler/rustc_codegen_ssa/src/errors.rs` -- codegen error types with `#[derive(Diagnostic)]`
- **Zig** `src/Compilation.zig` -- `error.CodegenFail`, error accumulation during codegen
- **Gleam** `compiler-core/src/error.rs` -- structured error types with suggestions

**Current state:** `ori_llvm/src/lib.rs` has `install_fatal_error_handler()` for LLVM fatal errors. Most codegen errors are panics or `Option` unwraps. The `oric/src/problem/` and `oric/src/reporting/` modules provide the established pattern for structured diagnostics. V2 extends this pattern to codegen.

---

## 15.1 Existing Diagnostic Infrastructure

### ori_diagnostic Crate

The `ori_diagnostic` crate provides the core types:

- **`Diagnostic`** -- message, severity, error code, labeled spans, notes, suggestions
- **`Severity`** -- Error, Warning, Note, Help
- **`ErrorCode`** -- enum with `E####` format, phase-based first digit:
  - E0xxx: Lexer errors (16 codes)
  - E1xxx: Parser errors (15 codes)
  - E2xxx: Type errors (18 codes)
  - E3xxx: Pattern errors (3 codes)
  - E9xxx: Internal compiler errors (2 codes)
  - **E4xxx: Reserved for ori_arc (V2)**
  - **E5xxx: Reserved for ori_llvm (V2)**
- **Emitters** -- terminal (ANSI colors), JSON, SARIF output formats

### oric/src/problem/ -- Data Layer

Structured problem types, one per compilation phase:

| Type | Phase | Error Range |
|------|-------|-------------|
| `LexProblem` | Lexer | E0xxx |
| `ParseProblem` | Parser | E1xxx |
| `SemanticProblem` | Semantic analysis | E2xxx |
| **`ArcProblem`** | **ARC IR (V2)** | **E4xxx** |
| **`LlvmProblem`** | **LLVM codegen (V2)** | **E5xxx** |

The `Problem` enum wraps all problem types with `From` impls and type predicates. The `HasSpan` trait extracts the primary source location. Macros (`impl_has_span!`, `impl_from_problem!`, `impl_problem_predicates!`) reduce boilerplate.

Note: `ArcProblem` and `LlvmProblem` follow the **direct-variant pattern** (like `ParseProblem`), where each error condition is a variant of the enum itself. This differs from the **wrapper pattern** used by `LexProblem`, which wraps an inner type. The direct-variant pattern is preferred for codegen because each error variant has distinct fields.

### oric/src/reporting/ -- Presentation Layer

The `Render` trait converts problems to `Diagnostic` objects. The 1:1 coupling is intentional: every problem type has a corresponding renderer in the `reporting/` module. The `Report` struct collects diagnostics with `has_errors()`, `error_count()`, `warning_count()` queries.

### Current Codegen Error Handling

The current codegen path uses several ad-hoc error mechanisms:

- `install_fatal_error_handler()` -- catches LLVM internal panics
- Per-module error enums in `ori_llvm` -- `EmitError`, `OptimizationError`, `LinkerError`, `TargetError`, `DebugInfoError`, etc. (not integrated with Problem/Reporting)
- `panic!` on unexpected states during lowering
- `Option::unwrap()` on LLVM operations that should not fail

V2 replaces all of these with structured `ArcProblem`/`LlvmProblem` types that flow through the existing diagnostic pipeline.

- [ ] Add E4xxx and E5xxx ranges to `ErrorCode` enum in `ori_diagnostic`
- [ ] Create `ArcProblem` and `LlvmProblem` types in `oric/src/problem/`
- [ ] Create corresponding renderers in `oric/src/reporting/`
- [ ] Replace codegen panics with error accumulation

---

## 15.2 Codegen Error Codes

### E4xxx: ori_arc Errors

ARC IR transformation errors. These indicate internal compiler issues (user code that reaches codegen has already passed type checking), but they carry source spans for context.

| Code | Name | Description | Span Source |
|------|------|-------------|-------------|
| E4001 | `ArcIrLoweringFailure` | Unreachable expression kind during AST-to-ARC-IR lowering | Expression span |
| E4002 | `BorrowInferenceFailure` | Borrow inference failed to converge after maximum iterations | Function span |
| E4003 | `RcInsertionError` | RC insertion encountered inconsistent liveness state | Instruction span |
| E4004 | `TypeClassificationFailure` | Unresolved generic type reaching codegen (should have been monomorphized) | Type annotation span |
| E4005 | `DecisionTreeFailure` | Pattern compilation produced invalid decision tree | Match expression span |

```rust
// Pseudocode: ArcProblem in oric/src/problem/arc.rs
// Note: Codegen errors are terminal — they do not flow through Salsa queries.
// Only Debug + Clone are needed (for collection and display), not the Salsa-
// compatible Eq/PartialEq/Hash derives that earlier phases require.
#[derive(Debug, Clone)]
pub enum ArcProblem {
    ArcIrLoweringFailure {
        span: Span,
        expr_kind: String,
        message: String,
    },
    BorrowInferenceFailure {
        span: Span,
        function_name: String,
        iterations: u32,
    },
    RcInsertionError {
        span: Span,
        variable: String,
        message: String,
    },
    TypeClassificationFailure {
        span: Span,
        type_name: String,
    },
    DecisionTreeFailure {
        span: Span,
        pattern_count: usize,
        message: String,
    },
}

impl_has_span!(ArcProblem {
    span: [
        ArcIrLoweringFailure,
        BorrowInferenceFailure,
        RcInsertionError,
        TypeClassificationFailure,
        DecisionTreeFailure,
    ],
});
```

### E5xxx: ori_llvm Errors

LLVM backend errors. Some of these (target, linker) may not have source spans because they relate to build configuration rather than specific source code.

| Code | Name | Description | Has Span? |
|------|------|-------------|-----------|
| E5001 | `ModuleVerificationFailed` | LLVM module verification failed (invalid IR) | Sometimes (depends on which instruction failed) |
| E5002 | `PassPipelineError` | LLVM optimization pass pipeline error | No |
| E5003 | `ObjectEmissionFailed` | Failed to emit object file | No |
| E5004 | `TargetNotSupported` | Target triple not recognized or not available | No |
| E5005 | `RuntimeLibraryNotFound` | `libori_rt.a` not found in any search path | No |
| E5006 | `LinkerFailed` | Linker invocation returned non-zero exit code | No |
| E5007 | `DebugInfoCreationFailed` | Debug info creation failed (LLVM DIBuilder error) | Expression span |

```rust
// Pseudocode: LlvmProblem in oric/src/problem/llvm.rs
// Terminal errors — no Salsa derives needed (see ArcProblem note above).
#[derive(Debug, Clone)]
pub enum LlvmProblem {
    ModuleVerificationFailed {
        span: Option<Span>,
        message: String,
    },
    PassPipelineError {
        pipeline: String,
        message: String,
    },
    ObjectEmissionFailed {
        target: String,
        message: String,
    },
    TargetNotSupported {
        triple: String,
        available: Vec<String>,
    },
    RuntimeLibraryNotFound {
        search_paths: Vec<String>,
    },
    LinkerFailed {
        command: String,
        exit_code: Option<i32>,
        stderr: String,
    },
    DebugInfoCreationFailed {
        span: Option<Span>,
        type_name: String,
        message: String,
    },
}
```

Note: `LlvmProblem` uses `Option<Span>` rather than mandatory spans because several error conditions (target/linker/runtime) are not tied to specific source locations. The `HasSpan` implementation returns `Span::DUMMY` for span-less errors, and renderers omit the source annotation in those cases.

- [ ] Add E4001-E4005 to ErrorCode enum
- [ ] Add E5001-E5007 to ErrorCode enum
- [ ] Create `ArcProblem` enum with all variants
- [ ] Create `LlvmProblem` enum with all variants

---

## 15.3 CodegenProblem Type

### Wrapping at oric Level

Following the established pattern, `CodegenProblem` wraps both `ArcProblem` and `LlvmProblem` at the `oric` crate level:

```rust
// Pseudocode: in oric/src/problem/codegen.rs
// Terminal errors — no Salsa derives needed.
#[derive(Debug, Clone)]
pub enum CodegenProblem {
    Arc(ArcProblem),
    Llvm(LlvmProblem),
}

impl_from_problem!(ArcProblem => CodegenProblem::Arc);
impl_from_problem!(LlvmProblem => CodegenProblem::Llvm);
```

### Codegen Errors Are Separate From the Problem Enum (Design Decision)

**Chosen approach:** Codegen errors live in a separate `Vec<CodegenProblem>` and are **not** added as a variant to the `Problem` enum.

**Rationale:** The `Problem` enum is used in Salsa-tracked queries and requires `Clone + Eq + PartialEq + Hash + Debug`. `CodegenProblem` is terminal-only (produced after all Salsa queries complete) and does not need these derives. Adding a `Codegen` variant to `Problem` would either:
- Force Salsa-incompatible derives onto `Problem`, breaking the query system, or
- Require manual `Eq`/`Hash` impls that special-case the `Codegen` variant, adding fragile complexity.

Neither is acceptable. Instead, codegen errors merge with front-end diagnostics only at the **rendering stage**:

```rust
// Problem enum is UNCHANGED — Salsa-compatible, no Codegen variant.
// Codegen errors are collected separately and merged for display only.

/// Codegen errors collected during the codegen pipeline.
/// These are NOT part of the Salsa-tracked Problem enum.
struct CodegenResult {
    object_files: Vec<PathBuf>,
    problems: Vec<CodegenProblem>,
}

/// At the rendering stage, merge all diagnostics for unified output.
fn render_all_diagnostics(
    front_end_problems: &[Problem],
    codegen_problems: &[CodegenProblem],
    interner: &StringInterner,
) -> Vec<Diagnostic> {
    let mut diagnostics: Vec<Diagnostic> = front_end_problems
        .iter()
        .map(|p| p.render(interner))
        .collect();
    diagnostics.extend(
        codegen_problems.iter().map(|p| p.render(interner))
    );
    // Sort by source location when spans are available
    diagnostics.sort_by_key(|d| d.primary_span());
    diagnostics
}
```

This keeps the `Problem` enum clean for Salsa, keeps `CodegenProblem` free of unnecessary trait constraints, and provides unified diagnostic output at the rendering boundary.

### Render Implementation

Create `oric/src/reporting/codegen.rs` with renderers for both `ArcProblem` and `LlvmProblem`. Following the existing pattern, rendering logic lives in `into_diagnostic()` inherent methods on each problem type, with `Render` trait impls delegating to `into_diagnostic()`. This matches how `ParseProblem` and `SemanticProblem` are structured.

```rust
// Pseudocode: inherent method on ArcProblem
impl ArcProblem {
    pub fn into_diagnostic(&self, _interner: &StringInterner) -> Diagnostic {
        match self {
            ArcProblem::BorrowInferenceFailure { span, function_name, iterations } => {
                Diagnostic::error(ErrorCode::E4002)
                    .with_message(format!(
                        "borrow inference for `{function_name}` did not converge \
                         after {iterations} iterations"
                    ))
                    .with_label(*span, "in this function")
                    .with_note("try simplifying the function's parameter usage patterns")
            }
            // ... other variants
        }
    }
}

// Render trait delegates to into_diagnostic()
impl Render for ArcProblem {
    fn render(&self, interner: &StringInterner) -> Diagnostic {
        self.into_diagnostic(interner)
    }
}

// Pseudocode: inherent method on LlvmProblem (some without spans)
impl LlvmProblem {
    pub fn into_diagnostic(&self, _interner: &StringInterner) -> Diagnostic {
        match self {
            LlvmProblem::RuntimeLibraryNotFound { search_paths } => {
                Diagnostic::error(ErrorCode::E5005)
                    .with_message("runtime library `libori_rt.a` not found")
                    .with_note(format!("searched in: {}", search_paths.join(", ")))
                    .with_suggestion("build the runtime with `cargo bl` or `cargo blr`")
            }
            LlvmProblem::LinkerFailed { command, exit_code, stderr } => {
                Diagnostic::error(ErrorCode::E5006)
                    .with_message(format!(
                        "linker failed{}",
                        exit_code.map(|c| format!(" with exit code {c}")).unwrap_or_default()
                    ))
                    .with_note(format!("command: {command}"))
                    .with_note(format!("stderr: {stderr}"))
            }
            // ... other variants
        }
    }
}

// Render trait delegates to into_diagnostic()
impl Render for LlvmProblem {
    fn render(&self, interner: &StringInterner) -> Diagnostic {
        self.into_diagnostic(interner)
    }
}
```

All error messages follow Ori diagnostic guidelines: imperative suggestions ("try X", "use Y"), verb phrase fixes ("Replace X with Y"), and clear context about what went wrong and why.

- [ ] Create `codegen.rs` in `oric/src/problem/`
- [ ] Create `codegen.rs` in `oric/src/reporting/`
- [ ] Keep `Problem` enum unchanged (no `Codegen` variant) -- codegen errors merge at rendering stage only
- [ ] Add `Render` impl for `CodegenProblem`, `ArcProblem`, `LlvmProblem`
- [ ] Implement `render_all_diagnostics()` that merges front-end `Problem`s with `CodegenProblem`s for display

---

## 15.4 Cross-Section Integration

Codegen error codes must be consistent with the error conditions defined in other sections:

### Section 11 (LLVM Passes) References

The existing `ori_llvm` crate uses per-module error enums (not a unified `CodegenError`). The mapping from these crate-local types to diagnostic codes is:

- `EmitError::VerificationFailed` from Section 11's module verification maps to **E5001** (`ModuleVerificationFailed`). The V2 `verify_module()` call converts LLVM's verification error string into an `LlvmProblem::ModuleVerificationFailed`.
- `OptimizationError::PassesFailed` maps to **E5002** (`PassPipelineError`).
- `EmitError::EmissionFailed` (object emission failure) maps to **E5003** (`ObjectEmissionFailed`).
- `TargetError` variants map to **E5004** (`TargetNotSupported`).
- `LinkerError` variants map to **E5006** (`LinkerFailed`).
- `DebugInfoError` variants map to **E5007** (`DebugInfoCreationFailed`).

The `oric` crate wraps these crate-local errors into `LlvmProblem` variants at the boundary (in `oric/src/problem/`), following the established pattern where `ori_llvm` returns its own error types and `oric` converts them.

### Section 07 (RC Insertion) References

- RC insertion encountering inconsistent liveness state maps to **E4003** (`RcInsertionError`). This can occur if a variable is live but has no definition in a predecessor block -- indicating an ARC IR lowering bug.

### Section 06 (Borrow Inference) References

- Borrow inference not converging maps to **E4002** (`BorrowInferenceFailure`). The fixed-point iteration has a maximum iteration count; exceeding it is an error.

### Section 05 (Type Classification) References

- Unresolved generic type reaching codegen maps to **E4004** (`TypeClassificationFailure`). This means monomorphization failed to resolve all type variables before the ARC IR lowering phase.

### Section 10 (Decision Trees) References

- Decision tree construction failure maps to **E4005** (`DecisionTreeFailure`). This can occur if the pattern matrix contains an unhandled pattern kind.

### Section 13 (Debug Info) References

- Debug info creation failure maps to **E5007** (`DebugInfoCreationFailed`). This wraps `DebugInfoError` from `aot/debug.rs` into the diagnostic pipeline.

- [ ] Map existing ori_llvm error types (EmitError, OptimizationError, LinkerError, TargetError, DebugInfoError) to E5xxx codes
- [ ] Map ARC IR transformation errors to E4xxx codes
- [ ] Ensure all error paths have source spans where possible

---

## 15.5 Error Recovery

Not all codegen errors are fatal. The error recovery strategy varies by error type:

### Recoverable Errors

**Module verification (E5001):** When a module fails LLVM verification, report the error and continue to the next module. Other modules may still compile successfully. This is particularly useful in incremental compilation (Section 12) where a single function's bug should not prevent the rest of the codebase from compiling.

**Debug info creation (E5007):** If debug type creation fails, fall back to no debug info for that variable/function. The generated code is still correct -- it just lacks debug information at that point. Log a warning and continue.

### Non-Recoverable Errors

**LLVM assertion failure:** LLVM's internal assertion failures are caught by `install_fatal_error_handler()` and terminate the compiler. These indicate bugs in Ori's IR generation that produce illegal LLVM IR that the verifier did not catch. The fatal handler should:
1. Log the LLVM error message
2. Report an E9001 (internal compiler error) with as much context as possible
3. Abort the process (LLVM's internal state may be corrupted)

**Target not supported (E5004):** If the requested target triple is not available, compilation cannot proceed. Report and exit.

**Runtime library not found (E5005):** Without `libori_rt.a`, linking cannot succeed. Report the error with the searched paths and suggest building the runtime.

### Semi-Recoverable Errors

**Pass pipeline failure (E5002):** If the optimization pipeline fails at a given level, try a lower optimization level as fallback:

```rust
// Pseudocode: optimization fallback
fn optimize_with_fallback(module: &Module, config: &OptimizationConfig) -> Result<(), LlvmProblem> {
    match run_optimization_passes(module, config) {
        Ok(()) => Ok(()),
        Err(e) if config.level > OptimizationLevel::O0 => {
            tracing::warn!("optimization at {:?} failed, falling back to O0: {}", config.level, e);
            let fallback = OptimizationConfig::debug(); // O0
            run_optimization_passes(module, &fallback)
                .map_err(|e2| LlvmProblem::PassPipelineError {
                    pipeline: fallback.pipeline_string(),
                    message: e2.to_string(),
                })
        }
        Err(e) => Err(LlvmProblem::PassPipelineError {
            pipeline: config.pipeline_string(),
            message: e.to_string(),
        }),
    }
}
```

**Linker failure (E5006):** Report the linker's stderr and exit code. No automatic recovery, but the error message should include the full linker command for manual debugging.

### Error Accumulation Pattern

Codegen errors accumulate into a `Vec<CodegenProblem>` that is merged with earlier phase diagnostics at the rendering stage (not into the `Problem` enum -- see Section 15.3):

```rust
// Pseudocode: error accumulation in codegen pipeline
struct CodegenResult {
    object_files: Vec<PathBuf>,
    problems: Vec<CodegenProblem>,
}

// In the compilation driver — merge at rendering time, not at collection time
let codegen_result = compile_modules(typed_modules);
let front_end_problems: Vec<Problem> = parse_problems
    .into_iter()
    .chain(type_problems)
    .collect();
let diagnostics = render_all_diagnostics(
    &front_end_problems,
    &codegen_result.problems,
    &interner,
);
```

This ensures codegen errors appear alongside parse and type errors in the unified diagnostic output, sorted by source location when spans are available, without polluting the Salsa-tracked `Problem` enum.

- [ ] Implement module verification recovery (skip failed module)
- [ ] Implement debug info fallback (degrade gracefully)
- [ ] Implement optimization level fallback
- [ ] Implement error accumulation in codegen pipeline
- [ ] Replace all codegen `panic!` with `LlvmProblem` / `ArcProblem` reporting

**Exit Criteria:** No codegen panics on valid Ori code. Invalid IR is caught by verification and reported with source context. All codegen errors flow through the unified diagnostic pipeline with proper error codes, messages, and suggestions.
