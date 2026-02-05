# V2 Cross-System Conventions

> **Single source of truth** for shared design patterns across lexer V2, parser V2, and types V2.
>
> This is NOT a shared abstraction — it's a shared design language.
> Each system remains independent in implementation and crate dependencies.

---

## §1 Index Types

All inter-phase identifiers are `u32` newtypes. This provides type safety, cheap copies, and Salsa compatibility.

**Required properties:**

| Property | Value |
|----------|-------|
| Inner type | `u32` |
| Repr | `#[repr(transparent)]` |
| Sentinel | `NONE = Self(u32::MAX)` |
| Derives | `Copy, Clone, Eq, PartialEq, Hash, Debug` |
| Defined in | `ori_ir` (cross-phase) or phase-local crate (internal) |

**Canonical pattern:**

```rust
/// Strongly-typed index into [description].
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
#[repr(transparent)]
pub struct FooIdx(u32);

impl FooIdx {
    pub const NONE: Self = Self(u32::MAX);

    #[inline]
    pub const fn from_raw(raw: u32) -> Self { Self(raw) }

    #[inline]
    pub const fn raw(self) -> u32 { self.0 }
}

const _: () = assert!(std::mem::size_of::<FooIdx>() == 4);
```

**Existing examples:**

| Type | Crate | Purpose |
|------|-------|---------|
| `ExprId(u32)` | `ori_ir` | Expression arena index |
| `StmtId(u32)` | `ori_ir` | Statement arena index |
| `Name(u32)` | `ori_ir` | Interned string handle |
| `Idx(u32)` | `ori_types` | Type pool index (types V2) |
| `TokenIdx(u32)` | `ori_ir` | Token storage index (lexer V2) |

---

## §2 Tag/Discriminant Enums

Each phase defines a compact discriminant enum for its primary data. These are `#[repr(u8)]` with semantic ranges and gaps for future variants.

**Required properties:**

| Property | Value |
|----------|-------|
| Repr | `#[repr(u8)]` |
| Derives | `Copy, Clone, Eq, PartialEq, Hash, Debug` |
| Layout | Semantic ranges with gaps (e.g., keywords 10–59, operators 60–119) |
| Method | `name() -> &'static str` for display/debugging |
| Assertion | `const _: () = assert!(size_of::<Tag>() == 1);` |

**Naming convention:**

Each crate namespaces its own tag. No collision because different crates + different semantics:

| Crate | Tag Type | Purpose |
|-------|----------|---------|
| `ori_types` | `Tag` | Type discriminant (Int, Function, Struct, …) |
| `ori_ir` | `TokenTag` | Cooked token discriminant (shared across phases) |
| `ori_lexer_core` | `RawTag` | Raw tokenizer discriminant (standalone, no `ori_*` deps) |

**Semantic range pattern** (from types V2):

```rust
#[repr(u8)]
pub enum Tag {
    // Primitives (0–15) — reserved, pre-interned
    Int = 0,
    Float = 1,
    // ...

    // Containers (16–31)
    List = 16,
    Map = 17,
    // ...

    // Functions (32–47)
    Function = 32,
    Closure = 33,
    // ...

    // Gap for future categories
}
```

Gaps between ranges allow adding variants without renumbering.

---

## §3 SoA Container API

When using Structure-of-Arrays storage, follow a consistent accessor pattern across all phases.

**Required accessors:**

```rust
impl Storage {
    fn len(&self) -> usize;
    fn tag(&self, idx: Idx) -> Tag;       // Primary discriminant
    fn flags(&self, idx: Idx) -> Flags;   // Precomputed metadata
    fn get(&self, idx: Idx) -> Item;      // Or domain-specific accessor
}
```

**Existing examples:**

| Container | Phase | Accessors |
|-----------|-------|-----------|
| `Pool` (types V2) | Type checking | `.tag(idx)`, `.flags(idx)`, `.item(idx)`, `.len()` |
| `TokenStorage` (lexer V2) | Lexing | `.tag(idx)`, `.flags(idx)`, `.value(idx)`, `.start(idx)`, `.len()` |
| `ExprArena` (parser) | Parsing | `.get(id)`, `.len()` |

**Construction:**

All containers use `with_capacity(source_len / N)` with a documented empirical `N`:

| Container | N | Rationale |
|-----------|---|-----------|
| `TokenStorage` | 6 | ~1 token per 6 source bytes (empirical, matches Zig) |
| `ExprArena` | 20 | ~1 expression per 20 source bytes (empirical) |
| `Pool` | fixed 256 | Per-module type count is bounded |

---

## §4 Flag Types

Precomputed metadata stored parallel to primary data. Uses `bitflags!` macro with semantic bit ranges.

**Required properties:**

| Property | Value |
|----------|-------|
| Macro | `bitflags::bitflags!` |
| Derives | `Copy, Clone, Eq, PartialEq, Hash, Debug` |
| Propagation | Bitwise OR where applicable |
| Width | Domain-appropriate (see below) |

**Width is domain-appropriate — the convention is the pattern, not the width:**

| Flag Type | Width | Reason |
|-----------|-------|--------|
| `TypeFlags` (types V2) | `u32` | ~20+ flags across 4 categories (presence, category, optimization, capability) |
| `TokenFlags` (lexer V2) | `u8` | ~8 flags (space_before, newline_before, adjacent, line_start, …) |

**Semantic range pattern** (from `TypeFlags`):

```rust
bitflags::bitflags! {
    pub struct TypeFlags: u32 {
        // Presence flags (bits 0–7)
        const HAS_VAR       = 1 << 0;
        const HAS_ERROR     = 1 << 3;
        // ...

        // Category flags (bits 8–15)
        const IS_PRIMITIVE  = 1 << 8;
        const IS_FUNCTION   = 1 << 10;
        // ...

        // Optimization flags (bits 16–23)
        const NEEDS_SUBST   = 1 << 16;
        const IS_MONO       = 1 << 18;
        // ...
    }
}
```

Document the bit ranges even for smaller flag types:

```rust
bitflags::bitflags! {
    pub struct TokenFlags: u8 {
        // Whitespace flags (bits 0–3)
        const SPACE_BEFORE   = 1 << 0;
        const NEWLINE_BEFORE = 1 << 1;
        const TRIVIA_BEFORE  = 1 << 2;
        const ADJACENT       = 1 << 3;

        // Position flags (bits 4–5)
        const LINE_START     = 1 << 4;
        const CONTEXTUAL_KW  = 1 << 5;

        // Status flags (bits 6–7)
        const HAS_ERROR      = 1 << 6;
        const IS_DOC         = 1 << 7;
    }
}
```

---

## §5 Error Shape

Every phase error follows the same structural pattern: WHERE + WHAT + WHY + HOW.

**Canonical shape:**

```rust
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct PhaseError {
    pub span: Span,                     // WHERE (from ori_ir)
    pub kind: PhaseErrorKind,           // WHAT (phase-specific enum)
    pub context: PhaseErrorContext,      // WHY we were checking (phase-specific)
    pub suggestions: Vec<PhaseSuggestion>, // HOW to fix (phase-specific)
}
```

**Factory methods:**

```rust
impl PhaseError {
    #[cold]
    pub fn specific_error(/* minimal params */) -> Self { /* ... */ }

    #[must_use]
    pub fn with_context(mut self, ctx: PhaseErrorContext) -> Self {
        self.context = ctx;
        self
    }

    #[must_use]
    pub fn with_suggestion(mut self, suggestion: PhaseSuggestion) -> Self {
        self.suggestions.push(suggestion);
        self
    }
}
```

`#[cold]` on factory methods tells the compiler errors are rare, keeping the happy path fast.
`#[must_use]` on fluent builders prevents accidentally discarding the modified value.

**Suggestion pattern** (from `ori_types::type_error::Suggestion`):

```rust
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct PhaseSuggestion {
    pub message: String,
    pub replacement: Option<PhaseReplacement>,
    pub priority: u8,
}
```

The phase-specific suggestion type is internal. Final rendering in `oric` converts to `ori_diagnostic::Suggestion` (with `Applicability`). This separation keeps phase crates independent of the diagnostic rendering system.

**Existing examples:**

| Phase | Error | Kind | Context | Suggestion |
|-------|-------|------|---------|------------|
| Types V2 | `TypeCheckError` | `TypeErrorKind` | `ErrorContext` | `Suggestion` |
| Lexer V2 | `LexError` | `LexErrorKind` | `LexErrorContext` | `LexSuggestion` |
| Parser V2 | `ParseError` | `ParseErrorKind` | (via expected tokens) | (via recovery) |

---

## §6 Phase Output Shape

Every phase returns a self-contained, immutable result. The next phase borrows read-only.

**Pattern:**

```rust
pub struct PhaseOutput {
    pub data: PhaseData,           // Primary output
    pub errors: Vec<PhaseError>,   // Accumulated errors
    pub metadata: PhaseMetadata,   // Non-semantic info (trivia, stats)
}
```

**Existing examples:**

| Phase | Output | Data | Errors | Metadata |
|-------|--------|------|--------|----------|
| Lexer V2 | `LexOutput` | `TokenStorage` | `Vec<LexError>` | `ModuleExtra` (from `ori_ir`) |
| Parser V2 | `ParseOutput` | `Module` + `ExprArena` | `Vec<ParseError>` | `ModuleExtra` (extended) |
| Types V2 | `TypeCheckResult` | Typed AST | (accumulated via Salsa) | `TypeGuarantee` |

**Rules:**

1. Each output is **immutable after creation** — no mutable references leak out
2. Next phase **borrows read-only** — no cloning phase outputs
3. Errors are **accumulated, not fatal** — phases continue past errors for IDE support
4. Metadata is **non-semantic** — removing it doesn't change program behavior

---

## §7 Shared Types Live in `ori_ir`

All types that cross phase boundaries are defined **once** in `ori_ir`. No phase redefines these.

**Cross-phase types:**

| Type | Purpose | Size |
|------|---------|------|
| `Span` | Source location (start: u32, end: u32) | 8 bytes |
| `Name` | Interned string handle | 4 bytes |
| `ExprId` | Expression index | 4 bytes |
| `StmtId` | Statement index | 4 bytes |
| `TokenIdx` | Token storage index (lexer V2) | 4 bytes |
| `TokenTag` | Cooked token discriminant (lexer V2) | 1 byte |
| `ModuleExtra` | Non-semantic module metadata | Variable |
| `ErrorCode` | Unique error identifier | 4 bytes |

**Rules:**

1. If two phases need the same type, it goes in `ori_ir`
2. Phase-internal types stay in their crate (e.g., `RawTag` in `ori_lexer_core`)
3. Mapping between phase-internal and shared types happens at the phase boundary
   - Example: `ori_lexer_core::RawTag` → `ori_ir::TokenTag` in `ori_lexer`

---

## §8 Salsa Compatibility

All output types and their transitive fields must be Salsa-compatible. This enables incremental compilation.

**Required derives for all query-visible types:**

```rust
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
```

**Forbidden in query results:**

| Forbidden | Reason | Alternative |
|-----------|--------|-------------|
| `Arc<Mutex<T>>` | Non-deterministic | Clone or `Arc<T>` (immutable) |
| `fn()` pointers | Not `Hash`/`Eq` | Enum dispatch |
| `dyn Trait` | Not `Hash`/`Eq` | Enum dispatch |
| `HashMap` iteration | Non-deterministic order | Sort before returning, or use `BTreeMap` |
| `f64` / `f32` | `NaN != NaN` breaks `Eq` | `u64` bits via `f64::to_bits()` |

**Determinism rule:** Sort any collection before returning from a Salsa query. Use `BTreeMap`/`BTreeSet` when order matters.

---

## §9 Capacity Estimation

All containers use `with_capacity(source_len / N)` where `N` is empirically determined.

| Container | N | Tokens/Bytes | Source |
|-----------|---|-------------|--------|
| `TokenStorage` | 6 | ~1 token per 6 bytes | Zig tokenizer measurements |
| `ExprArena` | 20 | ~1 expression per 20 bytes | Ori parser measurements |
| `Pool` | fixed 256 | Per-module ceiling | Types V2 design |
| Error vectors | 0 (empty) | Errors are rare | — |

**Rules:**

1. Always document the empirical source for `N`
2. Re-measure when token/AST structure changes significantly
3. Error vectors start empty — `Vec::new()`, not `with_capacity`
4. Use `debug_assert!(idx < self.len())` not `get().unwrap()` in accessors

---

## §10 Two-Layer Crate Pattern

For components that should be reusable outside the compiler (LSP, formatter, syntax highlighter):

**Layer 1: Core crate** — standalone, no `ori_*` dependencies

```
[dependencies]
# No ori_* crates. Only std + small utilities (e.g., unicode-ident).
```

- Pure functions, minimal types
- Defines its own lightweight tag/enum (e.g., `RawTag`)
- No spans, no interning, no diagnostics
- Stable API suitable for external tools

**Layer 2: Integration crate** — depends on core + `ori_ir`

```
[dependencies]
phase_core = { path = "../phase_core" }
ori_ir = { path = "../ori_ir" }
```

- Maps core types to `ori_ir` shared types (e.g., `RawTag` → `TokenTag`)
- Adds spans, interning, diagnostics
- Salsa integration
- Compiler-specific optimizations

**Existing example:**

| Layer | Crate | Tag Type | Dependencies |
|-------|-------|----------|--------------|
| Core | `ori_lexer_core` | `RawTag` (local) | None |
| Integration | `ori_lexer` | `TokenTag` (from `ori_ir`) | `ori_lexer_core`, `ori_ir` |

**The boundary is the mapping.** Core produces raw data; integration "cooks" it into compiler-ready form. This is the Rust compiler's `rustc_lexer` → `rustc_parse::lexer` pattern.

---

## Quick Reference

| Convention | Section | Key Rule |
|------------|---------|----------|
| Index types | §1 | `u32` newtype, `#[repr(transparent)]`, `NONE = u32::MAX` |
| Tag enums | §2 | `#[repr(u8)]`, semantic ranges with gaps, `name()` method |
| SoA accessors | §3 | `.tag()`, `.flags()`, `.get()`, `.len()` |
| Flag types | §4 | `bitflags!`, semantic bit ranges, domain-appropriate width |
| Error shape | §5 | WHERE (span) + WHAT (kind) + WHY (context) + HOW (suggestions) |
| Phase output | §6 | Immutable after creation, next phase borrows read-only |
| Shared types | §7 | Cross-phase types in `ori_ir`, phase-internal stays local |
| Salsa compat | §8 | `Clone, Eq, PartialEq, Hash, Debug` on all query-visible types |
| Capacity | §9 | `with_capacity(source_len / N)`, document N empirically |
| Two-layer | §10 | Core (standalone) → Integration (compiler-specific) |

---

## Plans That Follow These Conventions

| Plan | Location | Status |
|------|----------|--------|
| Lexer V2 | `plans/lexer_v2/` | Adopting (§1–§10) |
| Parser V2 | `plans/parser_v2/` | Established (§1, §3, §6, §9) |
| Types V2 | `plans/types_v2/` | Established (§1–§9) |
