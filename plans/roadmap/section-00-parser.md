---
section: 0
title: Full Parser Support
status: in-progress
tier: 0
goal: Complete parser support for entire Ori spec grammar (parsing only, not evaluation)
spec:
  - spec/grammar.ebnf
  - spec/02-source-code.md
  - spec/03-lexical-elements.md
  - spec/06-types.md
  - spec/08-declarations.md
  - spec/09-expressions.md
  - spec/10-patterns.md
sections:
  - id: "0.1"
    title: Lexical Grammar
    status: complete
  - id: "0.2"
    title: Source Structure
    status: complete
  - id: "0.3"
    title: Declarations
    status: complete
  - id: "0.4"
    title: Types
    status: in-progress
  - id: "0.4.5"
    title: Trait Objects
    status: complete
  - id: "0.5"
    title: Expressions
    status: complete
  - id: "0.6"
    title: Patterns
    status: in-progress
  - id: "0.7"
    title: Constant Expressions
    status: complete
  - id: "0.8"
    title: Section Completion Checklist
    status: in-progress
  - id: "0.9"
    title: Parser Bugs (from Comprehensive Tests)
    status: in-progress
  - id: "0.10"
    title: "Block Expression Syntax (PRIORITY)"
    status: not-started
---

# Section 0: Full Parser Support

**Goal**: Complete parser support for entire Ori spec grammar (parsing only — evaluator in Section 23)

> **SPEC**: `spec/grammar.ebnf` (authoritative), `spec/02-source-code.md`, `spec/03-lexical-elements.md`

**Status**: In Progress — Re-verified 2026-02-14. ~3 parser bugs remain (down from ~24). 23 items previously broken now parse correctly. Remaining gaps: const functions, `.match()` method syntax. See § 0.8 for full bug list.

---

## OVERVIEW

This section ensures the parser handles every syntactic construct in the Ori specification. It has **no dependencies** and can be worked on at any time. Other sections may overlap with this work — that's expected and acceptable.

**Why this matters**: The formatter, LSP, and other tooling depend on complete parser support. Without it, valid Ori code may fail to parse, blocking downstream work.

**Approach**:
1. Audit current parser against each grammar production
2. Implement missing productions
3. Add parser tests for each production
4. Mark items complete as verified

---

## 0.1 Lexical Grammar

> **SPEC**: `grammar.ebnf` § LEXICAL GRAMMAR, `spec/03-lexical-elements.md`

### 0.1.1 Comments

- [x] **Audit**: Line comments `// ...` — grammar.ebnf § comment [done] (2026-02-10)
  - [x] **Rust Tests**: `oric/tests/phases/parse/lexer.rs` — 14 comment tests (classification, spans, detached warnings)
  - [x] **Ori Tests**: `tests/spec/lexical/comments.ori` — 30+ tests

- [x] **Audit**: Doc comments with markers — grammar.ebnf § doc_comment [done] (2026-02-10)
  - [x] `// ` (description), `// *` (param), `// !` (warning), `// >` (example)
  - [x] **Rust Tests**: `oric/tests/phases/parse/lexer.rs` — doc comment type tests
  - [x] **Ori Tests**: `tests/spec/lexical/comments.ori` — all marker types tested

### 0.1.2 Identifiers

- [x] **Audit**: Standard identifiers `letter { letter | digit | "_" }` — grammar.ebnf § identifier [done] (2026-02-10)
  - [x] **Rust Tests**: `oric/tests/phases/parse/lexer.rs` — identifier tokenization
  - [x] **Ori Tests**: `tests/spec/lexical/identifiers.ori` — 40+ tests (letters, digits, underscores, case sensitivity)

### 0.1.3 Keywords

- [x] **Audit**: Reserved keywords — grammar.ebnf § Keywords [done] (2026-02-10)
  - [x] `break`, `continue`, `def`, `do`, `else`, `extern`, `false`, `for`, `if`, `impl`
  - [x] `in`, `let`, `loop`, `match`, `pub`, `self`, `Self`, `then`, `trait`, `true`
  - [x] `type`, `unsafe`, `use`, `uses`, `void`, `where`, `with`, `yield`
  - [x] **Rust Tests**: `oric/tests/phases/parse/lexer.rs` — 45+ keyword recognition tests
  - [x] **Ori Tests**: `tests/spec/lexical/keywords.ori` — 50+ tests

- [x] **Audit**: Context-sensitive keywords (patterns) — grammar.ebnf § Keywords [done] (2026-02-10)
  - [x] `cache`, `catch`, `for`, `match`, `nursery`, `parallel`, `recurse`, `run`, `spawn`, `timeout`, `try`, `with`
  - [x] **Rust Tests**: `oric/tests/phases/parse/lexer.rs` — soft keyword lookahead, contextual flag tests
  - [x] **Ori Tests**: `tests/spec/lexical/keywords.ori` — context-sensitive usage verified

- [x] **Audit**: Context-sensitive keywords (stdlib methods) — spec/03-lexical-elements.md § Context-Sensitive [done] (2026-02-10)
  - [x] `collect`, `filter`, `find`, `fold`, `map`, `retry`, `validate`
  - [x] **Note**: These are stdlib iterator methods, not compiler patterns — listed in spec as context-sensitive
  - [x] **Rust Tests**: `oric/tests/phases/parse/lexer.rs` — builtin_names_are_identifiers test
  - [x] **Ori Tests**: `tests/spec/lexical/keywords.ori` — `fold` tested as identifier; stdlib methods are plain identifiers

- [x] **Audit**: Context-sensitive keywords (other) — grammar.ebnf § Keywords [done] (2026-02-10)
  - [x] `without` (imports only), `by` (ranges only), `max` (fixed-capacity only)
  - [x] `int`, `float`, `str`, `byte` (type conversion call position)
  - [x] **Rust Tests**: `oric/tests/phases/parse/lexer.rs` — type keywords always resolved, soft keyword handling
  - [x] **Ori Tests**: `tests/spec/lexical/keywords.ori` — type keywords and context-sensitive usage tested

### 0.1.4 Operators

- [x] **Audit**: Arithmetic operators — grammar.ebnf § arith_op [done] (2026-02-10)
  - [x] `+`, `-`, `*`, `/`, `%`, `div` — **Fixed**: Added `div` to parser (was missing)
  - [x] **Rust Tests**: implicit through tokenization
  - [x] **Ori Tests**: `tests/spec/lexical/operators.ori` — 80+ tests including precedence

- [x] **Audit**: Comparison operators — grammar.ebnf § comp_op [done] (2026-02-10)
  - [x] `==`, `!=`, `<`, `>`, `<=`, `>=`
  - [x] **Rust Tests**: implicit through tokenization
  - [x] **Ori Tests**: `tests/spec/lexical/operators.ori`

- [x] **Audit**: Logic operators — grammar.ebnf § logic_op [done] (2026-02-10)
  - [x] `&&`, `||`, `!`
  - [x] **Rust Tests**: implicit through tokenization
  - [x] **Ori Tests**: `tests/spec/lexical/operators.ori`

- [x] **Audit**: Bitwise operators — grammar.ebnf § bit_op [done] (2026-02-10)
  - [x] `&`, `|`, `^`, `~`, `<<`, `>>`
  - [x] **Rust Tests**: implicit through tokenization
  - [x] **Ori Tests**: `tests/spec/lexical/operators.ori`

- [x] **Audit**: Other operators — grammar.ebnf § other_op [done] (2026-02-10)
  - [x] `..`, `..=`, `??`, `?`, `->`, `=>` — Note: `??` parses but evaluator incomplete
  - [x] **Rust Tests**: implicit through tokenization
  - [x] **Ori Tests**: `tests/spec/lexical/operators.ori`

### 0.1.5 Delimiters

- [x] **Audit**: All delimiters — grammar.ebnf § delimiter [done] (2026-02-10)
  - [x] `(`, `)`, `[`, `]`, `{`, `}`, `,`, `:`, `.`, `@`, `$`
  - [x] **Rust Tests**: implicit through parsing
  - [x] **Ori Tests**: `tests/spec/lexical/delimiters.ori` — 70+ tests (all delimiter types in context)

### 0.1.6 Integer Literals

- [x] **Audit**: Decimal integers — grammar.ebnf § decimal_lit [done] (2026-02-10)
  - [x] Basic: `42`, with underscores: `1_000_000`
  - [x] **Rust Tests**: `oric/tests/phases/parse/lexer.rs` — decimal/hex/binary underscore tests
  - [x] **Ori Tests**: `tests/spec/lexical/int_literals.ori` — 50+ tests (decimal, hex, binary, underscores, negative, boundary)

- [x] **Audit**: Hexadecimal integers — grammar.ebnf § hex_lit [done] (2026-02-10)
  - [x] Basic: `0xFF`, with underscores: `0x1A_2B_3C`
  - [x] **Rust Tests**: `oric/tests/phases/parse/lexer.rs` — hex underscore tests
  - [x] **Ori Tests**: `tests/spec/lexical/int_literals.ori` — hex literals with case/underscore variants

### 0.1.7 Float Literals

- [x] **Audit**: Basic floats — grammar.ebnf § float_literal [done] (2026-02-10)
  - [x] Simple: `3.14`, with exponent: `2.5e-8`, `1.0E+10`
  - [x] **Rust Tests**: `oric/tests/phases/parse/lexer.rs` — float/scientific notation tests
  - [x] **Ori Tests**: `tests/spec/lexical/float_literals.ori` — 50+ tests (basic, exponents, underscores, precision)

### 0.1.8 String Literals

- [x] **Audit**: Basic strings — grammar.ebnf § string_literal [done] (2026-02-10)
  - [x] Simple: `"hello"`, empty: `""`
  - [x] **Rust Tests**: `oric/tests/phases/parse/lexer.rs` — string escape tests
  - [x] **Ori Tests**: `tests/spec/lexical/string_literals.ori` — 60+ tests

- [x] **Audit**: Escape sequences — grammar.ebnf § escape [done] (2026-02-10)
  - [x] `\\`, `\"`, `\n`, `\t`, `\r`, `\0`
  - [x] **Rust Tests**: `oric/tests/phases/parse/lexer.rs` — escape parsing
  - [x] **Ori Tests**: `tests/spec/lexical/string_literals.ori` — escapes tested in same file

### 0.1.9 Template Literals

- [x] **Audit**: Template strings — grammar.ebnf § template_literal [done] (2026-02-10)
  - [x] Simple: `` `hello` ``, interpolation: `` `{name}` ``
  - [x] **Rust Tests**: `oric/tests/phases/parse/lexer.rs` — 12 template tests (head/middle/tail tokens, interpolation)
  - [x] **Ori Tests**: `tests/spec/expressions/template_literals.ori` — end-to-end template tests

- [x] **Audit**: Template escapes — grammar.ebnf § template_escape, template_brace [done] (2026-02-10)
  - [x] Escapes: `` \` ``, `\\`, `\n`, `\t`, `\r`, `\0`
  - [x] Brace escapes: `{{`, `}}`
  - [x] **Rust Tests**: `oric/tests/phases/parse/lexer.rs` — template escape/brace tests

- [x] **Audit**: Format specifications — grammar.ebnf § format_spec [done] (2026-02-10)
  - [x] Fill/align: `{x:<10}`, `{x:>5}`, `{x:^8}`
  - [x] Sign: `{x:+}` (always sign), `{x:-}` (negative only), `{x: }` (space for positive)
  - [x] Width/precision: `{x:10}`, `{x:.2}`, `{x:10.2}`
  - [x] Format types: `{x:b}`, `{x:x}`, `{x:X}`, `{x:o}`, `{x:e}`, `{x:E}`, `{x:f}`, `{x:%}`
  - [x] **Rust Tests**: `oric/tests/phases/parse/lexer.rs` — format spec parsing tests (`:x`, `>10.2f`)

### 0.1.10 Character Literals

- [x] **Audit**: Character literals — grammar.ebnf § char_literal [done] (2026-02-10)
  - [x] Simple: `'a'`, escapes: `'\n'`, `'\t'`, `'\''`, `'\\'`
  - [x] **Rust Tests**: `oric/tests/phases/parse/lexer.rs` — char literal parsing
  - [x] **Ori Tests**: `tests/spec/lexical/char_literals.ori` — 60+ tests (letters, digits, punctuation, escapes)

### 0.1.11 Boolean Literals

- [x] **Audit**: Boolean literals — grammar.ebnf § bool_literal [done] (2026-02-10)
  - [x] `true`, `false`
  - [x] **Rust Tests**: implicit through keyword recognition
  - [x] **Ori Tests**: `tests/spec/lexical/bool_literals.ori` — 60+ tests (truth tables, De Morgan's, short-circuit)

### 0.1.12 Duration Literals

- [x] **Audit**: Duration literals — grammar.ebnf § duration_literal [done] (2026-02-10)
  - [x] All units: `100ns`, `50us`, `10ms`, `5s`, `2m`, `1h`
  - [x] Decimal syntax: `0.5s`, `1.5m` (compile-time sugar) — tested
  - [x] **Rust Tests**: `oric/tests/phases/parse/lexer.rs` — 10+ duration tests (units, decimal, many digits)
  - [x] **Ori Tests**: `tests/spec/lexical/duration_literals.ori` — 70+ tests (all units, decimal, cross-unit equivalences)

### 0.1.13 Size Literals

- [x] **Audit**: Size literals — grammar.ebnf § size_literal [done] (2026-02-10)
  - [x] All units: `100b`, `10kb`, `5mb`, `1gb`, `500tb`
  - [x] Decimal syntax: `1.5kb`, `2.5mb` (compile-time sugar) — tested
  - [x] **Rust Tests**: `oric/tests/phases/parse/lexer.rs` — 5+ size tests (units, decimal)
  - [x] **Ori Tests**: `tests/spec/lexical/size_literals.ori` — 70+ tests (all units, decimal, SI units verified: 1kb == 1000b)

---

## 0.2 Source Structure

> **SPEC**: `grammar.ebnf` § SOURCE STRUCTURE, `spec/02-source-code.md`, `spec/12-modules.md`

### 0.2.1 Source File

- [x] **Audit**: Source file structure — grammar.ebnf § source_file [done] (2026-02-10)
  - [x] `[ file_attribute ] { import } { declaration }` — imports + types + functions + tests all parse
  - [x] **Rust Tests**: implicit through full-file parsing
  - [x] **Ori Tests**: `tests/spec/source/file_structure.ori` — 6 tests

- [x] **Audit**: File-level attributes — grammar.ebnf § file_attribute [done] (2026-02-13)
  - [x] `#!target(os: "linux")`, `#!cfg(debug)` — parses correctly, stored in `Module.file_attr`
  - [x] **Rust Tests**: `oric/tests/phases/parse/file_attr.rs` — 16 tests, `ori_parse::grammar::attr` — 5 tests
  - [x] **Ori Tests**: `tests/spec/source/file_attr_target.ori`, `file_attr_cfg.ori`, `file_attributes.ori`

### 0.2.2 Imports

- [x] **Audit**: Import statements — grammar.ebnf § import [done] (2026-02-10)
  - [x] Relative: `use "./path" { items }` — parses correctly
  - [x] Module: `use std.math { sqrt }` — parses correctly
  - [x] Alias: `use std.net.http as http` — parses correctly
  - [x] **Ori Tests**: `tests/spec/source/imports.ori` — 3 tests

- [x] **Audit**: Import items — grammar.ebnf § import_item [done] (2026-02-10)
  - [x] Basic: `{ name }`, aliased: `{ name as alias }` — parses correctly
  - [x] Private: `{ ::internal }` — parses correctly (2026-02-13), constants: `{ $CONST }` — parses correctly (parser + formatter, evaluator pending)
  - [x] Without default impl: `{ Trait without def }` — parses correctly (2026-02-13, trait resolution pending)
  - [x] **Rust Tests**: `oric/tests/phases/parse/imports.rs` — 12 tests (all import_item forms)
  - [x] **Ori Tests**: `tests/spec/source/imports.ori`, `tests/spec/modules/use_imports.ori`, `tests/spec/modules/_test/use_constants.test.ori`

### 0.2.3 Re-exports

- [x] **Audit**: Re-export statements — grammar.ebnf § reexport [done] (2026-02-10)
  - [x] `pub use path { items }` — parses correctly (verified via `ori parse`)
  - [x] **Ori Tests**: `tests/spec/modules/reexporter.ori` — 1 test

### 0.2.4 Extensions

- [x] **Audit**: Extension definitions — grammar.ebnf § extension_def [done] (2026-02-10)
  - [x] `extend Type { methods }` — parses correctly (verified via `ori parse`)
  - [x] `extend Type where T: Bound { methods }` — parses correctly (2026-02-13), including multiple bounds
  - [x] **Ori Tests**: `tests/spec/source/extensions.ori` — 3 tests

- [x] **Audit**: Extension imports — grammar.ebnf § extension_import [done] (2026-02-13)
  - [x] `extension std.iter.extensions { Iterator.count }` — parses correctly (2026-02-13)
  - [x] `pub extension path { Type.method }` — parses correctly (public re-export)
  - [x] `extension "./path" { Type.method }` — parses correctly (relative paths)
  - [x] **Rust Tests**: `oric/tests/phases/parse/extensions.rs` — 8 extension import tests
  - [x] **Formatter**: round-trips correctly

### 0.2.5 FFI

- [x] **Audit**: Extern blocks — grammar.ebnf § extern_block [done] (2026-02-13)
  - [x] C: `extern "c" from "lib" { ... }` — parses correctly (verified via `ori parse`)
  - [x] JS: `extern "js" { ... }` — parses correctly
  - [x] `pub extern` visibility — parses correctly
  - [x] `from "path"` library clause — parses correctly
  - [x] Empty extern blocks — parses correctly
  - [x] **Note**: Extern blocks are now proper AST nodes (`ExternBlock`, `ExternItem`, `ExternParam`)

- [x] **Audit**: Extern items — grammar.ebnf § extern_item [done] (2026-02-13)
  - [x] `@_sin (x: float) -> float as "sin"` — parses correctly (as alias)
  - [x] `@sin (x: float) -> float` — parses correctly (no alias)
  - [x] Mixed alias/no-alias items in same block — parses correctly

- [x] **Audit**: C variadics — grammar.ebnf § c_variadic [done] (2026-02-13)
  - [x] `@printf (fmt: CPtr, ...) -> c_int` — parses correctly
  - [x] `(...) -> void` — parses correctly (no named params)

- [x] **Rust Tests**: `oric/tests/phases/parse/extern_def.rs` — 20 tests (2026-02-13)
- [x] **Formatter**: round-trips correctly (2026-02-13)

---

## 0.3 Declarations

> **SPEC**: `grammar.ebnf` § DECLARATIONS, `spec/08-declarations.md`

### 0.3.1 Attributes

- [x] **Audit**: Item attributes — grammar.ebnf § attribute [done] (2026-02-10)
  - [x] `#derive(Eq, Clone)` — parses correctly
  - [x] `#skip("reason")`, `#fail("expected")`, `#compile_fail("E1234")` — parse correctly
  - [x] `#target(os: "linux")`, `#cfg(debug)` — parses correctly [done] (2026-02-13)
  - [x] `#repr("c")` — parses correctly [done] (2026-02-13)
  - [x] **Ori Tests**: `tests/spec/declarations/attributes.ori` — 24+ tests

- [x] **Audit**: Attribute arguments — grammar.ebnf § attribute_arg [done] (2026-02-10)
  - [x] Expression: `#attr(42)`, named: `#attr(key: value)` — tested in attributes.ori
  - [x] Array: `#attr(["a", "b"])` — tested via `#derive(Eq, Clone)` syntax
  - [x] **Ori Tests**: `tests/spec/declarations/attributes.ori` — covers argument forms

### 0.3.2 Functions

- [x] **Audit**: Function declarations — grammar.ebnf § function [done] (2026-02-10)
  - [x] Basic: `@name (x: int) -> int = expr` — parses correctly
  - [x] Public: `pub @name` — parses correctly
  - [x] **Rust Tests**: `oric/tests/phases/parse/function.rs` — 8 tests for return type annotations
  - [x] **Ori Tests**: extensive coverage across all test files (3068 tests use function syntax)

- [x] **Audit**: Function generics — grammar.ebnf § generics (partial) [done] (2026-02-10)
  - [x] Type params: `@f<T> (x: T) -> T` — parses correctly (verified via `ori parse`)
  - [x] Bounded: `@f<T: Eq> (x: T) -> bool` — parses correctly
  - [x] Multiple bounds: `@f<T: Eq + Clone> (x: T) -> T` — parses correctly
  - [x] Const params: `@f<$N: int> () -> [int, max $N]` — parses correctly [done] (2026-02-13)
  - [x] Default params: `@f<T = int> (x: T) -> T` — parses correctly
  - [x] **Ori Tests**: `tests/spec/declarations/generics.ori` — exists but tests commented out (type checker deps)

- [x] **Audit**: Clause parameters — grammar.ebnf § clause_params [done] (2026-02-13)
  - [x] Pattern param: `@fib (0: int) -> int = 1` — parses correctly (verified via `ori parse`)
  - [x] Default value: `@greet (name: str = "World") -> str` — parses correctly (verified via `ori parse`)
  - [x] **Ori Tests**: `tests/spec/declarations/clause_params.ori` — exists, commented out (blocked by type checker/evaluator)

- [x] **Audit**: Guard clauses — grammar.ebnf § guard_clause [done] (2026-02-13)
  - [x] `@f (n: int) -> int if n > 0 = n` — parses correctly (verified via `ori parse`)

- [x] **Audit**: Uses clause — grammar.ebnf § uses_clause [done] (2026-02-10)
  - [x] `@fetch (url: str) -> str uses Http = ...` — parses correctly (verified via `ori parse`)
  - [x] Multiple: `@process () -> void uses Http, FileSystem = ...` — parses correctly

- [x] **Audit**: Where clause — grammar.ebnf § where_clause [done] (2026-02-10)
  - [x] Type constraint: `where T: Clone` — parses correctly (verified via `ori parse`)
  - [x] Multiple: `where T: Clone, U: Default` — parses correctly
  - [x] Const constraint: `where N > 0` — parses correctly [done] (2026-02-13)
  - [x] **Ori Tests**: `tests/spec/declarations/where_clause.ori` — exists but tests commented out

### 0.3.3 Const Bound Expressions

- [x] **Audit**: Const bound expressions — grammar.ebnf § const_bound_expr [done] (2026-02-13)
  - [x] Comparison: `N > 0`, `N == M`, `N >= 1` — parses correctly
  - [x] Logical: `N > 0 && N < 100`, `A || B`, `!C` — parses correctly
  - [x] Grouped: `(N > 0 && N < 10) || N == 100` — parses correctly
  - [x] **Rust Tests**: `ori_parse/src/grammar/item/generics.rs` — 5 where clause tests

### 0.3.4 Type Definitions

- [x] **Audit**: Struct types — grammar.ebnf § struct_body [done] (2026-02-10)
  - [x] `type Point = { x: int, y: int }` — parses correctly
  - [x] Generic: `type Box<T> = { value: T }` — parses correctly
  - [x] **Ori Tests**: `tests/spec/declarations/struct_types.ori` — 39 active tests (comprehensive)

- [x] **Audit**: Sum types — grammar.ebnf § sum_body [done] (2026-02-10)
  - [x] `type Option<T> = Some(value: T) | None` — parses correctly
  - [x] Unit variants: `type Color = Red | Green | Blue` — parses correctly
  - [x] **Ori Tests**: `tests/spec/declarations/sum_types.ori` — 35+ active tests

- [x] **Audit**: Newtype aliases — grammar.ebnf § type_body (type reference) [done] (2026-02-10)
  - [x] `type UserId = int` — parses correctly (verified via `ori parse`)
  - [x] **Ori Tests**: `tests/spec/types/newtypes.ori` — tests in types/ directory

### 0.3.5 Traits

- [x] **Audit**: Trait definitions — grammar.ebnf § trait_def [done] (2026-02-10)
  - [x] Basic: `trait Printable { @to_str (self) -> str }` — parses correctly
  - [x] With inheritance: `trait Comparable: Eq { ... }` — parses correctly
  - [x] With generics: `trait Into<T> { @into (self) -> T }` — parses correctly
  - [x] Default type params: `trait Add<Rhs = Self> { ... }` — parses correctly
  - [x] **Ori Tests**: `tests/spec/declarations/traits.ori` — 30+ active tests

- [x] **Audit**: Method signatures — grammar.ebnf § method_sig [done] (2026-02-10)
  - [x] `@method (self) -> T`, `@method (self, other: Self) -> bool` — parse correctly
  - [x] **Ori Tests**: `tests/spec/declarations/traits.ori` — method sigs tested within trait tests

- [x] **Audit**: Default methods — grammar.ebnf § default_method [done] (2026-02-10)
  - [x] `@method (self) -> T = expr` — parses correctly
  - [x] **Ori Tests**: `tests/spec/declarations/traits.ori` — default methods tested within trait tests

- [x] **Audit**: Associated types — grammar.ebnf § assoc_type [done] (2026-02-10)
  - [x] Basic: `type Item` — parses correctly
  - [x] Bounded: `type Item: Eq` — parses correctly
  - [x] Default: `type Output = Self` — parses correctly
  - [x] **Ori Tests**: `tests/spec/traits/associated_types.ori` — comprehensive tests

- [x] **Audit**: Variadic parameters — grammar.ebnf § variadic_param [done] (2026-02-13)
  - [x] `@sum (nums: ...int) -> int` — parses correctly (verified via `ori parse`)

### 0.3.6 Implementations

- [x] **Audit**: Inherent impl — grammar.ebnf § inherent_impl [done] (2026-02-10)
  - [x] `impl Point { @distance (self) -> float = ... }` — parses correctly
  - [x] Generic: `impl<T> Box<T> { ... }` — parses correctly (verified via `ori parse`)
  - [x] **Ori Tests**: tested across trait/struct test files

- [x] **Audit**: Trait impl — grammar.ebnf § trait_impl [done] (2026-02-10)
  - [x] `impl Printable for Point { ... }` — parses correctly
  - [x] Generic: `impl<T: Printable> Printable for Box<T> { ... }` — parses correctly
  - [x] **Ori Tests**: `tests/spec/declarations/traits.ori` — trait impls tested

- [x] **Audit**: Default impl — grammar.ebnf § def_impl [done] (2026-02-10)
  - [x] `def impl Printable { @to_str (self) -> str = ... }` — parses correctly (verified via `ori parse`)

### 0.3.7 Tests

- [x] **Audit**: Test declarations — grammar.ebnf § test [done] (2026-02-14)
  - [x] Attached: `@t tests @target () -> void = ...` — parses correctly (used in 3068 tests)
  - [x] Floating: `@t tests _ () -> void = ...` — parses correctly [done] (2026-02-14)
  - [x] Multi-target: `@t tests @a tests @b () -> void = ...` — parses correctly [done] (2026-02-14)
  - [x] **Ori Tests**: `tests/spec/free_floating_test.ori` — 3 tests (2 floating with `tests _`, 1 legacy `test_` prefix)
  - [x] **Rust Tests**: `ori_parse::grammar::item::function::tests` — 5 tests

### 0.3.8 Constants

- [x] **Audit**: Module-level constants — grammar.ebnf § constant_decl (partial) [done] (2026-02-10)
  - [x] `let $PI = 3.14159` — parses correctly (verified via `ori parse`)
  - [x] Computed: `let $X = 2 + 3` — parses correctly
  - [x] Typed: `let $MAX_SIZE: int = 1000` — parses correctly [done] (2026-02-14)
  - [x] **Ori Tests**: `tests/spec/declarations/constants.ori` — exists but all commented out

---

## 0.4 Types

> **SPEC**: `grammar.ebnf` § TYPES, `spec/06-types.md`

### 0.4.1 Type Paths

- [x] **Audit**: Simple type paths — grammar.ebnf § type_path [done] (2026-02-10)
  - [x] `int`, `Point`, `std.math.Complex` — parses correctly
  - [x] **Rust Tests**: `ori_ir/tests/` — 16 parsed_type tests (primitive, named, generic, nested, associated, function, list, map, tuple, unit)

- [x] **Audit**: Generic type arguments — grammar.ebnf § type_args [done] (2026-02-10)
  - [x] `Option<int>`, `Result<T, E>`, `Map<str, int>` — parses correctly
  - [x] With const: `[int, max 10]` — parses correctly [done] (2026-02-13)
  - [x] `Array<int, $N>` — parses correctly [done] (2026-02-13)

### 0.4.2 Existential Types

- [ ] **Audit**: impl Trait — grammar.ebnf § impl_trait_type  <!-- blocked-by:19 -->
  - [ ] Basic: `impl Iterator` — **BROKEN**: parser rejects `impl` in type position
  - [ ] Multi-trait: `impl Iterator + Clone` — blocked by above
  - [ ] With where: `impl Iterator where Item == int` — blocked by above

### 0.4.3 Compound Types

- [x] **Audit**: List types — grammar.ebnf § list_type [done] (2026-02-10)
  - [x] Dynamic: `[int]`, `[Option<str>]` — parses correctly

- [x] **Audit**: Fixed-capacity list types — grammar.ebnf § fixed_list_type [done] (2026-02-13)
  - [x] `[int, max 10]`, `[T, max N]` — parses correctly (verified via `ori parse`)

- [x] **Audit**: Map types — grammar.ebnf § map_type [done] (2026-02-10)
  - [x] `{str: int}`, `{K: V}` — parses correctly

- [x] **Audit**: Tuple types — grammar.ebnf § tuple_type [done] (2026-02-10)
  - [x] Unit: `()`, pairs: `(int, str)`, nested: `((int, int), str)` — parses correctly

- [x] **Audit**: Function types — grammar.ebnf § function_type [done] (2026-02-10)
  - [x] `() -> void`, `(int) -> int`, `(int, str) -> bool` — parses correctly (verified via typed lambda)

### 0.4.4 Const Expressions in Types

- [x] **Audit**: Const expressions — grammar.ebnf § const_expr [done] (2026-02-13)
  - [x] Literal: `10` in type argument position (e.g., `Array<int, 10>`) — parses correctly
  - [x] Parameter: `$N` in type argument position — parses correctly
  - [x] Arithmetic: `$N + 1`, `$N * 2` in type argument position — parses correctly
  - [x] **Rust Tests**: `ori_parse/src/grammar/ty.rs` — 4 const expression type arg tests

### 0.4.5 Trait Objects

> **SPEC**: `spec/06-types.md` § Trait Objects

- [x] **Audit**: Simple trait objects — spec/06-types.md § Trait Objects [done] (2026-02-10)
  - [x] Trait name as type: `@display (item: Printable) -> void` — parses correctly (verified via `ori parse`)
  - [x] In collections: `[Printable]`, `{str: Printable}` — parses correctly [done] (2026-02-13)
  - [x] **Ori Tests**: `tests/spec/types/trait_objects.ori` — tests exist

- [x] **Audit**: Bounded trait objects — spec/06-types.md § Bounded Trait Objects [done] (2026-02-14)
  - [x] Multiple bounds: `Printable + Hashable` — parses correctly as `TraitBounds` variant [done] (2026-02-14)
  - [x] As parameter type: `@store (item: Printable + Hashable) -> void` — parses correctly [done] (2026-02-14)
  - [x] **Grammar**: `trait_object_bounds` already in grammar.ebnf; parser now implements it [done] (2026-02-14)

---

## 0.5 Expressions

> **SPEC**: `grammar.ebnf` § EXPRESSIONS, `spec/09-expressions.md`

### 0.5.1 Primary Expressions

> **Note**: Pattern expressions (`run`, `try`, `match`, `parallel`, `nursery`, `channel`, etc.) are valid primary expressions per grammar.ebnf § primary → pattern_expr. See **section 0.6** for pattern-specific audit items.

- [x] **Audit**: Literals — grammar.ebnf § primary [done] (2026-02-10)
  - [x] All literal types covered in 0.1 — all parse correctly
  - [x] **Ori Tests**: `tests/spec/lexical/` — 690+ tests across all literal types

- [x] **Audit**: Identifiers and self — grammar.ebnf § primary [done] (2026-02-10)
  - [x] `x`, `self`, `Self` — parse correctly
  - [x] **Ori Tests**: `tests/spec/lexical/identifiers.ori` — 40+ tests

- [x] **Audit**: Grouped expressions — grammar.ebnf § primary [done] (2026-02-10)
  - [x] `(expr)`, nested: `((a + b) * c)` — parse correctly (verified via `ori parse`)

- [x] **Audit**: Length placeholder — grammar.ebnf § primary [done] (2026-02-13)
  - [x] `list[# - 1]` (last element) — parses correctly (verified via `ori parse`)

### 0.5.2 Unsafe Expression

- [x] **Audit**: Unsafe expressions — grammar.ebnf § unsafe_expr [done] (2026-02-10)
  - [x] `unsafe(expr)` — parses correctly (verified via `ori parse`)

### 0.5.3 List Literals

- [x] **Audit**: List literals — grammar.ebnf § list_literal [done] (2026-02-10)
  - [x] Empty: `[]`, simple: `[1, 2, 3]` — parse correctly
  - [x] With spread: `[...a, 4, ...b]` — parses correctly (verified via `ori parse`)

### 0.5.4 Map Literals

- [x] **Audit**: Map literals — grammar.ebnf § map_literal [done] (2026-02-10)
  - [x] Empty: `{}`, simple: `{"a": 1, "b": 2}` — parse correctly
  - [x] String keys: `{"key": value}` — parses correctly
  - [x] Computed keys: `{[expr]: value}` — parses correctly (verified via `ori parse`)
  - [x] With spread: `{...base, extra: 1}` — parses correctly (verified via `ori parse`)

### 0.5.5 Struct Literals

- [x] **Audit**: Struct literals — grammar.ebnf § struct_literal [done] (2026-02-10)
  - [x] Basic: `Point { x: 1, y: 2 }` — parses correctly
  - [x] Shorthand: `Point { x, y }` — parses correctly (verified via `ori parse`)
  - [x] With spread: `Point { ...base, x: 10 }` — parses correctly (verified via `ori parse`)
  - [x] **Ori Tests**: `tests/spec/declarations/struct_types.ori` — 39 active tests

### 0.5.6 Postfix Expressions

- [x] **Audit**: Field/method access — grammar.ebnf § postfix_op [done] (2026-02-10)
  - [x] Field: `point.x` — parses correctly
  - [x] Method: `list.len()` — parses correctly
  - [x] **Ori Tests**: extensive coverage across all test files

- [x] **Audit**: Index access — grammar.ebnf § postfix_op [done] (2026-02-10)
  - [x] `list[0]`, `map["key"]` — parse correctly
  - [x] `list[# - 1]` — parses correctly [done] (2026-02-13)

- [x] **Audit**: Function calls — grammar.ebnf § call_args [done] (2026-02-10)
  - [x] Named: `greet(name: "Alice")` — parses correctly
  - [x] Positional (lambda): `list.map(x -> x * 2)` — parses correctly
  - [x] Spread: `sum(...numbers)` — parses correctly (verified via `ori parse`)
  - [x] **Ori Tests**: `tests/spec/declarations/named_arguments.ori` — 149 lines

- [x] **Audit**: Error propagation — grammar.ebnf § postfix_op [done] (2026-02-10)
  - [x] `result?` — parses correctly (verified via `ori parse`)

- [x] **Audit**: Type conversion — grammar.ebnf § postfix_op [done] (2026-02-10)
  - [x] Infallible: `42 as float` — parses correctly (verified via `ori parse`)
  - [x] Fallible: `"42" as? int` — parses correctly (verified via `ori parse`)

### 0.5.7 Unary Expressions

- [x] **Audit**: Unary operators — grammar.ebnf § unary_expr [done] (2026-02-10)
  - [x] Logical not: `!condition` — parses correctly
  - [x] Negation: `-number` — parses correctly
  - [x] Bitwise not: `~bits` — parses correctly
  - [x] **Ori Tests**: `tests/spec/lexical/operators.ori` — 80+ tests including unary

### 0.5.8 Binary Expressions

- [x] **Audit**: Null coalesce — grammar.ebnf § coalesce_expr [done] (2026-02-10)
  - [x] `option ?? default` — parses correctly (verified via `ori parse`)

- [x] **Audit**: Logical operators — grammar.ebnf § or_expr, and_expr [done] (2026-02-10)
  - [x] `a || b`, `a && b` — parse correctly
  - [x] **Ori Tests**: `tests/spec/lexical/operators.ori`, `tests/spec/lexical/bool_literals.ori`

- [x] **Audit**: Bitwise operators — grammar.ebnf § bit_or_expr, bit_xor_expr, bit_and_expr [done] (2026-02-10)
  - [x] `a | b`, `a ^ b`, `a & b` — parse correctly
  - [x] **Ori Tests**: `tests/spec/lexical/operators.ori`

- [x] **Audit**: Equality operators — grammar.ebnf § eq_expr [done] (2026-02-10)
  - [x] `a == b`, `a != b` — parse correctly
  - [x] **Ori Tests**: `tests/spec/lexical/operators.ori`

- [x] **Audit**: Comparison operators — grammar.ebnf § cmp_expr [done] (2026-02-10)
  - [x] `a < b`, `a > b`, `a <= b`, `a >= b` — parse correctly
  - [x] **Ori Tests**: `tests/spec/lexical/operators.ori`

- [x] **Audit**: Range expressions — grammar.ebnf § range_expr [done] (2026-02-10)
  - [x] Exclusive: `0..10`, inclusive: `0..=10` — parse correctly
  - [x] With step: `0..10 by 2` — parses correctly
  - [x] **Ori Tests**: `tests/spec/lexical/operators.ori`

- [x] **Audit**: Shift operators — grammar.ebnf § shift_expr [done] (2026-02-10)
  - [x] `a << n`, `a >> n` — parse correctly
  - [x] **Ori Tests**: `tests/spec/lexical/operators.ori`

- [x] **Audit**: Arithmetic operators — grammar.ebnf § add_expr, mul_expr [done] (2026-02-10)
  - [x] `a + b`, `a - b`, `a * b`, `a / b`, `a % b`, `a div b` — parse correctly
  - [x] **Ori Tests**: `tests/spec/lexical/operators.ori`

### 0.5.9 With Expression

- [x] **Audit**: Capability provision — grammar.ebnf § with_expr [done] (2026-02-10)
  - [x] `with x = value in expr` — parses correctly (verified via `ori parse`)
  - [x] Capability form: `with Http = MockHttp in expr` — parses correctly [done] (2026-02-13)

### 0.5.10 Let Binding

- [x] **Audit**: Let expressions — grammar.ebnf § let_expr [done] (2026-02-10)
  - [x] Mutable: `let x = 42` — parses correctly
  - [x] Immutable: `let $x = 42` — parses correctly
  - [x] Typed: `let x: int = 42` — parses correctly
  - [x] **Ori Tests**: extensive coverage across all test files (3068 tests use let bindings)

- [x] **Audit**: Assignment — grammar.ebnf § assignment [done] (2026-02-10)
  - [x] `x = new_value` — parses correctly

### 0.5.11 Conditional

- [x] **Audit**: If expressions — grammar.ebnf § if_expr [done] (2026-02-10)
  - [x] Simple: `if cond then a else b` — parses correctly
  - [x] Void: `if cond then action` — parses correctly
  - [x] Chained: `if c1 then a else if c2 then b else c` — parses correctly (verified via `ori parse`)
  - [x] **Ori Tests**: extensive coverage across test files

### 0.5.12 For Expression

- [x] **Audit**: For loops — grammar.ebnf § for_expr [done] (2026-02-10)
  - [x] Do: `for x in items do action` — parses correctly
  - [x] Yield: `for x in items yield x * 2` — parses correctly (verified via `ori parse`)
  - [x] Filter: `for x in items if x > 0 yield x` — parses correctly (verified via `ori parse`)
  - [x] Labeled: `for:outer x in items do ...` — parses correctly [done] (2026-02-14)

### 0.5.13 Loop Expression

- [x] **Audit**: Loop expressions — grammar.ebnf § loop_expr [done] (2026-02-14)
  - [x] Basic: `loop { body }` — parses correctly (verified via `ori parse`)
  - [x] Labeled: `loop:name { body }` — parses correctly [done] (2026-02-14)

### 0.5.14 Labels

- [x] **Audit**: Loop labels — grammar.ebnf § label [done] (2026-02-14)
  - [x] `:name` (no space around colon) — parses correctly (verified via labeled for/loop/break/continue)

### 0.5.15 Lambda

- [x] **Audit**: Simple lambdas — grammar.ebnf § simple_lambda [done] (2026-02-10)
  - [x] Single param: `x -> x + 1` — parses correctly
  - [x] Multiple: `(a, b) -> a + b` — parses correctly
  - [x] No params: `() -> 42` — parses correctly
  - [x] **Ori Tests**: extensive coverage (lambdas used throughout test suite)

- [x] **Audit**: Typed lambdas — grammar.ebnf § typed_lambda [done] (2026-02-10)
  - [x] `(x: int) -> int = x * 2` — parses correctly (verified via `ori parse`)

### 0.5.16 Control Flow

- [x] **Audit**: Break expression — grammar.ebnf § break_expr [done] (2026-02-14)
  - [x] Simple: `break` — parses correctly
  - [x] With value: `break result` — parses correctly (verified via loop test)
  - [x] Labeled: `break:outer`, `break:outer result` — parses correctly [done] (2026-02-14)

- [x] **Audit**: Continue expression — grammar.ebnf § continue_expr [done] (2026-02-14)
  - [x] Simple: `continue` — parses correctly (verified via `ori parse`)
  - [x] With value: `continue replacement` — parses correctly [done] (2026-02-13)
  - [x] Labeled: `continue:outer` — parses correctly [done] (2026-02-14)

---

## 0.6 Patterns

> **SPEC**: `grammar.ebnf` § PATTERNS, `spec/10-patterns.md`

### 0.6.1 Sequential Patterns

- [x] **Audit**: Block expressions and function contracts — grammar.ebnf § block_expr [done] (2026-02-14)
  - [x] Basic: `{ let x = a \n result }` — parses correctly (verified via `ori parse`)
  - [x] Pre-contract: `pre(cond)` on function declaration — parses correctly [done] (2026-02-14)
  - [x] Post-contract: `post(r -> cond)` on function declaration — parses correctly [done] (2026-02-14)
  - [x] Pre-contract with message: `pre(cond | "msg")` — parses correctly [done] (2026-02-14)
  - [x] Post-contract with message: `post(r -> cond | "msg")` — parses correctly [done] (2026-02-14)
  - [x] Multiple pre-contracts: `pre(a) pre(b)` — parses correctly [done] (2026-02-14)
  - [ ] **Enforcement**: pre/post contracts not yet enforced at runtime  <!-- blocked-by:23 -->

- [x] **Audit**: Try pattern — grammar.ebnf § try_expr [done] (2026-02-13)
  - [x] `try { let x = f()? \n Ok(x) }` — parses correctly (verified via `ori parse`)

- [x] **Audit**: Match pattern — grammar.ebnf § match_expr [done] (2026-02-10)
  - [x] `match expr { Some(x) -> x, None -> default }` — parses correctly (verified via `ori parse`)

- [x] **Audit**: Guard syntax — grammar.ebnf § guard [done] (2026-02-14)
  - [x] `.match(...)` syntax — verified (now superseded: guards use `if` syntax per match-arm-comma-separator-proposal)

- [x] **Audit**: For pattern — grammar.ebnf § for_pattern [done] (2026-02-10)
  - [x] Basic form: `for(over: items, match: x -> x, default: 0)` — parses correctly (verified via `ori parse`)

- [x] **Audit**: Catch pattern — grammar.ebnf § catch_expr [done] (2026-02-10)
  - [x] `catch(expr: risky_operation)` — parses correctly (verified via `ori parse`)

- [x] **Audit**: Nursery pattern — grammar.ebnf § nursery_expr [done] (2026-02-10)
  - [x] `nursery(body: n -> ..., on_error: CancelRemaining)` — parses correctly (verified via `ori parse`)

### 0.6.2 Function Expression Patterns

- [x] **Audit**: Pattern arguments — grammar.ebnf § pattern_arg [done] (2026-02-10)
  - [x] Named argument syntax: `identifier ":" expression` — parses correctly
  - [x] All function_exp patterns use this form — verified across recurse/parallel/spawn/timeout/cache
  - [x] **Ori Tests**: Named args verified through pattern tests

- [x] **Audit**: Recurse pattern — grammar.ebnf § function_exp [done] (2026-02-10)
  - [x] `recurse(condition: n -> n > 0, base: 1, step: n -> n - 1)` — parses correctly (verified via `ori parse`)
  - [x] With memo: `recurse(..., memo: true)` — parses correctly [done] (2026-02-13)

- [x] **Audit**: Parallel pattern — grammar.ebnf § function_exp [done] (2026-02-10)
  - [x] `parallel(tasks: [...], max_concurrent: 4)` — parses correctly (verified via `ori parse`)
  - [x] With timeout: `parallel(tasks: [...], timeout: 10s)` — parses correctly [done] (2026-02-13)

- [x] **Audit**: Spawn pattern — grammar.ebnf § function_exp [done] (2026-02-10)
  - [x] `spawn(tasks: [...], max_concurrent: 10)` — parses correctly (verified via `ori parse`)

- [x] **Audit**: Timeout pattern — grammar.ebnf § function_exp [done] (2026-02-10)
  - [x] `timeout(op: expr, after: 5s)` — parses correctly (verified via `ori parse`)

- [x] **Audit**: Cache pattern — grammar.ebnf § function_exp [done] (2026-02-10)
  - [x] `cache(key: k, op: 42, ttl: 1h)` — parses correctly (verified via `ori parse`)

- [x] **Audit**: With pattern (RAII) — grammar.ebnf § function_exp [done] (2026-02-14)
  - [x] `with(resource: expr, body: f -> ...)` — parses correctly with named args (verified via `ori parse`)
  - [x] `with(acquire: expr, action: f -> ..., release: f -> ...)` — parses correctly [done] (2026-02-14). Spec uses `action:` not `use:` (because `use` is a reserved keyword)
  - [x] **Note**: `with` keyword disambiguated correctly: `with Ident =` → capability provision, `with(` → RAII pattern

### 0.6.3 Type Conversion Patterns

- [x] **Audit**: Type conversion calls — grammar.ebnf § function_val [done] (2026-02-10)
  - [x] `int(x)`, `float(x)`, `str(x)`, `byte(x)` — all parse correctly (verified via `ori parse`)
  - [x] **Ori Tests**: `tests/spec/expressions/type_conversion.ori` — if exists; verified parsing via temp files

### 0.6.4 Channel Constructors

- [x] **Audit**: Channel creation — grammar.ebnf § channel_expr [done] (2026-02-14)
  - [x] `channel<int>(buffer: 10)` — FIXED [done] (2026-02-14): Added FunctionExpKind::Channel variants + parse_channel_expr with generic type args
  - [x] `channel_in<T>(buffer: 5)`, `channel_out<T>(buffer: 5)` — FIXED [done] (2026-02-14)
  - [x] `channel_all<T>(buffer: 5)` — FIXED [done] (2026-02-14)
  - [x] `channel(buffer: 10)` — still works (no generics)

### 0.6.5 Match Patterns

- [x] **Audit**: Literal patterns — grammar.ebnf § literal_pattern [done] (2026-02-10)
  - [x] Int: `42`, `-1` — parses correctly in match arms
  - [x] String: `"hello"` — parses correctly in match arms
  - [x] Bool: `true`, `false` — parses correctly in match arms
  - [x] Char: `'a'` — parses and evaluates correctly [done] (2026-02-14)
  - [x] **Verified**: `match 42 { 42 -> 1, _ -> 0 }` parses correctly (via `ori parse`)

- [x] **Audit**: Identifier pattern — grammar.ebnf § identifier_pattern [done] (2026-02-10)
  - [x] `x` (binds value) — parses correctly (verified via `ori parse`)

- [x] **Audit**: Wildcard pattern — grammar.ebnf § wildcard_pattern [done] (2026-02-10)
  - [x] `_` — parses correctly in match arms (verified via `ori parse`)

- [x] **Audit**: Variant patterns — grammar.ebnf § variant_pattern [done] (2026-02-10)
  - [x] `Red`, `Green`, `Blue` — unit variants parse correctly in match arms (verified via `ori parse`)
  - [x] `Some(x)`, `Ok(value)`, `Err(e)` — parses correctly with named-field sum types [done] (2026-02-13)

- [x] **Audit**: Struct patterns — grammar.ebnf § struct_pattern (partial) [done] (2026-02-10)
  - [x] `{ x, y }` — parses correctly in match arms (verified via `ori parse`)
  - [x] `{ x: px, y: py }` — parses correctly [done] (2026-02-13)
  - [x] With rest: `{ x, .. }` — parses and evaluates correctly [done] (2026-02-14)

- [x] **Audit**: Tuple patterns — grammar.ebnf § tuple_pattern [done] (2026-02-10)
  - [x] `(a, b)`, `(x, y, z)` — parse correctly in match arms (verified via `ori parse`)

- [x] **Audit**: List patterns — grammar.ebnf § list_pattern [done] (2026-02-10)
  - [x] `[a, b, c]` — parses correctly in match arms (verified via `ori parse`)
  - [x] `[head, ..tail]` — parses correctly (verified via `ori parse`)
  - [x] Rest only: `[..]`, `[..rest]` — parses correctly [done] (2026-02-13)

- [x] **Audit**: Range patterns — grammar.ebnf § range_pattern [done] (2026-02-10)
  - [x] `1..10` — parses correctly in match arms (verified via `ori parse`)
  - [x] `'a'..='z'` — char range patterns parse and evaluate correctly [done] (2026-02-14)

- [x] **Audit**: Or patterns — grammar.ebnf § or_pattern [done] (2026-02-10)
  - [x] `1 | 2` — parses correctly in match arms (verified via `ori parse`)
  - [x] `Some(1) | Some(2)` — variant or-patterns parse correctly [done] (2026-02-13)

- [x] **Audit**: At patterns — grammar.ebnf § at_pattern [done] (2026-02-10)
  - [x] `x @ 42` — parses correctly in match arms (verified via `ori parse`)
  - [x] `list @ [_, ..]` — parses correctly [done] (2026-02-13)

### 0.6.6 Binding Patterns

- [x] **Audit**: Identifier bindings — grammar.ebnf § binding_pattern [done] (2026-02-14)
  - [x] Mutable: `let x = 42` — parses correctly (verified via `ori parse`)
  - [x] Immutable: `let $x = 42` in function body — parses and evaluates correctly [done] (2026-02-14)
  - [x] Wildcard: `_` — parses correctly (verified in for loops and match arms)

- [x] **Audit**: Struct destructure — grammar.ebnf § binding_pattern [done] (2026-02-14)
  - [x] `let { x, y } = Point { ... }` — parses correctly (verified via `ori parse`)
  - [x] `let { x: px, y: py } = ...` — parses correctly [done] (2026-02-13)
  - [x] Immutable: `let { $x, $y } = ...` — parses and evaluates correctly [done] (2026-02-14)

- [x] **Audit**: Tuple destructure — grammar.ebnf § binding_pattern [done] (2026-02-14)
  - [x] `let (a, b) = (1, 2)` — parses correctly (verified via `ori parse`)
  - [x] `let ($a, $b) = ...` — parses and evaluates correctly [done] (2026-02-14)

- [x] **Audit**: List destructure — grammar.ebnf § binding_pattern [done] (2026-02-14)
  - [x] `let [head, ..tail] = [1, 2, 3]` — parses correctly (verified via `ori parse`)
  - [x] `let [$first, $second, ..rest] = ...` — parses and evaluates correctly [done] (2026-02-14)

---

## 0.7 Constant Expressions

> **SPEC**: `grammar.ebnf` § CONSTANT EXPRESSIONS, `spec/04-constants.md`, `spec/21-constant-expressions.md`

- [x] **Audit**: Literal const expressions — grammar.ebnf § const_expr [done] (2026-02-10)
  - [x] `let $A = 42`, `let $B = true`, `let $C = "hello"` — all parse correctly (verified via `ori parse`)

- [x] **Audit**: Arithmetic const expressions — grammar.ebnf § const_expr [done] (2026-02-14)
  - [x] `let $D = $A + 1`, `let $E = $A * 2` — parses correctly (verified via `ori parse`)
  - [x] **Fix**: Replaced `parse_literal_expr()` with `parse_expr()` in constant initializer parsing

- [x] **Audit**: Comparison const expressions — grammar.ebnf § const_expr [done] (2026-02-14)
  - [x] `let $F = $A > 0` — parses correctly (verified via `ori parse`)

- [x] **Audit**: Logical const expressions — grammar.ebnf § const_expr [done] (2026-02-14)
  - [x] `let $G = $A && $B` — parses correctly (verified via `ori parse`)

- [x] **Audit**: Grouped const expressions — grammar.ebnf § const_expr [done] (2026-02-14)
  - [x] `let $H = ($A + 1) * 2` — parses correctly (verified via `ori parse`)

---

## 0.8 Section Completion Checklist

> **STATUS**: In Progress — Re-verified 2026-02-14. 8/10 checklist items complete, 2 blocked. Remaining: 0.4 audit (blocked-by:19, only impl Trait), 0.6 audit (blocked-by:23, only runtime enforcement). All unblocked parser work is complete. 3176 Ori tests pass, 1443 Rust tests pass.

- [x] All lexical grammar items audited and tested (0.1) [done] (2026-02-10)
- [x] All source structure items audited and tested (0.2) [done] (2026-02-13) — file attributes, extern `as`, C variadics all work now
- [x] All declaration items audited and tested (0.3) [done] (2026-02-14) — 92/92 checkboxes complete; typed constants, const generics, floating tests, clause params, guard clauses, variadic params all work
- [ ] All type items audited and tested (0.4) — 30/34 complete; only impl Trait remains (0.4.2, 4 items)  <!-- blocked-by:19 -->
- [x] All expression items audited and tested (0.5) [done] (2026-02-14) — length placeholder `#` now works; labeled break/continue/for/loop NOW WORK [done] (2026-02-14)
- [ ] All pattern items audited and tested (0.6) — 93/94 complete; only runtime contract enforcement remains (0.6.1, 1 item)  <!-- blocked-by:23 -->
- [x] All constant expression items audited and tested (0.7) [done] (2026-02-14) — computed constants now work (arithmetic, comparison, logical, grouped)
- [x] Run `cargo t -p ori_parse` — all parser tests pass [done] (2026-02-14)
- [x] Run `cargo t -p ori_lexer` — all lexer tests pass [done] (2026-02-14)
- [x] Run `cargo st tests/` — 3176 passed, 0 failed, 59 skipped [done] (2026-02-14)

**Exit Criteria**: Every grammar production in `grammar.ebnf` has verified parser support with tests.

**Remaining Parser Bugs (verified 2026-02-13):**
- Const functions (`$name (params)`) — rejected  <!-- blocked-by:18 -->
- ~~Computed constants (`let $D = $A + 1`)~~ — FIXED [done] (2026-02-14)
- impl Trait in type position — rejected  <!-- blocked-by:19 -->
- Associated type constraints (`where I.Item == int`) — `==` rejected  <!-- blocked-by:3 -->
- ~~Floating tests (`tests _`)~~ — FIXED [done] (2026-02-14)
- ~~Typed constants (`let $X: int`)~~ — FIXED [done] (2026-02-14)
- ~~Channel generic syntax (`channel<int>`)~~ — FIXED [done] (2026-02-14)
- ~~Struct rest pattern (`{ x, .. }`)~~ — FIXED [done] (2026-02-14)
- ~~`.match()` method syntax~~ — FIXED [done] (2026-02-14) — desugars `expr.match { arms }` to `match expr { arms }` at parser level
- ~~`with()` RAII pattern (`acquire:/action:/release:`)~~ — FIXED [done] (2026-02-14) — was doc error: spec uses `action:` not `use:` (`use` is reserved keyword)
- ~~Function-level contracts (`pre()`/`post()`)~~ — FIXED [done] (2026-02-14)
- ~~Immutable binding in function body (`let $x = 42`)~~ — FIXED [done] (2026-02-14)
- ~~Labeled continue (`continue:outer`)~~ — FIXED [done] (2026-02-14)
- ~~Char patterns in match (`'a'`, `'a'..='z'`)~~ — FIXED [done] (2026-02-14)

**Fixed since 2026-02-10** (23 items):
File attributes, extern `as` alias, C variadics, pattern params, guard clauses, default params, variadic params, `#repr`/`#target`/`#cfg` attributes, fixed-capacity lists, length placeholder, try `?` inside `try { }`, const generic type args (`Array<int, $N>`), const expressions in types, const bounds in where clauses (`where N > 0`), labeled continue (`continue:outer`), function-level contracts (`pre()`/`post()`), computed constants (`let $D = $A + 1`), struct rest pattern (`{ x, .. }`), immutable bindings in function bodies (`let $x`, `let ($a, $b)`, `let { $x }`, `let [$h, ..]`), `with()` RAII pattern (`acquire:/action:/release:` — was doc error, spec uses `action:` not `use:`), `.match()` method syntax (`expr.match { arms }` desugars to `match expr { arms }`)

---

## 0.9 Parser Bugs (from Comprehensive Tests)

> **STATUS**: Re-verified 2026-02-13. Many items previously marked "STILL BROKEN" now parse correctly.

> **POLICY**: Skipping tests is NOT acceptable. Every test must pass. If a feature is tested, it must work. Fix the code, not the tests.

This section documents **parser-only** bugs discovered by the comprehensive test suite. Evaluator/type checker bugs are tracked in **Section 23: Full Evaluator Support**.

### 0.9.1 Parser/Syntax Bugs — STILL BROKEN (verified 2026-02-13)

These features fail at the parse phase — the parser does not recognize the syntax.

- [x] **Implement**: Const generics parser support [done] (2026-02-13)
  - [x] **Parser**: `$N: int` in generic parameters — was already working
  - [x] **Syntax**: `Array<int, $N>` — const expressions in type arguments (NEW)
  - [x] **Syntax**: `[int, max $N]` — const expressions in fixed-list capacity (NEW)
  - [x] **Syntax**: `where N > 0` — const bounds in where clauses (NEW)

- [ ] **Implement**: Associated type constraints in where clauses  <!-- blocked-by:3 -->
  - [ ] **Syntax**: `where I.Item == int` — **BROKEN**: parser expects `:`, finds `==`

- [ ] **Implement**: Const functions  <!-- blocked-by:18 -->
  - [ ] **Syntax**: `$add (a: int, b: int) = a + b` — **BROKEN**: parser error

- [x] **Implement**: Computed constants [done] (2026-02-14)
  - [x] **Syntax**: `let $D = $A + 1` — parses correctly (constant initializer now uses general expression parser)

- [ ] **Implement**: `impl Trait` in type position  <!-- blocked-by:19 -->
  - [ ] **Syntax**: `@f () -> impl Iterator` — **BROKEN**: parser rejects `impl` in type

- [x] **Implement**: Channel generic syntax [done] (2026-02-14)
  - [x] **Syntax**: `channel<int>(buffer: 10)` — FIXED: detect channel identifiers in parse_primary, parse_channel_expr with generic type args

- [x] **Implement**: Struct rest pattern in match [done] (2026-02-14)
  - [x] **Syntax**: `{ x, .. }` — parses and evaluates correctly [done] (2026-02-14)

- [x] **Implement**: `.match()` method syntax [done] (2026-02-14)
  - [x] **Syntax**: `42.match { ... }` — method-style match desugars to `match 42 { ... }` at parse level [done] (2026-02-14)

- [x] **Implement**: Immutable bindings in function bodies [done] (2026-02-14)
  - [x] **Syntax**: `let $x = 42` inside function — parses and evaluates correctly [done] (2026-02-14)
  - [x] **Syntax**: `let ($a, $b) = ...` — tuple destructuring with `$` [done] (2026-02-14)
  - [x] **Syntax**: `let { $x, $y } = ...` — struct destructuring with `$` [done] (2026-02-14)
  - [x] **Syntax**: `let [$first, ..rest] = ...` — list destructuring with `$` [done] (2026-02-14)

- [x] **Implement**: Floating tests with `_` target [done] (2026-02-14)
  - [x] **Syntax**: `@t tests _ () -> void = ...` — parses correctly [done] (2026-02-14)

- [x] **Implement**: Typed constants [done] (2026-02-14)
  - [x] **Syntax**: `let $MAX_SIZE: int = 1000` — parses correctly [done] (2026-02-14)

- [x] **Implement**: Function-level contracts (`pre()`/`post()`) [done] (2026-02-14)
  - [x] **Syntax**: `pre(cond)` on function declaration — parses correctly [done] (2026-02-14)
  - [x] **Syntax**: `post(r -> cond)` on function declaration — parses correctly [done] (2026-02-14)
  - [x] **Syntax**: `pre(c | "msg") post(r -> c | "msg")` — parses correctly [done] (2026-02-14)
  - [ ] **Enforcement**: Runtime contract execution tracked in Section 23  <!-- blocked-by:23 -->

- [x] **Implement**: `with(acquire:, action:, release:)` RAII pattern [done] (2026-02-14)
  - [x] **Syntax**: `with(acquire: expr, action: f -> ..., release: f -> ...)` — parses correctly. Spec uses `action:` not `use:` (`use` is reserved keyword)

- [x] **Implement**: Labeled continue [done] (2026-02-14)
  - [x] **Syntax**: `continue:outer` — parses correctly [done] (2026-02-14)

- [x] **Implement**: Char patterns in match [done] (2026-02-14)
  - [x] **Syntax**: `'a'` in match arm — parses and evaluates correctly
  - [x] **Syntax**: `'a'..='z'` range — parses and evaluates correctly

### 0.9.2 Previously Fixed Bugs — VERIFIED WORKING

These features were previously reported as broken but now parse correctly.

- [x] **Fixed**: Guard clauses — `@f (n: int) -> int if n > 0 = n` [done] (2026-02-13)
- [x] **Fixed**: Pattern params — `@fib (0: int) -> int = 1` [done] (2026-02-13)
- [x] **Fixed**: Default params — `@greet (name: str = "World") -> str` [done] (2026-02-13)
- [x] **Fixed**: Variadic parameters — `@sum (nums: ...int)` [done] (2026-02-13)
- [x] **Fixed**: `#repr` attribute — `#repr("c")` [done] (2026-02-13)
- [x] **Fixed**: `#target` attribute — `#target(os: "linux")` [done] (2026-02-13)
- [x] **Fixed**: `#cfg` attribute — `#cfg(debug)` [done] (2026-02-13)
- [x] **Fixed**: Fixed-capacity list type — `[int, max 10]` [done] (2026-02-13)
- [x] **Fixed**: File-level attributes — `#!target(os: "linux")` [done] (2026-02-13)
- [x] **Fixed**: Extern `as` alias — `@_sin (x: float) -> float as "sin"` [done] (2026-02-13)
- [x] **Fixed**: C variadics — `@printf (fmt: CPtr, ...) -> c_int` [done] (2026-02-13)
- [x] **Fixed**: Try `?` inside try — `try { let x = f()? \n Ok(x) }` [done] (2026-02-13)
- [x] **Fixed**: Length placeholder — `list[# - 1]` [done] (2026-02-13)
- [x] **Fixed**: Spread in function calls — `sum(...list)` [done] (2026-02-10)
- [x] **Fixed**: `timeout` as identifier — `let timeout = 5` [done] (2026-02-10)
- [x] **Fixed**: List spread syntax — `[...result, i]` [done] (2026-02-10)
- [x] **Fixed**: Map spread syntax — `{...base, "c": 3}` [done] (2026-02-10)
- [x] **Fixed**: Tuple destructuring in for loops — `for (k, v) in m do ...` [done] (2026-02-10)
- [x] **Fixed**: Multiple derives — `#derive(Eq, Clone, Debug)` [done] (2026-02-10)
- [x] **Fixed**: `as`/`as?` type conversion — `42 as float`, `"42" as? int` [done] (2026-02-10)
- [x] **Fixed**: Wildcard pattern in for loops — `for _ in 0..n do ...` [done] (2026-02-10)
- [x] **Fixed**: Context-sensitive pattern keywords — `let timeout = 5`, `let cache = 10` [done] (2026-02-10)

---

> **NOTE**: Type checker and evaluator bugs have been moved to **Section 23: Full Evaluator Support**.

---

## Completion Summary

**Full Audit Completed: 2026-02-10**

Systematic `ori parse` verification of every grammar production against actual parser behavior.

**Previously Fixed (verified working):**
1. `div` operator in `match_multiplicative_op()`
2. Spread in function calls (`sum(...list)`)
3. Context-sensitive keywords (`timeout`, `parallel`, `cache` as identifiers)
4. List spread (`[...a, ...b]`), map spread (`{...base, key: val}`)
5. Tuple destructure in for (`for (k, v) in m`)
6. Multiple derives (`#derive(Eq, Clone, Debug)`)
7. `as`/`as?` type conversion operators
8. Wildcard in for loops (`for _ in range`)

**Verified Parser Bugs — 24 items originally, 19 fixed, ~5 remain:**
1. Guard clauses (`if` before `=`) — ~~parser rejects~~ FIXED [done]
2. List/pattern params (`@fib (0: int)`) — ~~parser rejects~~ FIXED [done]
3. Const generics (`$` in generics) — ~~parser rejects~~ FIXED [done] (2026-02-13)
4. Variadic params (`...int`) — ~~parser rejects~~ FIXED [done]
5. `#repr`/`#target`/`#cfg` attributes — unknown attribute error
6. Associated type constraints (`where I.Item == int`) — `==` rejected
7. Const functions (`$name (params)`) — parser error
8. ~~Computed constants (`let $D = $A + 1`)~~ — FIXED [done] (2026-02-14)
9. Fixed-capacity lists (`[T, max N]`) — comma rejected in type
10. `impl Trait` in type position — parser rejects
11. ~~Channel generics (`channel<int>`)~~ — FIXED [done] (2026-02-14)
12. Struct rest pattern (`{ x, .. }`) — `..` rejected
13. `.match()` method syntax — keyword conflict
14. `with()` RAII pattern (`acquire:/use:/release:`) — named arg rejection
15. File attributes (`#!target(...)`) — `!` rejected
16. Extern `as` alias — rejected
17. C variadics (`...` in params) — rejected
18. ~~Floating tests (`tests _`)~~ — FIXED [done] (2026-02-14)
19. ~~Typed constants (`let $X: int = 1000`)~~ — FIXED [done] (2026-02-14)
20. Try `?` inside `try()` — rejected
21. ~~Function-level contracts (`pre()`/`post()`)~~ — FIXED [done] (2026-02-14)
22. Length placeholder (`#`) — attribute marker conflict
23. ~~Immutable binding in function body (`let $x = 42`)~~ — FIXED [done] (2026-02-14)
24. ~~`.match()` guard method syntax~~ — RESOLVED: guards now use `if` syntax (match-arm-comma-separator-proposal)

**Known Limitations (Parser works, but semantics incomplete — tracked in Section 23):**
- `??` operator: Parses but evaluator support incomplete
- Primitive trait methods: Parse but evaluator doesn't resolve
- Map indexing semantics: Parses but returns wrong type

---

## Notes

- This section intentionally overlaps with other roadmap sections
- Other sections add semantics; this section ensures syntax parses
- Work can proceed in any order within this section
- Parser tests should verify both success and error cases
- Error messages should include spans for IDE integration

---

## Grammar Inconsistencies Identified

The following grammar inconsistencies were identified during the audit and require resolution:

### 1. Extension Generics (grammar.ebnf § extension_def) — RESOLVED

**Resolution:** Updated grammar.ebnf to support generics and any type in extensions:
```ebnf
extension_def = "extend" [ generics ] type [ where_clause ] "{" { method } "}" .
```

This allows:
```ori
extend<T: Clone> [T] { ... }           // Angle bracket generics
extend [T] where T: Clone { ... }       // List type with where clause
extend Iterator where Self.Item: Add { ... }
```

### 2. Bounded Trait Objects (grammar.ebnf § type) — RESOLVED

**Resolution:** Added `trait_object_bounds` production to grammar.ebnf:
```ebnf
type = type_path [ type_args ]
     | trait_object_bounds            /* Printable + Hashable */
     | list_type | fixed_list_type | map_type | tuple_type | function_type
     | impl_trait_type .

trait_object_bounds = type_path "+" type_path { "+" type_path } .
```

---

**Resolution Status:** All grammar inconsistencies resolved.

---

## 0.10 Block Expression Syntax (PRIORITY)

**Proposal**: `proposals/approved/block-expression-syntax.md`
**Migration script**: `scripts/migrate_block_syntax.py`

> **This section blocks all other roadmap work.** Every feature built on `run()`/`match()`/`try()` syntax creates migration debt. Complete this before continuing.

### Overview

Replace parenthesized `function_seq` syntax with curly-brace block expressions. Remove `run()` entirely. Move contracts to function-level `pre()`/`post()` declarations.

| Old | New |
|-----|-----|
| `run(a, b, c)` | `{ a \n b \n c }` |
| `match(expr, P -> e)` | `match expr { P -> e }` |
| `try(a, b)` | `try { a \n b }` |
| `loop(run(a, b))` | `loop { a \n b }` |
| `unsafe(run(a, b))` | `unsafe { a \n b }` |
| `run(pre_check: c, body, post_check: r -> c)` | `pre(c) post(r -> c)` on function decl |

### Implementation

#### Phase 1: Parser — Block Expressions
- [ ] **Implement**: Newline-as-separator tokenization inside `{ }` blocks
  - [ ] **Rust Tests**: `ori_parse/src/tests/parser.rs` — newline separation tests
  - [ ] **Ori Tests**: `tests/spec/syntax/blocks/newline_separation.ori`
- [ ] **Implement**: Block expression parsing (`{ block_body }`)
  - [ ] **Rust Tests**: `ori_parse/src/tests/parser.rs` — block expression tests
  - [ ] **Ori Tests**: `tests/spec/syntax/blocks/basic_blocks.ori`
- [ ] **Implement**: Block vs map vs struct disambiguation (2-token lookahead)
  - [ ] **Rust Tests**: `ori_parse/src/tests/parser.rs` — disambiguation tests
  - [ ] **Ori Tests**: `tests/spec/syntax/blocks/disambiguation.ori`
- [ ] **Implement**: Balanced delimiter continuation rules (newlines suppressed inside `()`, `[]`, `{}`)
  - [ ] **Rust Tests**: `ori_parse/src/tests/parser.rs` — continuation tests
  - [ ] **Ori Tests**: `tests/spec/syntax/blocks/continuation.ori`

#### Phase 2: Parser — Construct Migration
- [ ] **Implement**: `match expr { arms }` syntax (scrutinee before block)
  - [ ] **Rust Tests**: `ori_parse/src/tests/parser.rs` — match block syntax
  - [ ] **Ori Tests**: `tests/spec/syntax/blocks/match_block.ori`
- [ ] **Implement**: `try { block_body }` syntax
  - [ ] **Rust Tests**: `ori_parse/src/tests/parser.rs` — try block syntax
  - [ ] **Ori Tests**: `tests/spec/syntax/blocks/try_block.ori`
- [ ] **Implement**: `loop { block_body }` syntax (drop parens)
  - [ ] **Ori Tests**: `tests/spec/syntax/blocks/loop_block.ori`
- [ ] **Implement**: `unsafe { block_body }` syntax (retain `unsafe(expr)` for single-expression)
  - [ ] **Ori Tests**: `tests/spec/syntax/blocks/unsafe_block.ori`
- [ ] **Implement**: `for...do { block_body }` and `for...yield { block_body }`
  - [ ] **Ori Tests**: `tests/spec/syntax/blocks/for_block.ori`
- [ ] **Implement**: Remove old `run()`/`match()`/`try()` paren forms from parser
  - [ ] **Rust Tests**: Verify old syntax produces helpful error messages
  - [ ] **Ori Tests**: `tests/compile-fail/syntax/old_run_syntax.ori`

#### Phase 3: Parser — Function-Level Contracts
- [ ] **Implement**: `pre(condition)` parsing between return type and `=`
  - [ ] **Rust Tests**: `ori_parse/src/tests/parser.rs` — pre contract parsing
  - [ ] **Ori Tests**: `tests/spec/syntax/contracts/pre_basic.ori`
- [ ] **Implement**: `post(r -> condition)` parsing between return type and `=`
  - [ ] **Rust Tests**: `ori_parse/src/tests/parser.rs` — post contract parsing
  - [ ] **Ori Tests**: `tests/spec/syntax/contracts/post_basic.ori`
- [ ] **Implement**: Multiple `pre()`/`post()` declarations
  - [ ] **Ori Tests**: `tests/spec/syntax/contracts/multiple_contracts.ori`
- [ ] **Implement**: Message syntax `pre(condition | "message")`
  - [ ] **Ori Tests**: `tests/spec/syntax/contracts/contract_messages.ori`
- [ ] **Implement**: IR changes — move `pre_checks`/`post_checks` from `FunctionSeq::Run` to function definition node

#### Phase 4: Migration
- [ ] **Run**: `scripts/migrate_block_syntax.py` on all documentation (.md files)
- [ ] **Manual**: Migrate `pre_check:`/`post_check:` references to function-level `pre()`/`post()`
- [ ] **Run**: `scripts/migrate_block_syntax.py` on all `.ori` test files
- [ ] **Update**: `grammar.ebnf` with new block/match/try/contract rules
- [ ] **Update**: `.claude/rules/ori-syntax.md` with new syntax
- [ ] **Update**: Spec files with new syntax (invoke `/sync-spec`)
- [ ] **Verify**: `./test-all.sh` passes with new syntax

#### Phase 5: Formatter
- [ ] **Implement**: Block formatting rules (indentation, newline separation)
- [ ] **Implement**: Blank-line-before-result enforcement
- [ ] **Implement**: Match block formatting (arm alignment)
- [ ] **Implement**: Contract formatting (pre/post between signature and `=`)
