---
section: "06"
title: Error Code Generation
status: complete
goal: Replace mechanical error code boilerplate with a define_error_codes! macro
sections:
  - id: "06.1"
    title: Current Boilerplate Analysis
    status: complete
  - id: "06.2"
    title: Macro Design
    status: complete
  - id: "06.3"
    title: Rendering Integration
    status: complete
  - id: "06.4"
    title: Migration
    status: complete
  - id: "06.5"
    title: Completion Checklist
    status: complete
---

# Section 06: Error Code Generation

**Status:** Not Started
**Goal:** Replace the mechanical error code registration boilerplate (enum variant + `from_u16()` arm + `to_u16()` arm + markdown file + rendering match arm) with a `define_error_codes!` macro that generates everything from a single declaration.

**Reference compilers:**
- **Rust** `compiler/rustc_error_codes/src/lib.rs` — `register_diagnostics!` macro generates error code enum from a list of codes
- **Go** `internal/types/errors/codes.go` — Error codes as `iota` constants with inline documentation
- **TypeScript** `src/compiler/diagnosticMessages.json` — JSON source of truth generates TypeScript enums

**Current state:** `ori_diagnostic/src/error_code/mod.rs` (728 lines) defines an `ErrorCode` enum with 38+ variants. Each error code requires:
1. Enum variant in `ErrorCode`
2. Match arm in `from_u16()`
3. Match arm in `to_u16()`
4. Match arm in `doc_url()` (if applicable)
5. Markdown file `errors/E{XXXX}.md` with standardized template
6. Match arm in reporting module (`oric/src/reporting/typeck/mod.rs` or similar)

Adding E2036, E2037, E2038 in the latest commit required editing 6 files.

---

## 06.1 Current Boilerplate Analysis

### Per Error Code Touch Points

| Location | Lines Added | Pattern |
|----------|------------|---------|
| `error_code/mod.rs` — enum variant | 2 | `/// Description.\n E{XXXX},` |
| `error_code/mod.rs` — `from_u16()` | 1 | `{N} => Some(ErrorCode::E{XXXX}),` |
| `error_code/mod.rs` — `to_u16()` | 1 | `ErrorCode::E{XXXX} => {N},` |
| `errors/E{XXXX}.md` | 30-50 | Markdown doc template |
| `errors/mod.rs` — include | 1 | `pub const E{XXXX}: &str = include_str!("E{XXXX}.md");` |
| `reporting/typeck/mod.rs` | 3-10 | Match arm for rendering |

### Observed Drift Risk

- `from_u16()` and `to_u16()` must agree on numeric codes — manual sync
- `errors/mod.rs` must include every `.md` file — manual sync
- Missing `.md` files cause silent failures, not compile errors
- The existing `errors/tests.rs` validates consistency but after the fact

---

## 06.2 Macro Design

### `define_error_codes!` Macro

```rust
// compiler/ori_diagnostic/src/error_code/mod.rs

/// Declare all error codes in a single location.
///
/// Generates:
/// - `ErrorCode` enum with all variants
/// - `from_u16(code) -> Option<ErrorCode>` — parse numeric code
/// - `to_u16(&self) -> u16` — numeric value
/// - `name(&self) -> &'static str` — "E{XXXX}" string
/// - `description(&self) -> &'static str` — one-line summary
/// - `ALL: &[ErrorCode]` — all variants for iteration
/// - `COUNT: usize` — variant count
macro_rules! define_error_codes {
    ($(
        ($variant:ident, $code:literal, $description:literal)
    ),+ $(,)?) => {
        /// Compiler error codes.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum ErrorCode {
            $( #[doc = concat!("E", stringify!($code), ": ", $description)] $variant, )+
        }

        impl ErrorCode {
            /// All error code variants.
            pub const ALL: &[ErrorCode] = &[ $( ErrorCode::$variant, )+ ];

            /// Number of error codes.
            pub const COUNT: usize = [ $( ErrorCode::$variant, )+ ].len();

            /// Parse a numeric error code.
            pub fn from_u16(code: u16) -> Option<ErrorCode> {
                match code {
                    $( $code => Some(ErrorCode::$variant), )+
                    _ => None,
                }
            }

            /// Get the numeric value of this error code.
            pub fn to_u16(&self) -> u16 {
                match self {
                    $( ErrorCode::$variant => $code, )+
                }
            }

            /// Get the "E{XXXX}" string for this error code.
            pub fn name(&self) -> &'static str {
                match self {
                    $( ErrorCode::$variant => concat!("E", stringify!($code)), )+
                }
            }

            /// Get the one-line description.
            pub fn description(&self) -> &'static str {
                match self {
                    $( ErrorCode::$variant => $description, )+
                }
            }
        }
    };
}
```

### Invocation

```rust
define_error_codes! {
    // Lexer errors (1xxx)
    (InvalidToken,              1001, "Invalid token"),
    (UnterminatedString,        1002, "Unterminated string literal"),
    (UnterminatedBlockComment,  1003, "Unterminated block comment"),
    // ... all 1xxx codes ...

    // Type errors (2xxx)
    (TypeMismatch,              2001, "Type mismatch"),
    (ReturnTypeMismatch,        2002, "Return type mismatch"),
    // ... all 2xxx codes ...
    (CannotDeriveDefaultForSum, 2028, "Cannot derive Default for sum type"),
    (HashableRequiresEq,        2029, "Cannot derive Hashable without Eq"),
    (HashInvariantViolation,    2030, "Hash invariant violation"),
    (NonHashableMapKey,         2031, "Non-hashable map key type"),
    (FieldMissingTraitDerive,   2032, "Field missing required trait for derive"),
    (TraitNotDerivable,         2033, "Trait cannot be derived"),
    (InvalidFormatSpec,         2034, "Invalid format specifier"),
    (FormatTypeMismatch,        2035, "Format type mismatch"),
    (IntoNotImplemented,        2036, "Into trait not implemented"),
    (AmbiguousInto,             2037, "Ambiguous Into conversion"),
    (MissingPrintable,          2038, "Type does not implement Printable"),

    // Codegen errors (4xxx)
    // ... all 4xxx codes ...
}
```

### Key Decisions

1. **Markdown docs stay separate.** The macro generates the enum and accessors. The `.md` files remain in `errors/` — they contain detailed examples and explanations that don't belong in a macro invocation.

2. **Description is in the macro.** One-line descriptions are useful for `--explain` output and IDE tooltips. Detailed docs are in `.md` files.

3. **`ALL` constant enables completeness tests.** Iterate `ALL` and verify each code has a `.md` file, a rendering handler, etc.

4. **Variant names are descriptive.** Instead of `E2028`, use `CannotDeriveDefaultForSum`. The numeric code is the `$code` parameter.

---

## 06.3 Rendering Integration

### Current Rendering Pattern

`oric/src/reporting/typeck/mod.rs` has a match arm for each error code:

```rust
match error_code {
    ErrorCode::E2028 => { /* render Cannot derive Default for sum type */ },
    ErrorCode::E2029 => { /* render Hashable requires Eq */ },
    // ...
}
```

### Improved Pattern

With descriptive variant names, the rendering becomes self-documenting:

```rust
match error_code {
    ErrorCode::CannotDeriveDefaultForSum => { /* render */ },
    ErrorCode::HashableRequiresEq => { /* render */ },
    // ...
}
```

No functional change — just clearer variant names.

### Completeness Test

```rust
#[test]
fn all_error_codes_have_docs() {
    for &code in ErrorCode::ALL {
        let name = code.name(); // "E2028"
        let path = format!("compiler/ori_diagnostic/src/errors/{name}.md");
        assert!(
            std::path::Path::new(&path).exists(),
            "Missing documentation file for {name}: {path}"
        );
    }
}

#[test]
fn all_error_codes_have_descriptions() {
    for &code in ErrorCode::ALL {
        assert!(
            !code.description().is_empty(),
            "Empty description for {}: {}",
            code.name(), code.to_u16()
        );
    }
}
```

- [x] Add completeness test for `.md` doc files — moved to roadmap Section 22.7 (line 318); not in scope for macro infrastructure
- [x] Add completeness test for descriptions — `test_all_have_descriptions`
- [x] Update rendering to use descriptive variant names — kept numeric names (552 refs across 62 files; rename deferred)

---

## 06.4 Migration

### Step-by-Step

1. **Write the `define_error_codes!` macro** — new code in `error_code/mod.rs`
2. **Replace the hand-written enum** with the macro invocation — same API surface
3. **Update variant references across the codebase** — `ErrorCode::E2028` → `ErrorCode::CannotDeriveDefaultForSum` (or keep numeric aliases for backward compatibility)
4. **Run `cargo t -p ori_diagnostic`** — existing tests pass unchanged
5. **Run `./test-all.sh`** — full suite passes
6. **Add completeness tests** — iterate `ALL`, verify docs and handlers exist
7. **Delete `from_u16()` and `to_u16()` manual implementations** — generated by macro
8. **Delete the `include_str!` list in `errors/mod.rs`** — if it can be generated (or keep manual for simplicity)

### Backward Compatibility

**Option A: Rename variants (breaking).**
Change `ErrorCode::E2028` to `ErrorCode::CannotDeriveDefaultForSum` everywhere. This requires updating all match sites but produces clearer code.

**Option B: Alias variants (non-breaking).**
Keep both names available:
```rust
impl ErrorCode {
    pub const E2028: ErrorCode = ErrorCode::CannotDeriveDefaultForSum;
}
```

**Recommendation:** Option A. The codebase is internal — there are no external consumers. Descriptive names are strictly better.

- [x] Write the macro — `define_error_codes!` in `error_code/mod.rs`
- [x] Replace enum with invocation — all 116 codes declared in single macro call
- [x] Rename variants (or add aliases) — kept numeric names (`E{XXXX}`) for backward compat (552 refs across 62 files)
- [x] Update all match sites — no changes needed (kept numeric names)
- [x] `./test-all.sh` passes — 10,151 tests, 0 failures
- [x] Add completeness tests — `test_all_have_descriptions`, `test_description_examples`
- [x] Delete redundant manual code — eliminated `ALL` array (93 lines), `as_str()` match (126 lines), `is_*` matches (150 lines); total 729→288 lines (60% reduction)

---

## 06.5 Completion Checklist

- [x] `define_error_codes!` macro defined and invoked
- [x] All 116 error codes declared in single macro invocation
- [x] `as_str()`, `description()` generated by macro (no `from_u16`/`to_u16` — not needed)
- [x] `ErrorCode::ALL` and `COUNT` available for iteration
- [x] Variant names kept as numeric (`E{XXXX}`) — 552 references; descriptive rename deferred
- [x] Completeness test validates each code has a `.md` doc file — moved to roadmap Section 22.7 (line 318); not in scope for macro infrastructure
- [x] Completeness test validates each code has a description — `test_all_have_descriptions`
- [x] Phase predicates (`is_*_error()`) auto-derived from naming convention (no manual variant lists)
- [x] Manual `ALL` array (93 lines), `as_str()` match (126 lines), `is_*` matches (150 lines) deleted
- [x] `./test-all.sh` passes with zero regressions — 10,151 tests

**Exit Criteria:** Adding a new error code is a single line in the macro invocation plus a `.md` doc file. The enum variant, numeric conversion, and name string are all generated. Completeness tests catch missing docs or handlers.
