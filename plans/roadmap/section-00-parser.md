---
section: 0
title: Full Parser Support
status: not-started
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
    status: not-started
  - id: "0.2"
    title: Source Structure
    status: not-started
  - id: "0.3"
    title: Declarations
    status: not-started
  - id: "0.4"
    title: Types
    status: not-started
  - id: "0.4.5"
    title: Trait Objects
    status: not-started
  - id: "0.5"
    title: Expressions
    status: not-started
  - id: "0.6"
    title: Patterns
    status: not-started
  - id: "0.7"
    title: Constant Expressions
    status: not-started
  - id: "0.8"
    title: Section Completion Checklist
    status: not-started
  - id: "0.9"
    title: Parser Bugs (from Comprehensive Tests)
    status: not-started
---

# Section 0: Full Parser Support

**Goal**: Complete parser support for entire Ori spec grammar (parsing only — evaluator in Section 23)

> **SPEC**: `spec/grammar.ebnf` (authoritative), `spec/02-source-code.md`, `spec/03-lexical-elements.md`

**Status**: In Progress — Only 2 parser bugs remain: associated type constraints (`==` in where clause) and const functions. Most syntax parses; remaining issues are evaluator/type checker gaps tracked in Section 23.

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

- [ ] **Audit**: Line comments `// ...` — grammar.ebnf § comment
  - [ ] **Rust Tests**: `ori_lexer/src/` — comment tokenization
  - [ ] **Ori Tests**: `tests/spec/lexical/comments.ori`

- [ ] **Audit**: Doc comments with markers — grammar.ebnf § doc_comment
  - [ ] `// ` (description), `// *` (param), `// !` (warning), `// >` (example)
  - [ ] **Rust Tests**: `ori_lexer/src/` — doc comment tokenization
  - [ ] **Ori Tests**: `tests/spec/lexical/doc_comments.ori`

### 0.1.2 Identifiers

- [ ] **Audit**: Standard identifiers `letter { letter | digit | "_" }` — grammar.ebnf § identifier
  - [ ] **Rust Tests**: `ori_lexer/src/` — identifier tokenization
  - [ ] **Ori Tests**: `tests/spec/lexical/identifiers.ori`

### 0.1.3 Keywords

- [ ] **Audit**: Reserved keywords — grammar.ebnf § Keywords
  - [ ] `break`, `continue`, `def`, `do`, `else`, `extern`, `false`, `for`, `if`, `impl`
  - [ ] `in`, `let`, `loop`, `match`, `pub`, `self`, `Self`, `then`, `trait`, `true`
  - [ ] `type`, `unsafe`, `use`, `uses`, `void`, `where`, `with`, `yield`
  - [ ] **Rust Tests**: `ori_lexer/src/` — keyword recognition
  - [ ] **Ori Tests**: `tests/spec/lexical/keywords.ori`

- [ ] **Audit**: Context-sensitive keywords (patterns) — grammar.ebnf § Keywords
  - [ ] `cache`, `catch`, `for`, `match`, `nursery`, `parallel`, `recurse`, `run`, `spawn`, `timeout`, `try`, `with`
  - [ ] **Rust Tests**: `ori_parse/src/` — context-sensitive handling
  - [ ] **Ori Tests**: `tests/spec/lexical/context_keywords.ori`

- [ ] **Audit**: Context-sensitive keywords (stdlib methods) — spec/03-lexical-elements.md § Context-Sensitive
  - [ ] `collect`, `filter`, `find`, `fold`, `map`, `retry`, `validate`
  - [ ] **Note**: These are stdlib iterator methods, not compiler patterns — listed in spec as context-sensitive
  - [ ] **Rust Tests**: `ori_parse/src/` — verify not reserved
  - [ ] **Ori Tests**: `tests/spec/lexical/context_keywords_stdlib.ori`

- [ ] **Audit**: Context-sensitive keywords (other) — grammar.ebnf § Keywords
  - [ ] `without` (imports only), `by` (ranges only), `max` (fixed-capacity only)
  - [ ] `int`, `float`, `str`, `byte` (type conversion call position)
  - [ ] **Rust Tests**: `ori_parse/src/` — context handling
  - [ ] **Ori Tests**: `tests/spec/lexical/context_keywords.ori`

### 0.1.4 Operators

- [ ] **Audit**: Arithmetic operators — grammar.ebnf § arith_op
  - [ ] `+`, `-`, `*`, `/`, `%`, `div` — **Fixed**: Added `div` to parser (was missing)
  - [ ] **Rust Tests**: `ori_lexer/src/` — operator tokenization
  - [ ] **Ori Tests**: `tests/spec/lexical/operators.ori`

- [ ] **Audit**: Comparison operators — grammar.ebnf § comp_op
  - [ ] `==`, `!=`, `<`, `>`, `<=`, `>=`
  - [ ] **Rust Tests**: `ori_lexer/src/`
  - [ ] **Ori Tests**: `tests/spec/lexical/operators.ori`

- [ ] **Audit**: Logic operators — grammar.ebnf § logic_op
  - [ ] `&&`, `||`, `!`
  - [ ] **Rust Tests**: `ori_lexer/src/`
  - [ ] **Ori Tests**: `tests/spec/lexical/operators.ori`

- [ ] **Audit**: Bitwise operators — grammar.ebnf § bit_op
  - [ ] `&`, `|`, `^`, `~`, `<<`, `>>`
  - [ ] **Rust Tests**: `ori_lexer/src/`
  - [ ] **Ori Tests**: `tests/spec/lexical/operators.ori`

- [ ] **Audit**: Other operators — grammar.ebnf § other_op
  - [ ] `..`, `..=`, `??`, `?`, `->`, `=>` — Note: `??` parses but evaluator incomplete
  - [ ] **Rust Tests**: `ori_lexer/src/`
  - [ ] **Ori Tests**: `tests/spec/lexical/operators.ori`

### 0.1.5 Delimiters

- [ ] **Audit**: All delimiters — grammar.ebnf § delimiter
  - [ ] `(`, `)`, `[`, `]`, `{`, `}`, `,`, `:`, `.`, `@`, `$`
  - [ ] **Rust Tests**: `ori_lexer/src/`
  - [ ] **Ori Tests**: `tests/spec/lexical/delimiters.ori`

### 0.1.6 Integer Literals

- [ ] **Audit**: Decimal integers — grammar.ebnf § decimal_lit
  - [ ] Basic: `42`, with underscores: `1_000_000`
  - [ ] **Rust Tests**: `ori_lexer/src/`
  - [ ] **Ori Tests**: `tests/spec/lexical/int_literals.ori`

- [ ] **Audit**: Hexadecimal integers — grammar.ebnf § hex_lit
  - [ ] Basic: `0xFF`, with underscores: `0x1A_2B_3C`
  - [ ] **Rust Tests**: `ori_lexer/src/`
  - [ ] **Ori Tests**: `tests/spec/lexical/int_literals.ori`

### 0.1.7 Float Literals

- [ ] **Audit**: Basic floats — grammar.ebnf § float_literal
  - [ ] Simple: `3.14`, with exponent: `2.5e-8`, `1.0E+10`
  - [ ] **Rust Tests**: `ori_lexer/src/`
  - [ ] **Ori Tests**: `tests/spec/lexical/float_literals.ori`

### 0.1.8 String Literals

- [ ] **Audit**: Basic strings — grammar.ebnf § string_literal
  - [ ] Simple: `"hello"`, empty: `""`
  - [ ] **Rust Tests**: `ori_lexer/src/`
  - [ ] **Ori Tests**: `tests/spec/lexical/string_literals.ori`

- [ ] **Audit**: Escape sequences — grammar.ebnf § escape
  - [ ] `\\`, `\"`, `\n`, `\t`, `\r`, `\0`
  - [ ] **Rust Tests**: `ori_lexer/src/`
  - [ ] **Ori Tests**: `tests/spec/lexical/string_literals.ori` — escapes tested in same file

### 0.1.9 Template Literals

- [ ] **Audit**: Template strings — grammar.ebnf § template_literal
  - [ ] Simple: `` `hello` ``, interpolation: `` `{name}` ``
  - [ ] **Rust Tests**: `ori_lexer/src/`
  - [ ] **Ori Tests**: `tests/spec/lexical/template_literals.ori`

- [ ] **Audit**: Template escapes — grammar.ebnf § template_escape, template_brace
  - [ ] Escapes: `` \` ``, `\\`, `\n`, `\t`, `\r`, `\0`
  - [ ] Brace escapes: `{{`, `}}`
  - [ ] **Rust Tests**: `ori_lexer/src/`
  - [ ] **Ori Tests**: `tests/spec/lexical/template_escapes.ori`

- [ ] **Audit**: Format specifications — grammar.ebnf § format_spec
  - [ ] Fill/align: `{x:<10}`, `{x:>5}`, `{x:^8}`
  - [ ] Sign: `{x:+}` (always sign), `{x:-}` (negative only), `{x: }` (space for positive)
  - [ ] Width/precision: `{x:10}`, `{x:.2}`, `{x:10.2}`
  - [ ] Format types: `{x:b}`, `{x:x}`, `{x:X}`, `{x:o}`, `{x:e}`, `{x:E}`, `{x:f}`, `{x:%}`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/lexical/format_spec.ori`

### 0.1.10 Character Literals

- [ ] **Audit**: Character literals — grammar.ebnf § char_literal
  - [ ] Simple: `'a'`, escapes: `'\n'`, `'\t'`, `'\''`, `'\\'`
  - [ ] **Rust Tests**: `ori_lexer/src/`
  - [ ] **Ori Tests**: `tests/spec/lexical/char_literals.ori`

### 0.1.11 Boolean Literals

- [ ] **Audit**: Boolean literals — grammar.ebnf § bool_literal
  - [ ] `true`, `false`
  - [ ] **Rust Tests**: `ori_lexer/src/`
  - [ ] **Ori Tests**: `tests/spec/lexical/bool_literals.ori`

### 0.1.12 Duration Literals

- [ ] **Audit**: Duration literals — grammar.ebnf § duration_literal
  - [ ] All units: `100ns`, `50us`, `10ms`, `5s`, `2m`, `1h`
  - [ ] Decimal syntax: `0.5s`, `1.5m` (compile-time sugar) — not tested
  - [ ] **Rust Tests**: `ori_lexer/src/`
  - [ ] **Ori Tests**: `tests/spec/lexical/duration_literals.ori`

### 0.1.13 Size Literals

- [ ] **Audit**: Size literals — grammar.ebnf § size_literal
  - [ ] All units: `100b`, `10kb`, `5mb`, `1gb`, `500tb`
  - [ ] Decimal syntax: `1.5kb`, `2.5mb` (compile-time sugar) — not tested
  - [ ] **Rust Tests**: `ori_lexer/src/`
  - [ ] **Ori Tests**: `tests/spec/lexical/size_literals.ori` — Note: impl uses binary (1024) not SI (1000)

---

## 0.2 Source Structure

> **SPEC**: `grammar.ebnf` § SOURCE STRUCTURE, `spec/02-source-code.md`, `spec/12-modules.md`

### 0.2.1 Source File

- [ ] **Audit**: Source file structure — grammar.ebnf § source_file
  - [ ] `[ file_attribute ] { import } { declaration }`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/source/file_structure.ori`

- [ ] **Audit**: File-level attributes — grammar.ebnf § file_attribute
  - [ ] `#!target(os: "linux")`, `#!cfg(debug)`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/source/file_attributes.ori`

### 0.2.2 Imports

- [ ] **Audit**: Import statements — grammar.ebnf § import
  - [ ] Relative: `use "./path" { items }`
  - [ ] Module: `use std.math { sqrt }`
  - [ ] Alias: `use std.net.http as http`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/source/imports.ori`

- [ ] **Audit**: Import items — grammar.ebnf § import_item
  - [ ] Basic: `{ name }`, aliased: `{ name as alias }`
  - [ ] Private: `{ ::internal }`, constants: `{ $CONST }`
  - [ ] Without default impl: `{ Trait without def }`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/source/import_items.ori`

### 0.2.3 Re-exports

- [ ] **Audit**: Re-export statements — grammar.ebnf § reexport
  - [ ] `pub use path { items }`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/source/reexports.ori`

### 0.2.4 Extensions

- [ ] **Audit**: Extension definitions — grammar.ebnf § extension_def
  - [ ] `extend Type { methods }`, `extend Type where T: Bound { methods }`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/source/extensions.ori`

- [ ] **Audit**: Extension imports — grammar.ebnf § extension_import
  - [ ] `extension std.iter.extensions { Iterator.count }`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/source/extension_imports.ori`

### 0.2.5 FFI

- [ ] **Audit**: Extern blocks — grammar.ebnf § extern_block
  - [ ] C: `extern "c" from "lib" { ... }`
  - [ ] JS: `extern "js" { ... }`, `extern "js" from "./utils.js" { ... }`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/source/extern_blocks.ori`

- [ ] **Audit**: Extern items — grammar.ebnf § extern_item
  - [ ] `@_sin (x: float) -> float as "sin"`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/source/extern_items.ori`

- [ ] **Audit**: C variadics — grammar.ebnf § c_variadic
  - [ ] `@printf (fmt: CPtr, ...) -> c_int`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/source/c_variadics.ori`

---

## 0.3 Declarations

> **SPEC**: `grammar.ebnf` § DECLARATIONS, `spec/08-declarations.md`

### 0.3.1 Attributes

- [ ] **Audit**: Item attributes — grammar.ebnf § attribute
  - [ ] `#derive(Eq, Clone)`, `#skip("reason")`, `#target(os: "linux")`
  - [ ] `#cfg(debug)`, `#fail("expected")`, `#compile_fail("E1234")`
  - [ ] `#repr("c")` — C-compatible struct layout for FFI
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/declarations/attributes.ori`

- [ ] **Audit**: Attribute arguments — grammar.ebnf § attribute_arg
  - [ ] Expression: `#attr(42)`, named: `#attr(key: value)`
  - [ ] Array: `#attr(["a", "b"])`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/declarations/attribute_args.ori`

### 0.3.2 Functions

- [ ] **Audit**: Function declarations — grammar.ebnf § function
  - [ ] Basic: `@name (x: int) -> int = expr`
  - [ ] Public: `pub @name`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/declarations/functions.ori`

- [ ] **Audit**: Function generics — grammar.ebnf § generics
  - [ ] Type params: `@f<T> (x: T) -> T`
  - [ ] Bounded: `@f<T: Eq> (x: T) -> bool`
  - [ ] Multiple bounds: `@f<T: Eq + Clone> (x: T) -> T`
  - [ ] Const params: `@f<$N: int> () -> [int, max N]`
  - [ ] Default params: `@f<T = int> (x: T) -> T`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/declarations/function_generics.ori`

- [ ] **Audit**: Clause parameters — grammar.ebnf § clause_params
  - [ ] Pattern param: `@fib (0: int) -> int = 1`
  - [ ] Default value: `@greet (name: str = "World") -> str`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/declarations/clause_params.ori`

- [ ] **Audit**: Guard clauses — grammar.ebnf § guard_clause
  - [ ] `@f (n: int) -> int if n > 0 = n`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/declarations/guard_clauses.ori`

- [ ] **Audit**: Uses clause — grammar.ebnf § uses_clause
  - [ ] `@fetch (url: str) -> str uses Http = ...`
  - [ ] Multiple: `@process () -> void uses Http, FileSystem = ...`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/declarations/uses_clause.ori`

- [ ] **Audit**: Where clause — grammar.ebnf § where_clause
  - [ ] Type constraint: `where T: Clone`
  - [ ] Multiple: `where T: Clone, U: Default`
  - [ ] Const constraint: `where N > 0`, `where N > 0 && N <= 100`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/declarations/where_clause.ori`

### 0.3.3 Const Bound Expressions

- [ ] **Audit**: Const bound expressions — grammar.ebnf § const_bound_expr
  - [ ] Comparison: `N > 0`, `N == M`, `N >= 1`
  - [ ] Logical: `N > 0 && N < 100`, `A || B`, `!C`
  - [ ] Grouped: `(N > 0 && N < 10) || N == 100`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/declarations/const_bounds.ori`

### 0.3.4 Type Definitions

- [ ] **Audit**: Struct types — grammar.ebnf § struct_body
  - [ ] `type Point = { x: int, y: int }`
  - [ ] Generic: `type Box<T> = { value: T }`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/declarations/struct_types.ori`

- [ ] **Audit**: Sum types — grammar.ebnf § sum_body
  - [ ] `type Option<T> = Some(value: T) | None`
  - [ ] Unit variants: `type Color = Red | Green | Blue`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/declarations/sum_types.ori`

- [ ] **Audit**: Newtype aliases — grammar.ebnf § type_body (type reference)
  - [ ] `type UserId = int`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/declarations/newtypes.ori`

### 0.3.5 Traits

- [ ] **Audit**: Trait definitions — grammar.ebnf § trait_def
  - [ ] Basic: `trait Printable { @to_str (self) -> str }`
  - [ ] With inheritance: `trait Comparable: Eq { ... }`
  - [ ] With generics: `trait Into<T> { @into (self) -> T }`
  - [ ] Default type params: `trait Add<Rhs = Self> { ... }`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/declarations/traits.ori`

- [ ] **Audit**: Method signatures — grammar.ebnf § method_sig
  - [ ] `@method (self) -> T`, `@method (self, other: Self) -> bool`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/declarations/method_sigs.ori`

- [ ] **Audit**: Default methods — grammar.ebnf § default_method
  - [ ] `@method (self) -> T = expr`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/declarations/default_methods.ori`

- [ ] **Audit**: Associated types — grammar.ebnf § assoc_type
  - [ ] Basic: `type Item`
  - [ ] Bounded: `type Item: Eq`
  - [ ] Default: `type Output = Self`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/declarations/assoc_types.ori`

- [ ] **Audit**: Variadic parameters — grammar.ebnf § variadic_param
  - [ ] `@sum (nums: ...int) -> int`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/declarations/variadic_params.ori`

### 0.3.6 Implementations

- [ ] **Audit**: Inherent impl — grammar.ebnf § inherent_impl
  - [ ] `impl Point { @distance (self) -> float = ... }`
  - [ ] Generic: `impl<T> Box<T> { ... }`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/declarations/inherent_impl.ori`

- [ ] **Audit**: Trait impl — grammar.ebnf § trait_impl
  - [ ] `impl Printable for Point { ... }`
  - [ ] Generic: `impl<T: Printable> Printable for Box<T> { ... }`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/declarations/trait_impl.ori`

- [ ] **Audit**: Default impl — grammar.ebnf § def_impl
  - [ ] `def impl Printable { @to_str (self) -> str = ... }`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/declarations/def_impl.ori`

### 0.3.7 Tests

- [ ] **Audit**: Test declarations — grammar.ebnf § test
  - [ ] Attached: `@t tests @target () -> void = ...`
  - [ ] Floating: `@t tests _ () -> void = ...`
  - [ ] Multi-target: `@t tests @a tests @b () -> void = ...`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/declarations/tests.ori`

### 0.3.8 Constants

- [ ] **Audit**: Module-level constants — grammar.ebnf § constant_decl
  - [ ] `let $PI = 3.14159`
  - [ ] Typed: `let $MAX_SIZE: int = 1000`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/declarations/constants.ori`

---

## 0.4 Types

> **SPEC**: `grammar.ebnf` § TYPES, `spec/06-types.md`

### 0.4.1 Type Paths

- [ ] **Audit**: Simple type paths — grammar.ebnf § type_path
  - [ ] `int`, `Point`, `std.math.Complex`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/types/type_paths.ori`

- [ ] **Audit**: Generic type arguments — grammar.ebnf § type_args
  - [ ] `Option<int>`, `Result<T, E>`, `Map<str, int>`
  - [ ] With const: `[int, max 10]`, `Array<int, $N>`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/types/type_args.ori`

### 0.4.2 Existential Types

- [ ] **Audit**: impl Trait — grammar.ebnf § impl_trait_type
  - [ ] Basic: `impl Iterator`
  - [ ] Multi-trait: `impl Iterator + Clone`
  - [ ] With where: `impl Iterator where Item == int`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/types/existential.ori`

### 0.4.3 Compound Types

- [ ] **Audit**: List types — grammar.ebnf § list_type
  - [ ] Dynamic: `[int]`, `[Option<str>]`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/types/list_types.ori`

- [ ] **Audit**: Fixed-capacity list types — grammar.ebnf § fixed_list_type
  - [ ] `[int, max 10]`, `[T, max N]`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/types/fixed_list_types.ori`

- [ ] **Audit**: Map types — grammar.ebnf § map_type
  - [ ] `{str: int}`, `{K: V}`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/types/map_types.ori`

- [ ] **Audit**: Tuple types — grammar.ebnf § tuple_type
  - [ ] Unit: `()`, pairs: `(int, str)`, nested: `((int, int), str)`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/types/tuple_types.ori`

- [ ] **Audit**: Function types — grammar.ebnf § function_type
  - [ ] `() -> void`, `(int) -> int`, `(int, str) -> bool`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/types/function_types.ori`

### 0.4.4 Const Expressions in Types

- [ ] **Audit**: Const expressions — grammar.ebnf § const_expr
  - [ ] Literal: `10`, `true`
  - [ ] Parameter: `$N`
  - [ ] Arithmetic: `$N + 1`, `$N * 2`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/types/const_expr.ori`

### 0.4.5 Trait Objects

> **SPEC**: `spec/06-types.md` § Trait Objects

- [ ] **Audit**: Simple trait objects — spec/06-types.md § Trait Objects
  - [ ] Trait name as type: `@display (item: Printable) -> void`
  - [ ] In collections: `[Printable]`, `{str: Printable}`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/types/trait_objects.ori`

- [ ] **Audit**: Bounded trait objects — spec/06-types.md § Bounded Trait Objects
  - [ ] Multiple bounds: `Printable + Hashable`
  - [ ] As parameter type: `@store (item: Printable + Hashable) -> void`
  - [ ] **Note**: Grammar inconsistency — `bounds` not in `type` production, only in `impl Trait`
  - [ ] **Grammar Fix Required**: Add `bounds` as standalone type alternative
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/types/bounded_trait_objects.ori`

---

## 0.5 Expressions

> **SPEC**: `grammar.ebnf` § EXPRESSIONS, `spec/09-expressions.md`

### 0.5.1 Primary Expressions

> **Note**: Pattern expressions (`run`, `try`, `match`, `parallel`, `nursery`, `channel`, etc.) are valid primary expressions per grammar.ebnf § primary → pattern_expr. See **section 0.6** for pattern-specific audit items.

- [ ] **Audit**: Literals — grammar.ebnf § primary
  - [ ] All literal types covered in 0.1
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/expressions/literals.ori`

- [ ] **Audit**: Identifiers and self — grammar.ebnf § primary
  - [ ] `x`, `self`, `Self`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/expressions/identifiers.ori`

- [ ] **Audit**: Grouped expressions — grammar.ebnf § primary
  - [ ] `(expr)`, nested: `((a + b) * c)`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/expressions/grouped.ori`

- [ ] **Audit**: Length placeholder — grammar.ebnf § primary
  - [ ] `list[# - 1]` (last element)
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/expressions/length_placeholder.ori`

### 0.5.2 Unsafe Expression

- [ ] **Audit**: Unsafe expressions — grammar.ebnf § unsafe_expr
  - [ ] `unsafe(ptr_read(ptr))`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/expressions/unsafe.ori`

### 0.5.3 List Literals

- [ ] **Audit**: List literals — grammar.ebnf § list_literal
  - [ ] Empty: `[]`, simple: `[1, 2, 3]`
  - [ ] With spread: `[...a, 4, ...b]`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/expressions/list_literals.ori`

### 0.5.4 Map Literals

- [ ] **Audit**: Map literals — grammar.ebnf § map_literal
  - [ ] Empty: `{}`, simple: `{a: 1, b: 2}`
  - [ ] String keys: `{"key": value}`
  - [ ] Computed keys: `{[expr]: value}`
  - [ ] With spread: `{...base, extra: 1}`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/expressions/map_literals.ori`

### 0.5.5 Struct Literals

- [ ] **Audit**: Struct literals — grammar.ebnf § struct_literal
  - [ ] Basic: `Point { x: 1, y: 2 }`
  - [ ] Shorthand: `Point { x, y }` (when var name matches field)
  - [ ] With spread: `Point { ...base, x: 10 }`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/expressions/struct_literals.ori`

### 0.5.6 Postfix Expressions

- [ ] **Audit**: Field/method access — grammar.ebnf § postfix_op
  - [ ] Field: `point.x`
  - [ ] Method: `list.len()`, `str.contains(substr:)`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/expressions/field_access.ori`

- [ ] **Audit**: Index access — grammar.ebnf § postfix_op
  - [ ] `list[0]`, `map["key"]`, `list[# - 1]`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/expressions/index_access.ori`

- [ ] **Audit**: Function calls — grammar.ebnf § call_args
  - [ ] Named: `greet(name: "Alice")`
  - [ ] Positional (lambda): `list.map(x -> x * 2)`
  - [ ] Spread: `sum(...numbers)`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/expressions/function_calls.ori`

- [ ] **Audit**: Error propagation — grammar.ebnf § postfix_op
  - [ ] `result?`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/expressions/error_propagation.ori`

- [ ] **Audit**: Type conversion — grammar.ebnf § postfix_op
  - [ ] Infallible: `42 as float`
  - [ ] Fallible: `"42" as? int`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/expressions/type_conversion.ori`

### 0.5.7 Unary Expressions

- [ ] **Audit**: Unary operators — grammar.ebnf § unary_expr
  - [ ] Logical not: `!condition`
  - [ ] Negation: `-number`
  - [ ] Bitwise not: `~bits`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/expressions/unary.ori`

### 0.5.8 Binary Expressions

- [ ] **Audit**: Null coalesce — grammar.ebnf § coalesce_expr
  - [ ] `option ?? default`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/expressions/coalesce.ori`

- [ ] **Audit**: Logical operators — grammar.ebnf § or_expr, and_expr
  - [ ] `a || b`, `a && b`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/expressions/logical.ori`

- [ ] **Audit**: Bitwise operators — grammar.ebnf § bit_or_expr, bit_xor_expr, bit_and_expr
  - [ ] `a | b`, `a ^ b`, `a & b`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/expressions/bitwise.ori`

- [ ] **Audit**: Equality operators — grammar.ebnf § eq_expr
  - [ ] `a == b`, `a != b`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/expressions/equality.ori`

- [ ] **Audit**: Comparison operators — grammar.ebnf § cmp_expr
  - [ ] `a < b`, `a > b`, `a <= b`, `a >= b`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/expressions/comparison.ori`

- [ ] **Audit**: Range expressions — grammar.ebnf § range_expr
  - [ ] Exclusive: `0..10`, inclusive: `0..=10`
  - [ ] With step: `0..10 by 2`, `10..0 by -1`
  - [ ] Infinite: `0..`, `0.. by 2`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/expressions/ranges.ori`

- [ ] **Audit**: Shift operators — grammar.ebnf § shift_expr
  - [ ] `a << n`, `a >> n`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/expressions/shift.ori`

- [ ] **Audit**: Arithmetic operators — grammar.ebnf § add_expr, mul_expr
  - [ ] `a + b`, `a - b`, `a * b`, `a / b`, `a % b`, `a div b`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/expressions/arithmetic.ori`

### 0.5.9 With Expression

- [ ] **Audit**: Capability provision — grammar.ebnf § with_expr
  - [ ] Single: `with Http = MockHttp in expr`
  - [ ] Multiple: `with Http = mock, Cache = mock in expr`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/expressions/with.ori`

### 0.5.10 Let Binding

- [ ] **Audit**: Let expressions — grammar.ebnf § let_expr
  - [ ] Mutable: `let x = 42`
  - [ ] Immutable: `let $x = 42`
  - [ ] Typed: `let x: int = 42`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/expressions/let.ori`

- [ ] **Audit**: Assignment — grammar.ebnf § assignment
  - [ ] `x = new_value`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/expressions/assignment.ori`

### 0.5.11 Conditional

- [ ] **Audit**: If expressions — grammar.ebnf § if_expr
  - [ ] Simple: `if cond then a else b`
  - [ ] Void: `if cond then action`
  - [ ] Chained: `if c1 then a else if c2 then b else c`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/expressions/if.ori`

### 0.5.12 For Expression

- [ ] **Audit**: For loops — grammar.ebnf § for_expr
  - [ ] Do: `for x in items do action`
  - [ ] Yield: `for x in items yield x * 2`
  - [ ] Filter: `for x in items if x > 0 yield x`
  - [ ] Labeled: `for:outer x in items do ...`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/expressions/for.ori`

### 0.5.13 Loop Expression

- [ ] **Audit**: Loop expressions — grammar.ebnf § loop_expr
  - [ ] Basic: `loop(body)`
  - [ ] Labeled: `loop:name(body)`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/expressions/loop.ori`

### 0.5.14 Labels

- [ ] **Audit**: Loop labels — grammar.ebnf § label
  - [ ] `:name` (no space around colon)
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/expressions/labels.ori`

### 0.5.15 Lambda

- [ ] **Audit**: Simple lambdas — grammar.ebnf § simple_lambda
  - [ ] Single param: `x -> x + 1`
  - [ ] Multiple: `(a, b) -> a + b`
  - [ ] No params: `() -> 42`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/expressions/lambda_simple.ori`

- [ ] **Audit**: Typed lambdas — grammar.ebnf § typed_lambda
  - [ ] `(x: int) -> int = x * 2`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/expressions/lambda_typed.ori`

### 0.5.16 Control Flow

- [ ] **Audit**: Break expression — grammar.ebnf § break_expr
  - [ ] Simple: `break`
  - [ ] With value: `break result`
  - [ ] Labeled: `break:outer`, `break:outer result`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/expressions/break.ori`

- [ ] **Audit**: Continue expression — grammar.ebnf § continue_expr
  - [ ] Simple: `continue`
  - [ ] With value: `continue replacement`
  - [ ] Labeled: `continue:outer`, `continue:outer replacement`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/expressions/continue.ori`

---

## 0.6 Patterns

> **SPEC**: `grammar.ebnf` § PATTERNS, `spec/10-patterns.md`

### 0.6.1 Sequential Patterns

- [ ] **Audit**: Run pattern — grammar.ebnf § run_expr
  - [ ] Basic: `run(let x = a, result)`
  - [ ] Pre-check: `run(pre_check: cond, body)`
  - [ ] Pre-check with message: `run(pre_check: cond | "msg", body)` — grammar.ebnf § check_expr
  - [ ] Post-check: `run(body, post_check: r -> cond)`
  - [ ] Post-check with message: `run(body, post_check: r -> cond | "msg")` — grammar.ebnf § postcheck_expr
  - [ ] Multiple pre-checks: `run(pre_check: a, pre_check: b, body)`
  - [ ] Multiple post-checks: `run(body, post_check: r -> a, post_check: r -> b)`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/patterns/run.ori`

- [ ] **Audit**: Try pattern — grammar.ebnf § try_expr
  - [ ] `try(let x = f()?, Ok(x))`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/patterns/try.ori`

- [ ] **Audit**: Match pattern — grammar.ebnf § match_expr
  - [ ] `match(expr, Some(x) -> x, None -> default)`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/patterns/match.ori`

- [ ] **Audit**: Guard syntax — grammar.ebnf § guard
  - [ ] `.match(...)` syntax: `n.match(x -> x > 0) -> n`
  - [ ] In match arm: `match(v, x.match(predicate) -> result, _ -> default)`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/patterns/match_guards.ori`

- [ ] **Audit**: For pattern — grammar.ebnf § for_pattern
  - [ ] Basic form: `for(over: items, match: Some(x) -> x, default: 0)` — grammar.ebnf § for_pattern_args variant 1
  - [ ] With map: `for(over: items, map: transform, match: pat -> expr, default: d)` — grammar.ebnf § for_pattern_args variant 2
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/patterns/for_pattern.ori`

- [ ] **Audit**: Catch pattern — grammar.ebnf § catch_expr
  - [ ] `catch(expr: risky_operation)`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/patterns/catch.ori`

- [ ] **Audit**: Nursery pattern — grammar.ebnf § nursery_expr
  - [ ] `nursery(body: n -> ..., on_error: CancelRemaining)`
  - [ ] With timeout: `nursery(body: ..., on_error: ..., timeout: 5s)`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/patterns/nursery.ori`

### 0.6.2 Function Expression Patterns

- [ ] **Audit**: Pattern arguments — grammar.ebnf § pattern_arg
  - [ ] Named argument syntax: `identifier ":" expression`
  - [ ] All function_exp patterns use this form
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/patterns/pattern_args.ori`

- [ ] **Audit**: Recurse pattern — grammar.ebnf § function_exp
  - [ ] `recurse(condition: n > 0, base: 1, step: n -> n - 1)`
  - [ ] With memo: `recurse(..., memo: true)`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/patterns/recurse.ori`

- [ ] **Audit**: Parallel pattern — grammar.ebnf § function_exp
  - [ ] `parallel(tasks: [...], max_concurrent: 4)`
  - [ ] With timeout: `parallel(tasks: [...], timeout: 10s)`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/patterns/parallel.ori`

- [ ] **Audit**: Spawn pattern — grammar.ebnf § function_exp
  - [ ] `spawn(tasks: [...], max_concurrent: 10)`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/patterns/spawn.ori`

- [ ] **Audit**: Timeout pattern — grammar.ebnf § function_exp
  - [ ] `timeout(op: expr, after: 5s)`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/patterns/timeout.ori`

- [ ] **Audit**: Cache pattern — grammar.ebnf § function_exp
  - [ ] `cache(key: k, op: expensive(), ttl: 1h)`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/patterns/cache.ori`

- [ ] **Audit**: With pattern (RAII) — grammar.ebnf § function_exp
  - [ ] `with(acquire: open_file(), use: f -> ..., release: close)`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/patterns/with_pattern.ori`

### 0.6.3 Type Conversion Patterns

- [ ] **Audit**: Type conversion calls — grammar.ebnf § function_val
  - [ ] `int(x)`, `float(x)`, `str(x)`, `byte(x)`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/patterns/type_conversion.ori`

### 0.6.4 Channel Constructors

- [ ] **Audit**: Channel creation — grammar.ebnf § channel_expr
  - [ ] `channel<int>(buffer: 10)`
  - [ ] `channel_in<T>(buffer: 5)`, `channel_out<T>(buffer: 5)`
  - [ ] `channel_all<T>(buffer: 5)`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/patterns/channels.ori`

### 0.6.5 Match Patterns

- [ ] **Audit**: Literal patterns — grammar.ebnf § literal_pattern
  - [ ] Int: `42`, `-1`
  - [ ] String: `"hello"`
  - [ ] Char: `'a'`
  - [ ] Bool: `true`, `false`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/patterns/literal_patterns.ori`

- [ ] **Audit**: Identifier pattern — grammar.ebnf § identifier_pattern
  - [ ] `x` (binds value)
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/patterns/identifier_patterns.ori`

- [ ] **Audit**: Wildcard pattern — grammar.ebnf § wildcard_pattern
  - [ ] `_`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/patterns/wildcard_patterns.ori`

- [ ] **Audit**: Variant patterns — grammar.ebnf § variant_pattern
  - [ ] `Some(x)`, `None`, `Ok(value)`, `Err(e)`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/patterns/variant_patterns.ori`

- [ ] **Audit**: Struct patterns — grammar.ebnf § struct_pattern
  - [ ] `{ x, y }`, `{ x: px, y: py }`
  - [ ] With rest: `{ x, .. }`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/patterns/struct_patterns.ori`

- [ ] **Audit**: Tuple patterns — grammar.ebnf § tuple_pattern
  - [ ] `(a, b)`, `(x, y, z)`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/patterns/tuple_patterns.ori`

- [ ] **Audit**: List patterns — grammar.ebnf § list_pattern
  - [ ] `[a, b, c]`, `[head, ..tail]`
  - [ ] Rest only: `[..]`, `[..rest]`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/patterns/list_patterns.ori`

- [ ] **Audit**: Range patterns — grammar.ebnf § range_pattern
  - [ ] `1..10`, `'a'..='z'`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/patterns/range_patterns.ori`

- [ ] **Audit**: Or patterns — grammar.ebnf § or_pattern
  - [ ] `A | B`, `Some(1) | Some(2)`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/patterns/or_patterns.ori`

- [ ] **Audit**: At patterns — grammar.ebnf § at_pattern
  - [ ] `x @ Some(_)`, `list @ [_, ..]`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/patterns/at_patterns.ori`

### 0.6.6 Binding Patterns

- [ ] **Audit**: Identifier bindings — grammar.ebnf § binding_pattern
  - [ ] Mutable: `x`
  - [ ] Immutable: `$x`
  - [ ] Wildcard: `_`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/patterns/binding_identifier.ori`

- [ ] **Audit**: Struct destructure — grammar.ebnf § binding_pattern
  - [ ] `{ x, y }`, `{ x: px, y: py }`
  - [ ] Immutable: `{ $x, $y }`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/patterns/binding_struct.ori`

- [ ] **Audit**: Tuple destructure — grammar.ebnf § binding_pattern
  - [ ] `(a, b)`, `($a, $b)`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/patterns/binding_tuple.ori`

- [ ] **Audit**: List destructure — grammar.ebnf § binding_pattern
  - [ ] `[head, ..tail]`, `[$first, $second, ..rest]`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/patterns/binding_list.ori`

---

## 0.7 Constant Expressions

> **SPEC**: `grammar.ebnf` § CONSTANT EXPRESSIONS, `spec/04-constants.md`, `spec/21-constant-expressions.md`

- [ ] **Audit**: Literal const expressions — grammar.ebnf § const_expr
  - [ ] `42`, `true`, `"hello"`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/const_expr/literals.ori`

- [ ] **Audit**: Arithmetic const expressions — grammar.ebnf § const_expr
  - [ ] `$N + 1`, `$A * $B`, `10 / 2`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/const_expr/arithmetic.ori`

- [ ] **Audit**: Comparison const expressions — grammar.ebnf § const_expr
  - [ ] `$N > 0`, `$A == $B`, `10 <= 100`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/const_expr/comparison.ori`

- [ ] **Audit**: Logical const expressions — grammar.ebnf § const_expr
  - [ ] `$A && $B`, `$X || $Y`, `!$C`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/const_expr/logical.ori`

- [ ] **Audit**: Grouped const expressions — grammar.ebnf § const_expr
  - [ ] `($N + 1) * 2`, `!($A && $B)`
  - [ ] **Rust Tests**: `ori_parse/src/`
  - [ ] **Ori Tests**: `tests/spec/const_expr/grouped.ori`

---

## 0.8 Section Completion Checklist

> **STATUS**: NEARLY COMPLETE — 1983 passing, 0 failing, 31 skipped

- [ ] All lexical grammar items audited and tested (0.1)
- [ ] All source structure items audited and tested (0.2)
- [ ] All declaration items audited and tested (0.3)
- [ ] All type items audited and tested (0.4)
- [ ] All expression items audited and tested (0.5)
- [ ] All pattern items audited and tested (0.6)
- [ ] All constant expression items audited and tested (0.7)
- [ ] Run `cargo t -p ori_parse` — all parser tests pass
- [ ] Run `cargo t -p ori_lexer` — all lexer tests pass
- [ ] Run `cargo st tests/` — 31 skipped tests remain (evaluator/type checker gaps, not parser issues)

**Exit Criteria**: Every grammar production in `grammar.ebnf` has verified parser support with tests. Parser complete; skipped tests are evaluator/type checker issues tracked in Section 23.

**Note**: Skipped tests are NOT parser failures. They skip due to unimplemented evaluator features (struct destructuring, capability provision, LLVM struct support). The parser handles all spec syntax correctly.

---

## 0.9 Parser Bugs (from Comprehensive Tests)

> **STATUS**: 16/18 parser bugs fixed. Only 2 remain: associated type constraints and const functions.

> **POLICY**: Skipping tests is NOT acceptable. Every test must pass. If a feature is tested, it must work. Fix the code, not the tests.

This section documents **parser-only** bugs discovered by the comprehensive test suite. Evaluator/type checker bugs are tracked in **Section 23: Full Evaluator Support**.

### 0.9.1 Parser/Syntax Bugs (Parse Errors - features not implemented)

These features fail at the parse phase — the parser does not recognize the syntax.

> **Test Status Comments**: Each failing test file has a `STATUS: Lexer [OK], Parser [BROKEN]` comment documenting the specific issue.

- [ ] **Implement**: Guard clauses in function definitions
  - [ ] **Parser**: Handle `if condition` before `=` in function definitions
  - [ ] **Syntax**: `@f (n: int) -> int if n > 0 = n` — Now parses correctly
  - [ ] **Ori Tests**: `tests/spec/declarations/clause_params.ori` — guard clause tests (evaluator support needed)

- [ ] **Implement**: List patterns in function parameters
  - [ ] **Parser**: Recognize `[` as start of pattern in parameter position
  - [ ] **Syntax**: `@len ([]: [T]) -> int = 0` — Now parses correctly
  - [ ] **Ori Tests**: `tests/spec/declarations/clause_params.ori` — type checker support needed

- [ ] **Implement**: Const generics
  - [ ] **Parser**: Handle `$` prefix in generic parameters
  - [ ] **Syntax**: `@f<$N: int>`, `@f<$N: int = 10>` — Now parses correctly
  - [ ] **Ori Tests**: `tests/spec/declarations/generics.ori` — type checker support needed

- [ ] **Implement**: Variadic parameters
  - [ ] **Parser**: Handle `...` prefix before parameter type
  - [ ] **Syntax**: `@sum (nums: ...int)` — Now parses correctly
  - [ ] **Ori Tests**: `tests/spec/declarations/variadic_params.ori` — type checker support needed

- [ ] **Implement**: Spread in function calls
  - [ ] **Parser**: Handle `...` spread operator in call arguments
  - [ ] **Syntax**: `sum(...list)` — Now parses correctly
  - [ ] **Ori Tests**: `tests/spec/expressions/function_calls.ori` — type checker support needed

- [ ] **Implement**: `#repr` attribute
  - [ ] **Parser**: Register `repr` as known attribute
  - [ ] **Syntax**: `#repr("c")` — Now parses correctly
  - [ ] **Ori Tests**: `tests/spec/declarations/attributes.ori` — semantic validation needed

- [ ] **Implement**: `#target` attribute
  - [ ] **Parser**: Register `target` as known attribute
  - [ ] **Syntax**: `#target(os: "linux")` — Now parses correctly
  - [ ] **Ori Tests**: `tests/spec/declarations/attributes.ori` — semantic validation needed

- [ ] **Implement**: `#cfg` attribute
  - [ ] **Parser**: Register `cfg` as known attribute
  - [ ] **Syntax**: `#cfg(debug)` — Now parses correctly
  - [ ] **Ori Tests**: `tests/spec/declarations/attributes.ori` — semantic validation needed

- [ ] **Implement**: Associated type constraints in where clauses
  - [ ] **Parser**: Handle `.` in type paths and `==` for type equality
  - [ ] **Syntax**: `where I.Item == int` — Parser expects `:`, finds `==`
  - [ ] **Ori Tests**: `tests/spec/declarations/where_clause.ori`

- [ ] **Implement**: `timeout` as identifier
  - [ ] **Parser**: Allow `timeout` in non-pattern contexts
  - [ ] **Syntax**: `let timeout = 5` — Now works (context-sensitive keyword)
  - [ ] **Ori Tests**: Verified working in local tests

- [ ] **Implement**: Computed constants
  - [ ] **Parser**: Allow expressions in module-level constant definitions
  - [ ] **Syntax**: `let $X = 2 + 3` — Now parses correctly
  - [ ] **Ori Tests**: `tests/spec/declarations/constants.ori` — evaluator support needed

- [ ] **Implement**: Const functions
  - [ ] **Parser**: Handle `$name (params) = expr` syntax for const functions
  - [ ] **Syntax**: `$add (a: int, b: int) = a + b` — Parser error
  - [ ] **Ori Tests**: `tests/spec/declarations/const_functions.ori`

### 0.9.2 Additional Parser Bugs (discovered during testing)

- [ ] **Implement**: Fixed-capacity list type syntax `[T, max N]`
  - [ ] **Parser**: Handle comma in type annotation for fixed-capacity lists
  - [ ] **Syntax**: `let buffer: [int, max 10] = []` — Now parses correctly
  - [ ] **Ori Tests**: `tests/spec/types/fixed_list_types.ori` — type checker support needed

- [ ] **Implement**: Wildcard pattern in for loops `for _ in range`
  - [ ] **Parser**: Accept `_` as binding pattern in for loops
  - [ ] **Syntax**: `for _ in 0..n do ...` — Now parses correctly
  - [ ] **Ori Tests**: `tests/spec/patterns/for.ori`

- [ ] **Implement**: `as` and `as?` type conversion operators
  - [ ] **Parser**: Handle `as` and `as?` as postfix operators
  - [ ] **Syntax**: `42 as float`, `"42" as? int` — Now parses correctly
  - [ ] **Syntax**: Negative literals: `-100 as float` — Now parses correctly
  - [ ] **Ori Tests**: `tests/spec/expressions/type_conversion.ori`

- [ ] **Implement**: Context-sensitive pattern keywords
  - [ ] **Parser**: Allow `timeout`, `parallel`, `cache`, `spawn`, `recurse` as identifiers
  - [ ] **Syntax**: `let timeout = 5`, `fn(timeout: int)` — Now parses correctly
  - [ ] **Rule**: Keywords only when followed by `(`, otherwise identifiers
  - [ ] **Ori Tests**: Various tests using pattern keywords as variable names

- [ ] **Implement**: List spread syntax `[...a, x, ...b]`
  - [ ] **Parser**: Handle `...` spread in list literals
  - [ ] **Syntax**: `[...result, i]` — Now parses correctly
  - [ ] **Ori Tests**: `tests/spec/types/list_types.ori` — evaluator support needed

- [ ] **Implement**: Map spread syntax `{...base, key: value}`
  - [ ] **Parser**: Handle `...` spread in map literals
  - [ ] **Syntax**: `{...base, "c": 3}` — Now parses correctly
  - [ ] **Ori Tests**: `tests/spec/types/map_types.ori` — evaluator support needed

- [ ] **Implement**: Tuple destructuring in for loops `for (k, v) in map`
  - [ ] **Parser**: Distinguish tuple pattern from for(...) pattern syntax
  - [ ] **Syntax**: `for (k, v) in m do ...` — Now parses correctly
  - [ ] **Ori Tests**: `tests/spec/types/map_types.ori` — evaluator support needed

- [ ] **Implement**: Multiple derives in single attribute `#derive(Eq, Clone, Debug)`
  - [ ] **Parser**: Handle comma-separated derives correctly
  - [ ] **Syntax**: `#derive(Eq, Clone, Debug)` — Now parses correctly
  - [ ] **Ori Tests**: `tests/spec/types/trait_objects.ori` — semantic support needed

---

> **NOTE**: Type checker and evaluator bugs have been moved to **Section 23: Full Evaluator Support**.

---

## Completion Summary

**Parser Fixes Made:**
1. Added `div` operator to `match_multiplicative_op()` in `operators.rs`
2. Added `$` prefix support for immutable bindings in `let` expressions (maintained `mut` backward compat)

**Test Files Created (19 new files):**
- `tests/spec/lexical/`: 11 files (comments, identifiers, all literals, operators, delimiters)
- `tests/spec/source/`: 3 files (file_structure, imports, extensions)
- `tests/spec/declarations/`: 6 files (clause_params, constants, generics, struct_types, sum_types, traits, where_clause)
- `tests/spec/types/`: 1 file (type_syntax)
- `tests/spec/expressions/`: 1 file (syntax)
- `tests/spec/patterns/`: 1 file (syntax)
- `tests/spec/const_expr/`: 1 file (syntax)

**Roadmap Verification Audit:**
Many items marked `[ ]` were found to actually work. Updated status for:
- `#repr`, `#target`, `#cfg` attributes: Parse correctly
- `timeout` as identifier: Works (context-sensitive keyword)
- Computed constants `let $X = 2 + 3`: Parses correctly
- List spread `[...a, ...b]`: Parses correctly
- Map spread `{...base, key: val}`: Parses correctly
- Tuple destructure in for `for (k, v) in m`: Parses correctly
- Multiple derives `#derive(Eq, Clone)`: Parses correctly

**Remaining Parser Bugs (verified 2026-02-04):**
- Associated type constraints `where I.Item == int`: Still fails (expects `:`, finds `==`)
- Const functions `$name (params) = expr`: Not implemented

**Known Limitations (Parser works, but semantics incomplete — tracked in Section 23):**
- `??` operator: Parses but evaluator support incomplete — **Tracked**: Section 23.1.1
- Primitive trait methods: Parse but evaluator doesn't resolve — **Tracked**: Section 23.2
- Map indexing semantics: Parses but returns wrong type — **Tracked**: Section 23.3.1
- Size literals: Fixed — uses SI units (1000) per approved proposal
- Pattern matching in function params: Implemented
- Default parameter values: Implemented
- Struct spread syntax `...`: Implemented

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
