---
title: "Problem Types"
description: "Ori Compiler Design — Phase-Specific Error Types"
order: 1301
section: "Diagnostics"
---

# Problem Types

Every compiler that produces user-facing diagnostics faces a foundational design question: should errors be represented as a single unified type, or as separate types per phase? The answer shapes how context is preserved, how phases are decoupled, and how much of the error infrastructure is reusable. This chapter explains why Ori chose phase-specific types, documents each phase's concrete error type, and shows exactly how each converts to a displayable `Diagnostic`.

## Why Phase-Specific Error Types?

Three classical approaches exist, each with distinct tradeoffs.

**Unified error enum.** One giant `enum CompilerError` with variants for every phase. Routing is simple — one match arm anywhere handles everything. The costs are severe: the enum grows unboundedly, every phase must import every other phase's context types, and the type-level phase information is lost. A function returning `CompilerError` could be returning a lexer error, a type error, or a codegen error, and callers cannot know without inspecting the variant.

**Phase-specific types with a shared trait.** Each phase defines its own error type, all implementing a shared `IntoDiagnostic` or `Diagnosable` trait. This enforces a contract across all phases and enables trait-object dispatch. The downside: trait objects require heap allocation, and the trait itself must be stabilized before phases are written — creating coupling in a different direction.

**Phase-specific types with ad hoc conversion.** Each phase defines its own error type and its own conversion function. There is no shared trait, no common supertype, no protocol enforced by the type system. Phases are maximally independent. Ori uses this approach.

The rationale is straightforward: each phase has genuinely different context requirements. The lexer has spans and source bytes but no pool and no string interner. The type checker needs both a `Pool` (to format complex types like `[int]` or `(str, bool) -> float`) and a `StringInterner` (to resolve interned `Name` values to display strings). The evaluator needs a backtrace with call frames. The codegen pipeline is terminal — post-Salsa — and does not need `Eq + Hash`. Forcing all of these into a shared trait would either impoverish the context available to each converter or require every converter to accept a superset of what it needs. Neither is acceptable.

The ad hoc approach means each phase owns its conversion completely. The price is that there is no compile-time guarantee that every problem type has a converter; this is enforced by convention and code review rather than by types.

## The Error Flow by Phase

| Phase | Error Type | Crate | Error Range | Conversion |
|---|---|---|---|---|
| Lexer (errors) | `LexError` / `LexErrorKind` | `ori_lexer` | E0xxx | `render_lex_error()` in `oric` |
| Lexer (warnings) | `LexProblem` | `oric` | W1xxx | `LexProblem::into_diagnostic()` |
| Parser | `ParseError` | `ori_parse` | E1xxx | `ParseError::to_queued_diagnostic()` |
| Type Checker | `TypeCheckError` | `ori_types` | E2xxx | `TypeErrorRenderer::render()` in `oric` |
| Type Checker (warnings) | `TypeCheckWarning` | `ori_types` | W2xxx | direct `Diagnostic` construction |
| Semantic Analysis | `SemanticProblem` | `oric` | E3xxx | `SemanticProblem::into_diagnostic(&interner)` |
| Pattern Canon | `PatternProblem` | `ori_ir` / `ori_canon` | E3xxx | `pattern_problem_to_diagnostic()` in `oric` |
| ARC Analysis | `ArcProblem` | `ori_arc` | E4xxx | `From<ArcProblem> for CodegenProblem` |
| LLVM Codegen | `CodegenProblem` | `oric` | E4xxx–E5xxx | `CodegenProblem::into_diagnostic(self)` |
| Evaluator | `EvalError` / `EvalErrorKind` | `ori_patterns` | E6xxx | `EvalError::to_diagnostic()` |
| Evaluator (Salsa) | `EvalErrorSnapshot` | `oric` | E6xxx | `snapshot_to_diagnostic()` |

Parse errors bypass the `oric` problem module entirely — `ParseError::to_queued_diagnostic()` produces a `(Diagnostic, DiagnosticSeverity)` pair that goes directly into the diagnostic queue. Type errors require Pool-aware rendering and are handled by a dedicated renderer in `oric/src/reporting/typeck/`. All other types follow the `into_diagnostic()` pattern.

## Lexer Errors

The lexer produces two distinct categories of output: hard errors that block tokenization (`LexError`) and a single warning class (`LexProblem::DetachedDocComment`).

**`LexError`** is defined in `compiler/ori_lexer/src/lex_error/mod.rs`. Its structure follows the WHERE+WHAT+WHY+HOW shape:

```rust
pub struct LexError {
    pub span: Span,          // WHERE
    pub kind: LexErrorKind,  // WHAT
    pub context: LexErrorContext,  // WHY
    pub suggestions: Vec<LexSuggestion>,  // HOW
}
```

`LexErrorKind` covers four groups of errors:

- **String and character errors**: `UnterminatedString`, `UnterminatedChar`, `UnterminatedTemplate`, `InvalidStringEscape { escape_char }`, `InvalidCharEscape`, `InvalidTemplateEscape`, `SingleQuoteEscapeInString`, `DoubleQuoteEscapeInChar`, `EmptyCharLiteral`, `MultiCharLiteral`
- **Numeric errors**: `IntOverflow`, `HexIntOverflow`, `BinIntOverflow`, `FloatParseError`, `DecimalNotRepresentable`
- **Character and encoding errors**: `InvalidByte { byte }`, `StandaloneBackslash`, `UnicodeConfusable { found, suggested, name }`, `InvalidNullByte`, `Utf8Bom`, `Utf16LeBom`, `Utf16BeBom`, `InvalidControlChar { byte }`, `ReservedFutureKeyword { keyword }`
- **Cross-language habit errors**: `TripleEqual`, `SingleQuoteString`, `IncrementDecrement { op }`, `TernaryOperator`

The cross-language errors are particularly instructive. When the lexer encounters `===`, `++`, `--`, `? :`, or `'string'`, it produces a targeted error rather than a generic "unexpected character." These patterns are common habits from JavaScript, Python, and C that produce confusing failures in Ori without explicit guidance.

`LexSuggestion` carries an optional `LexReplacement { span, text }`. When a replacement is available — such as replacing `===` with `==`, or removing a UTF-8 BOM — the suggestion is machine-applicable. The conversion function `render_lex_error()` in `oric/src/problem/lex.rs` maps these to `Suggestion::machine_applicable()` on the resulting `Diagnostic`.

`LexProblem` is a thin wrapper in `oric` for the detached-doc-comment warning:

```rust
pub enum LexProblem {
    DetachedDocComment { span: Span, marker: DocMarker },
}

impl LexProblem {
    pub fn into_diagnostic(&self) -> Diagnostic {
        match self {
            LexProblem::DetachedDocComment { span, .. } =>
                Diagnostic::warning(ErrorCode::E0012)
                    .with_message("detached doc comment")
                    .with_label(*span, "this doc comment is not attached to any declaration")
                    .with_suggestion("doc comments should appear immediately before a function..."),
        }
    }
}
```

`LexError` values derive `Clone, Eq, PartialEq, Hash, Debug` for Salsa compatibility. `LexProblem` derives the same set for the same reason.

## Parse Errors

The parser bypasses the `oric` problem module. `ParseError` is self-contained in `ori_parse` and converts directly to `Diagnostic` without passing through any external type.

```rust
pub struct ParseError {
    pub(crate) code: ErrorCode,
    pub(crate) message: String,
    pub(crate) span: Span,
    pub(crate) context: Option<String>,
    pub(crate) help: Vec<String>,
    pub(crate) severity: DiagnosticSeverity,
}
```

The `severity` field distinguishes **hard** from **soft** errors. Soft errors arise from `ParseOutcome::EmptyErr` — the parser attempted a rule and consumed no tokens, so the failure is speculative. Hard errors arise from committed parsing that found something unexpected. The `DiagnosticQueue` suppresses soft errors after a hard error to prevent noise flooding when a single structural mistake produces dozens of speculative failures.

Three primary construction paths exist:

- `ParseError::new(code, message, span)` — raw construction, defaults to hard severity
- `ParseError::from_kind(&ParseErrorKind, span)` — structured construction from a rich kind enum; extracts hint, educational note, and context from the kind
- `ParseError::from_error_token(span, source_text)` — inspects the literal source text via `detect_common_mistake()` to recognize cross-language patterns and provide targeted help

`ParseErrorKind` carries structured context for each error category: `UnexpectedToken { found, expected, context }`, `UnexpectedEof { expected, unclosed }`, `ExpectedExpression { found, position: ExprPosition }`, `UnclosedDelimiter { open, open_span, expected_close }`, and nine others. Each variant knows how to generate an empathetic message, a hint, and an educational note — drawing on Elm's first-person phrasing style.

The conversion to `Diagnostic` is straightforward:

```rust
pub fn to_diagnostic(&self) -> Diagnostic {
    let mut diag = Diagnostic::error(self.code)
        .with_message(&self.message)
        .with_label(self.span, self.context.as_deref().unwrap_or("here"));
    for help in &self.help {
        diag = diag.with_note(help);
    }
    diag
}

pub fn to_queued_diagnostic(&self) -> (Diagnostic, DiagnosticSeverity) {
    (self.to_diagnostic(), self.severity)
}
```

`ParseError` derives `Clone, Eq, PartialEq, Hash, Debug` for Salsa compatibility.

## Type Errors

**`TypeCheckError`** is defined in `compiler/ori_types/src/type_error/check_error/mod.rs`. It is the richest error type in the compiler:

```rust
pub struct TypeCheckError {
    pub span: Span,
    pub kind: TypeErrorKind,
    pub context: ErrorContext,
    pub suggestions: Vec<Suggestion>,
}
```

`TypeErrorKind` has over 35 variants spanning type mismatches, unknown identifiers, arity mismatches, missing capabilities, infinite types, ambiguous methods, non-exhaustive matches, derive failures, index errors, assignment-to-immutable, and more.

The difficulty is rendering: a `TypeCheckError` stores types as `Idx` handles into the `Pool`. The `Pool` is needed to format `Idx` values into human-readable strings like `[int]` or `(str, bool) -> float`. The `TypeCheckError` itself has no `Pool` access. If it did, it could not implement `Eq + Hash` (Pool borrows are not `Eq`), which breaks Salsa compatibility.

The solution is `TypeErrorRenderer<'a>` in `oric/src/reporting/typeck/`:

```rust
pub struct TypeErrorRenderer<'a> {
    pool: &'a Pool,
    interner: &'a StringInterner,
}

impl<'a> TypeErrorRenderer<'a> {
    pub fn render(&self, error: &TypeCheckError) -> Diagnostic {
        let message = error.format_message_rich(
            &|idx| self.pool.format_type_resolved(idx, self.interner),
            &|name| self.interner.lookup(name).to_string(),
        );
        let primary_label = self.primary_label_text(error);

        let mut diag = Diagnostic::error(error.code())
            .with_message(message)
            .with_label(error.span, primary_label);

        Self::add_context(&mut diag, &error.context);
        Self::add_suggestions(&mut diag, &error.suggestions);
        diag
    }
}
```

`format_message_rich()` accepts closures for type and name formatting, keeping the rendering logic in `ori_types` while allowing callers to supply the pool. `primary_label_text()` dispatches on all `TypeErrorKind` variants; for `Mismatch` variants, it first checks for a `TypeProblem`-specific label that can override the generic `"expected X, found Y"` text.

**`TypeProblem`** is a companion to `TypeErrorKind` that identifies the specific nature of a mismatch. Rather than producing a generic "type mismatch," the type checker pattern-matches on type combinations to produce variants like `IntFloat { expected, found }` (suggests `int(x)` or `float(x)` conversion), `NotCallable { actual_type }` (explains the type cannot be called), `ReturnMismatch { expected, found }` (generates a label showing both types), and `ClosureSelfCapture` (unique to closures that reference their own binding). The `TypeProblem` variants also generate `Suggestion` values attached to the `TypeCheckError`.

**`ErrorContext`** provides the WHERE and WHY that surround a type error. It tracks the `ContextKind` (what was being checked — function call argument, return expression, if-branch, etc.) and can hold an `ExpectedOrigin` (why the expected type was expected — type annotation, previous element in a sequence, etc.) and freeform notes.

`TypeCheckWarning` is simpler — currently only `InfiniteIteratorConsumed { consumer, source }` (W2001) — and is constructed directly as a `Diagnostic` at the rendering site.

## Semantic Problems

**`SemanticProblem`** in `oric/src/problem/semantic/mod.rs` is the semantic analysis problem type. It has 22 variants:

| Variant | Notes |
|---|---|
| `UnknownIdentifier { span, name, similar }` | "did you mean?" with similar name |
| `UnknownFunction { span, name, similar }` | with `@` prefix in message |
| `UnknownConfig { span, name, similar }` | with `$` prefix in message |
| `DuplicateDefinition { span, name, kind, first_span }` | secondary label at first definition |
| `PrivateAccess { span, name, kind }` | suggests adding `pub` |
| `ImportNotFound { span, path }` | path is `String`, not `Name` |
| `ImportedItemNotFound { span, item, module }` | |
| `ImmutableMutation { span, name, binding_span }` | secondary label at binding |
| `UseBeforeInit { span, name }` | |
| `MissingTest { span, func_name }` | **severity-configurable** (off/warn/error) |
| `TestTargetNotFound { span, test_name, target_name }` | |
| `BreakOutsideLoop { span }` | |
| `ContinueOutsideLoop { span }` | |
| `SelfOutsideMethod { span }` | |
| `InfiniteRecursion { span, func_name }` | warning severity |
| `UnusedVariable { span, name }` | warning; suggests `_` prefix |
| `UnusedFunction { span, name }` | warning |
| `UnreachableCode { span }` | warning |
| `NonExhaustiveMatch { span, missing_patterns }` | |
| `RedundantPattern { span, covered_by_span }` | warning |
| `MissingCapability { span, capability }` | |
| `DuplicateCapability { span, capability, first_span }` | secondary label |

Warning variants are `UnusedVariable`, `UnusedFunction`, `UnreachableCode`, and `RedundantPattern`. All others are errors.

Currently, only `MissingTest` is produced in production code — by `check_test_coverage()` during the `ori check` command. Its severity is controlled by the `test-enforcement` setting: `off` (suppressed, default), `warn` (warning), or `error` (compilation error). Most other variants are implemented and tested but are reserved for a future dedicated semantic analysis pass. Some overlap with `TypeCheckError`: `UnknownIdentifier`, `DuplicateDefinition`, and similar variants are currently handled by the type checker's own error reporting. `SemanticProblem` provides clean infrastructure for when a dedicated semantic pass is added.

The `DefinitionKind` enum (`Function`, `Variable`, `Config`, `Type`, `Test`, `Import`) provides grammatically consistent error messages for variants that reference named items.

Conversion takes a `StringInterner` to resolve `Name` fields:

```rust
impl SemanticProblem {
    pub fn into_diagnostic(&self, interner: &StringInterner) -> Diagnostic { ... }
}
```

`check_test_coverage()` walks the module's function list and test list, collecting all tested function names into a `FxHashSet`, then emits `MissingTest` for every function not in the set (excluding `@main`):

```rust
pub fn check_test_coverage(module: &Module, interner: &StringInterner) -> Vec<SemanticProblem> {
    let main_name = interner.intern("main");
    let mut tested: FxHashSet<Name> = FxHashSet::default();
    for test in &module.tests {
        for target in &test.targets { tested.insert(*target); }
    }
    module.functions.iter()
        .filter(|f| f.name != main_name && !tested.contains(&f.name))
        .map(|f| SemanticProblem::MissingTest { span: f.span, func_name: f.name })
        .collect()
}
```

## Pattern Problems

**`PatternProblem`** originates in `ori_ir::canon` from the pattern exhaustiveness and redundancy checker. It has two variants:

- `NonExhaustive { match_span: Span, missing: Vec<String> }` — a match expression does not cover all cases
- `RedundantArm { arm_span: Span, match_span: Span, arm_index: usize }` — a match arm can never be reached because earlier arms already cover it

`pattern_problem_to_diagnostic()` in `oric/src/problem/semantic/mod.rs` bridges these to `SemanticProblem` and then to `Diagnostic`:

```rust
pub fn pattern_problem_to_diagnostic(
    problem: &ori_canon::PatternProblem,
    interner: &StringInterner,
) -> Diagnostic {
    let semantic = match problem {
        ori_canon::PatternProblem::NonExhaustive { match_span, missing } =>
            SemanticProblem::NonExhaustiveMatch {
                span: *match_span,
                missing_patterns: missing.clone(),
            },
        ori_canon::PatternProblem::RedundantArm { arm_span, match_span, .. } =>
            SemanticProblem::RedundantPattern {
                span: *arm_span,
                covered_by_span: *match_span,
            },
    };
    semantic.into_diagnostic(interner)
}
```

A non-exhaustive match on a bool type produces:

```
error[E3002]: non-exhaustive match
 --> main.ori:5:1
  |
5 | match b {
  | ^^^^^ patterns not covered
  |
  = note: missing patterns: false
```

A redundant arm produces:

```
warning[E3003]: redundant pattern
  --> main.ori:8:5
   |
8  |     _ -> "other"
   |     ^ this pattern is unreachable
   |
5  |     _ -> "fallback"
   |     - already covered by this pattern
```

## Codegen Problems

**`CodegenProblem`** in `oric/src/problem/codegen/mod.rs` covers the ARC analysis and LLVM backend phases. It is deliberately separate from the Salsa-compatible problem types: codegen is terminal (post-Salsa query graph) and errors there are converted to diagnostics and reported immediately, not stored. The type therefore does not derive `Eq + PartialEq + Hash` — only `Clone + Debug`.

```rust
pub enum CodegenProblem {
    // ARC Analysis (E4xxx)
    ArcUnsupportedPattern { kind: &'static str, span: Span },
    ArcInternalError { message: String, span: Span },
    ArcFbipViolation { func_name: String, missed_count: usize, achieved_count: usize, span: Span },

    // LLVM Verification (E5001)
    VerificationFailed { message: String },
    // Optimization (E5002)
    OptimizationFailed { pipeline: String, message: String },
    // Emission (E5003)
    EmissionFailed { format: String, path: String, message: String },
    // Target (E5004)
    TargetNotSupported { triple: String, message: String },
    // Runtime (E5005)
    RuntimeNotFound { search_paths: Vec<String> },
    // Linker (E5006)
    LinkerNotFound { linker: String, message: String },
    LinkFailed { command: String, exit_code: Option<i32>, stderr: String },
    // Debug Info (E5007)
    DebugInfoFailed { message: String },
    // WASM (E5008)
    WasmError { message: String },
    // Module Config (E5009)
    ModuleConfigFailed { message: String },
}
```

Conversion is consuming — `into_diagnostic(self)` rather than `into_diagnostic(&self)` — to avoid cloning potentially large `String` fields (linker stderr, LLVM messages).

**`ArcProblem`** from `ori_arc` converts to `CodegenProblem` via `From<ArcProblem>`. Similarly, `From` implementations exist for every distinct LLVM error type: `TargetError`, `EmitError`, `OptimizationError`, `ModulePipelineError`, `LinkerError`, `DebugInfoError`, `WasmError`, `RuntimeNotFound`. This means call sites can use `?` with `into()` and errors flow to `CodegenProblem` without explicit matching.

**`CodegenDiagnostics`** is an accumulator for non-fatal codegen warnings:

```rust
pub struct CodegenDiagnostics {
    problems: Vec<CodegenProblem>,
}
```

`ArcUnsupportedPattern` is a warning (lowering proceeds without optimization for that pattern); all other `CodegenProblem` variants are errors. `CodegenDiagnostics::has_errors()` checks this distinction. `into_diagnostics()` consumes the accumulator, rendering all problems to `Vec<Diagnostic>`.

## Eval/Runtime Errors

**`EvalError`** is defined in `ori_patterns` because that crate owns the interpreter value types and must be able to produce errors without depending on `oric`. The type carries:

- `message: String` — human-readable description
- `kind: Option<EvalErrorKind>` — structured kind for error code mapping
- `span: Option<Span>` — source location, if known
- `notes: Vec<String>` — additional context
- `backtrace: Option<EvalBacktrace>` — call stack at the error site

**`EvalErrorKind`** provides the structural mapping to E6xxx error codes:

| Range | Category | Selected Variants |
|---|---|---|
| E6001–E6006 | Arithmetic | `DivisionByZero`, `ModuloByZero`, `IntegerOverflow`, `SizeWouldBeNegative` |
| E6010–E6012 | Type/Operator | `TypeMismatch`, `InvalidBinaryOp`, `BinaryTypeMismatch` |
| E6020–E6027 | Access | `UndefinedVariable`, `UndefinedFunction`, `UndefinedField`, `IndexOutOfBounds`, `KeyNotFound`, `ImmutableBinding` |
| E6030–E6032 | Function | `ArityMismatch`, `StackOverflow`, `NotCallable` |
| E6040 | Pattern | `NonExhaustiveMatch` |
| E6050–E6052 | Assertion | `AssertionFailed`, `PanicCalled`, `TestFailed` |
| E6060–E6061 | Capability | `MissingCapability`, `CapabilityProviderError` |
| E6070–E6072 | Const-eval | `ConstEvalBudgetExceeded`, `ConstEvalDepthExceeded`, `ConstEvalTimeout` |
| E6080–E6081 | Not-implemented | `NotImplemented`, `UnsupportedOperation` |
| E6099 | Custom | `Custom` |

`EvalError::to_diagnostic()` in `ori_patterns/src/errors/diagnostics/mod.rs` builds the `Diagnostic` directly, attaching the primary span label, notes, and backtrace summary.

At the Salsa boundary, `EvalError` is serialized to `EvalErrorSnapshot` (a Salsa-compatible `Clone + Eq + Hash` form) before being stored in query results. `snapshot_to_diagnostic()` in `oric/src/problem/eval/mod.rs` converts the snapshot to `Diagnostic` with enriched location information: a `LineOffsetTable` is built from the source text to convert byte offsets to `file:line:col` strings in the backtrace:

```rust
pub fn snapshot_to_diagnostic(
    snapshot: &EvalErrorSnapshot,
    source: &str,
    file_path: &str,
) -> Diagnostic {
    let mut diag = Diagnostic::error(snapshot.error_code).with_message(&snapshot.message);
    let table = LineOffsetTable::build(source);
    if let Some(span) = snapshot.span {
        let (line, col) = table.offset_to_line_col(source, span.start);
        diag = diag.with_label(span, format!("runtime error at {file_path}:{line}:{col}"));
    }
    // backtrace frames enriched with file:line:col
    ...
    diag
}
```

## The into_diagnostic() Pattern

The common thread across all phases is the `into_diagnostic()` method — either on `&self` (when the error is stored for inspection) or on `self` (when the error is consumed for rendering). Each type implements this independently, which is the defining characteristic of the ad hoc conversion approach.

The reasons each phase's converter is different:

- **Lexer** (`render_lex_error`): no pool, no interner — only span and error kind
- **Parser** (`to_queued_diagnostic`): no pool — produces `(Diagnostic, DiagnosticSeverity)` pair for queue routing
- **Type checker** (`TypeErrorRenderer::render`): requires `&Pool` and `&StringInterner` — the renderer is a separate struct, not a method on `TypeCheckError`
- **Semantic** (`into_diagnostic(&interner)`): requires `&StringInterner` to resolve `Name` fields
- **Codegen** (`into_diagnostic(self)`): consuming, because owned `String` fields would otherwise require cloning
- **Eval** (`to_diagnostic()` / `snapshot_to_diagnostic()`): optional `LineOffsetTable` for file:line:col enrichment

The **builder pattern** on `Diagnostic` is consistent across all phases. Every converter uses:

```rust
Diagnostic::error(ErrorCode::Exxx)
    .with_message("...")
    .with_label(span, "...")
    .with_note("...")
    .with_suggestion("...")
```

Factory functions on error types (`LexError::unterminated_string(span)`, `TypeCheckError::mismatch(...)`, etc.) are marked `#[cold]` because they only execute on the error path. This prevents error-handling code from polluting the instruction cache of the hot compilation path.

## Prior Art

**[rustc](https://github.com/rust-lang/rust)** uses session-based error emission with `Diagnostic` derive macros and a `LintDiagnostic` trait. Errors are emitted eagerly through a `Handler` rather than accumulated and converted at boundaries. The `DiagnosticMessage` type uses fluent localization. Ori's approach is simpler: no localization infrastructure, no session handle threading.

**[Elm](https://elm-lang.org/)** separates errors into phase-specific modules: `Error.Type`, `Error.Syntax`, `Error.Canonicalize`. Each module owns its rendering, using `Doc` combinators for structured text layout. Ori's `ParseErrorKind::empathetic_message()` draws directly on Elm's first-person phrasing style.

**[Roc](https://roc-lang.org/)** follows the same phase-separation pattern in `crates/reporting/src/error/`: `type.rs`, `canonicalize.rs`, `parse.rs` are independent modules with independent rendering. Roc additionally separates the *report* (what went wrong) from the *rendering* (how to display it). Ori collapses these into a single `into_diagnostic()` call for simplicity.

**[GHC](https://www.haskell.org/ghc/)** implements a `GhcMessage` → `PsMessage` → `TcRnMessage` hierarchy where each phase has its own message type that ultimately renders through `SDoc`. The constraint solver errors in `TcRnMessage` are particularly elaborate. Ori's `TypeErrorKind` with 35+ variants is inspired by the same insight that type errors need fine-grained structural classification to generate useful messages.

## Design Tradeoffs

**Ad hoc conversion vs shared trait.** A shared trait like `IntoDiagnostic` would provide compile-time enforcement that every problem type has a converter. Ori does not use this because: (1) the context requirements differ per phase — a trait would need to accept the union of all contexts; (2) trait objects add heap allocation; (3) the phase-local independence is a feature, not a limitation.

**Phase-specific types vs unified enum.** Ori's types are genuinely heterogeneous. `LexError` derives `Eq + Hash` for Salsa; `CodegenProblem` does not. `ParseError` carries severity for queue routing; `SemanticProblem` does not. A unified enum would paper over these structural differences, not resolve them.

**Consuming vs borrowing `into_diagnostic`.** `CodegenProblem::into_diagnostic(self)` is consuming because it contains owned `String` fields (linker stderr, LLVM messages) that would otherwise require cloning. Most other types use `into_diagnostic(&self)` because they store `Name` (interned, cheap) and `Span` (copy). The asymmetry is intentional and reflects the different allocation patterns of each phase.

**`SemanticProblem` overlap with `TypeCheckError`.** Several `SemanticProblem` variants (`UnknownIdentifier`, `DuplicateDefinition`) duplicate concepts that `TypeCheckError` already handles through `TypeErrorKind::UnknownIdent` and `TypeErrorKind::DuplicateImpl`. This overlap exists because `SemanticProblem` is infrastructure for a future dedicated semantic analysis pass that will run before the type checker. When that pass lands, these variants will take over from the type checker for name-resolution-class errors, and the type checker will focus purely on type-level invariants.

## Related Documents

- [Index](./index.md) — diagnostics subsystem overview
- [Emitters](./emitters.md) — rendering diagnostics to terminal, JSON, and SARIF
- [Code Fixes](./code-fixes.md) — machine-applicable suggestions and `Applicability`
