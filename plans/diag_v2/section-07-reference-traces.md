---
section: "07"
title: Reference Traces
status: not-started
goal: Show how errors are reached through dependency graphs and import chains
sections:
  - id: "07.1"
    title: Trace Collection
    status: not-started
  - id: "07.2"
    title: Trace Rendering
    status: not-started
  - id: "07.3"
    title: Integration Points
    status: not-started
  - id: "07.4"
    title: Completion Checklist
    status: not-started
---

# Section 07: Reference Traces

**Status:** Not Started
**Goal:** Collect and render "reached via A → B → C" reference traces for errors that propagate through dependency graphs, import chains, or type instantiation paths. This helps users understand *how* an error in one module affects their code.

**Reference compilers:**
- **Zig** `src/Compilation.zig` — `ResolvedReference` traces through dependency graph; shows "referenced here" chains for compilation errors
- **Rust** `compiler/rustc_middle/src/ty/print/` — "required by this bound in `Trait`" with chains showing where constraints originate
- **TypeScript** `src/compiler/checker.ts` — `DiagnosticRelatedInformation[]` chains for import/re-export errors

**Current state:** No reference trace infrastructure. Errors from imported modules appear without context about how the import chain led to the error. Circular dependency errors show the cycle but not the path the compiler took to discover it.

---

## 07.1 Trace Collection

### ReferenceTrace Type

```rust
// In oric/src/traces.rs (new module)

/// A chain of references showing how an error was reached.
///
/// Each step represents one hop through the dependency graph:
/// function call, import, type instantiation, etc.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct ReferenceTrace {
    /// The steps from the user's code to the error site.
    /// First step is closest to the user's code; last step is at the error.
    pub steps: Vec<TraceStep>,
}

/// One step in a reference trace.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct TraceStep {
    /// What kind of reference this step represents.
    pub kind: TraceKind,

    /// Source location of this reference.
    pub span: Span,

    /// Human-readable description (e.g., "imported here", "called here").
    pub message: String,

    /// File path if this step is in a different file.
    pub file: Option<String>,
}

/// The kind of reference in a trace step.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum TraceKind {
    /// An import statement.
    Import,
    /// A function call.
    Call,
    /// A type instantiation (generic specialization).
    TypeInstantiation,
    /// A trait/interface implementation.
    Implementation,
    /// A re-export.
    ReExport,
    /// A field access chain.
    FieldAccess,
    /// A test targeting a function.
    TestTarget,
}
```

### Collection Strategy

Traces are collected lazily — only when an error occurs. This avoids any performance impact on successful compilations.

**Import chain traces:**

```rust
/// Collect the import chain from the current module to the target.
///
/// Called when an import error occurs to show the user how they
/// reached the problematic import.
pub fn collect_import_trace(
    db: &dyn CompilerDb,
    from_module: ModuleId,
    to_module: ModuleId,
) -> Option<ReferenceTrace>
```

**Call chain traces:**

```rust
/// Collect the call chain for a function-level error.
///
/// Shows how the user's code reached a function that has an error.
pub fn collect_call_trace(
    db: &dyn CompilerDb,
    call_site: Span,
    error_site: Span,
) -> Option<ReferenceTrace>
```

**Type instantiation traces:**

```rust
/// Collect the instantiation chain for a generic type error.
///
/// Shows how type parameters were specialized to reach the error.
pub fn collect_type_trace(
    db: &dyn CompilerDb,
    generic_site: Span,
    instantiation_site: Span,
) -> Option<ReferenceTrace>
```

- [ ] Define `ReferenceTrace`, `TraceStep`, `TraceKind` types
- [ ] Implement `collect_import_trace()`
- [ ] Implement `collect_call_trace()`
- [ ] Implement `collect_type_trace()`
- [ ] Tests: import chain collection
- [ ] Tests: call chain collection

---

## 07.2 Trace Rendering

### Converting to ExplanationChain

Reference traces convert to `ExplanationChain` (from Section 02) for rendering:

```rust
impl ReferenceTrace {
    /// Convert this trace to an ExplanationChain for diagnostic rendering.
    pub fn to_explanation_chain(&self) -> ExplanationChain {
        let mut chain = ExplanationChain::new(
            self.steps.last().map(|s| s.message.clone())
                .unwrap_or_default()
        );

        // Build chain from innermost to outermost
        for step in self.steps.iter().rev().skip(1) {
            let mut link = ExplanationChain::new(&step.message)
                .with_span(step.span);
            if let Some(ref file) = step.file {
                link = link.with_source(SourceInfo {
                    path: file.clone(),
                    content: String::new(), // Loaded lazily by emitter
                });
            }
            chain = link.because(chain);
        }

        chain
    }
}
```

### Example Output

```
error[E2005]: imported item `Config` not found in module `lib`
  --> src/main.ori:3:1
   |
 3 | use lib.Config
   | ^^^^^^^^^^^^^^ not found in `lib`
   |
   = reached via:
     --> src/main.ori:3:1
      |
    3 | use lib.Config
      | imported here
     --> src/lib.ori:1:1
      |
    1 | use core.Config
      | re-exported from `core`
     --> src/core.ori
      | `Config` was removed in a recent change
```

- [ ] Implement `to_explanation_chain()` conversion
- [ ] Implement `to_related_information()` for LSP/SARIF
- [ ] Tests: trace renders correctly as explanation chain
- [ ] Tests: cross-file traces include file paths

---

## 07.3 Integration Points

### Where Traces Are Useful

| Error Type | Trace Kind | Value |
|-----------|-----------|-------|
| `ImportNotFound` | Import chain | Shows re-export path |
| `ImportedItemNotFound` | Import chain | Shows where item was expected |
| `CircularDependency` | Import chain | Shows full cycle path |
| `MissingCapability` | Call chain | Shows which caller requires the capability |
| `TypeMismatch` (in generics) | Type instantiation | Shows how type params were specialized |
| `MissingTest` (via deps) | Test target | Shows test dependency path |

### Integration Pattern

```rust
// In oric/src/reporting/semantic.rs

SemanticProblem::ImportNotFound { span, module_name } => {
    let mut diag = Diagnostic::error(ErrorCode::E2005)
        .with_message(format!("module `{module_name}` not found"))
        .with_label(span, "not found");

    // Collect reference trace if available
    if let Some(trace) = collect_import_trace(db, current_module, target_module) {
        diag = diag.with_explanation(trace.to_explanation_chain());
        for step in &trace.steps {
            if step.file.is_some() {
                diag = diag.with_related(
                    RelatedInformation::cross_file(
                        &step.message,
                        step.span,
                        SourceInfo { path: step.file.clone().unwrap(), content: String::new() },
                    )
                );
            }
        }
    }

    diag
}
```

### Performance Considerations

- Traces are collected **only on error** — zero cost for successful compilations
- Import chain traces are bounded by module count (typically <100)
- Call chain traces are bounded by call stack depth (configurable max, default 10)
- Type instantiation traces are bounded by generic nesting depth (typically <5)

- [ ] Integrate import chain traces with `ImportNotFound` / `ImportedItemNotFound`
- [ ] Integrate call chain traces with capability errors
- [ ] Integrate type instantiation traces with generic type errors
- [ ] Tests: end-to-end trace in error output

---

## 07.4 Completion Checklist

- [ ] `oric/src/traces.rs` module created
- [ ] `ReferenceTrace`, `TraceStep`, `TraceKind` types defined
- [ ] `collect_import_trace()` implemented
- [ ] `collect_call_trace()` implemented
- [ ] `collect_type_trace()` implemented
- [ ] `to_explanation_chain()` conversion
- [ ] `to_related_information()` conversion
- [ ] Integrated with import errors
- [ ] Integrated with capability errors
- [ ] Tests: 15+ unit tests for trace collection
- [ ] Tests: 5+ rendering tests
- [ ] Tests: 3+ end-to-end tests
- [ ] `./test-all.sh` passes

**Exit Criteria:** Import chain errors, capability propagation errors, and generic type instantiation errors include reference traces showing the path from the user's code to the error site. Traces render as indented "reached via" blocks in terminal output and as `relatedLocations` in SARIF.
