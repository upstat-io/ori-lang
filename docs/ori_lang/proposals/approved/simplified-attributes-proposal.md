# Proposal: Simplified Attribute Syntax

**Status:** Approved
**Approved:** 2026-01-28
**Author:** Eric
**Created:** 2026-01-22

---

## Summary

Simplify attribute syntax from `#[name(...)]` to `#name(...)` by removing the brackets. This reduces visual noise while maintaining clear attribute identification.

```ori
// Before (Rust-style)
#[derive(Eq, Clone)]
pub type Point = { x: int, y: int }

#[skip("waiting on parser fix")]
@test_point tests @point () -> void = ...

// After (simplified)
#derive(Eq, Clone)
pub type Point = { x: int, y: int }

#skip("waiting on parser fix")
@test_point tests @point () -> void = ...
```

---

## Motivation

### The Problem

Ori currently uses Rust's attribute syntax `#[name(...)]`. While functional, this syntax has issues:

1. **Visual noise** — The brackets add clutter without semantic value
2. **Cryptic appearance** — `#[` looks like line noise to newcomers
3. **Inconsistent with Ori's philosophy** — Other oris (`@`, `$`) don't require brackets

### Why Attributes Don't Need Ori-Style Discoverability

Ori uses oris (`@`, `$`) to enable fast discovery:

| Ori | Purpose | Discovery Use Case |
|-------|---------|-------------------|
| `@` | Functions | "Find where `@calculate` is defined" |
| `$` | Config | "Find where `$timeout` is set" |
| `#` | Attributes | ??? |

**Key insight:** You rarely search for attributes. You don't grep "show me everything with `#derive`" — you look at a type and see what traits it derives. Attributes are metadata read in context, not searched for.

Since discoverability isn't a concern, attributes don't need bracket delimiters for disambiguation. A simple `#` prefix is sufficient.

### Prior Art

| Language | Syntax | Notes |
|----------|--------|-------|
| Rust | `#[name(...)]` | Brackets required |
| Python | `@name(...)` | No brackets |
| Java | `@Name(...)` | No brackets |
| C# | `[Name(...)]` | Brackets only |
| C++ | `[[name(...)]]` | Double brackets |

Most languages with similar concepts don't require nested brackets.

---

## Design

### Syntax

```ebnf
attribute      = "#" identifier [ "(" [ attribute_arg { "," attribute_arg } ] ")" ] .
attribute_arg  = expression | identifier "=" expression .
attributes     = { attribute } .

declaration = attributes [ "pub" ] ( function | type_def | trait_def | impl_block | test | constant_decl ) .
```

This grammar:
- Defines a single `attribute` production for all attribute types
- Allows any number of attributes (`attributes = { attribute }`) before any declaration
- Supports both positional arguments (expressions) and named arguments (`key = value`)
- The compiler validates which attributes are valid for which declarations

**Positioning rule:** Attributes must appear immediately before the declaration they modify, with no intervening blank lines or other declarations.

Examples:

```ori
#derive(Eq, Clone)
#skip("reason")
#deprecated("use @new_function instead")
#inline
#cfg(target = "wasm")
```

### Lexer Changes

Current tokenization:
```
#[derive(Eq)]
^^ ^^^^^^ ^^ ^
|  |      |  +-- RBracket
|  |      +-- RParen
|  +-- Identifier + LParen + Identifier + RParen
+-- HashBracket (single token)
```

New tokenization:
```
#derive(Eq)
^ ^^^^^^ ^^
| |      +-- LParen + Identifier + RParen
| +-- Identifier
+-- Hash
```

The lexer emits `Hash` followed by `Identifier`. The parser recognizes this as an attribute when `Hash` appears at statement position.

### Parser Changes

When the parser sees `#` at statement/declaration position:
1. Consume `#`
2. Expect identifier (attribute name)
3. Optionally parse `(` argument list `)`
4. Attach to following declaration

### Disambiguation with `#` Length Operator

Ori uses `#` for length inside index expressions:

```ori
list[# - 1]  // Last element: # means len(list)
```

No conflict exists because:

| Context | `#` Meaning |
|---------|------------|
| Statement position | Attribute prefix |
| Inside `[...]` indexing | Length operator |

The parser knows which context it's in. `#derive(...)` at statement position is unambiguously an attribute. `list[#]` is unambiguously the length operator.

---

## Attributes in Ori

### Current Attributes

| Attribute | Target | Purpose |
|-----------|--------|---------|
| `#derive(Traits...)` | Types | Auto-generate trait implementations |
| `#skip("reason")` | Tests | Skip test execution |

### Planned Attributes

| Attribute | Target | Purpose |
|-----------|--------|---------|
| `#deprecated("msg")` | Any | Mark as deprecated with warning |
| `#inline` | Functions | Suggest inlining |
| `#cold` | Functions | Mark as unlikely to be called |
| `#cfg(condition)` | Any | Conditional compilation |
| `#doc("...")` | Any | Documentation metadata |

### Attribute Arguments

Attributes support several argument styles:

```ori
// No arguments
#inline
@hot_function () -> int = ...

// Positional arguments
#derive(Eq, Clone, Hashable)
type Point = { x: int, y: int }

// String argument
#skip("waiting on upstream fix")
@test_pending tests @feature () -> void = ...

#deprecated("use @new_api instead")
@old_api () -> void = ...

// Named arguments (for complex attributes)
#cfg(target = "wasm", feature = "simd")
@optimized_function () -> void = ...
```

---

## Examples

### Type Derivation

```ori
#derive(Eq, Clone, Hashable)
pub type User = {
    id: int,
    name: str,
    email: str
}

#derive(Eq)
type Status = Active | Inactive | Pending(reason: str)
```

### Skipped Tests

```ori
#skip("flaky on CI, investigating")
@test_network tests @fetch_data () -> void = run(
    let result = fetch_data("https://example.com"),
    assert(is_ok(result))
)

#skip("not yet implemented")
@test_future_feature tests @coming_soon () -> void = run(
    assert(false)
)
```

### Deprecation

```ori
#deprecated("use @parse_v2 instead, will be removed in 0.3")
pub @parse (input: str) -> Result<Ast, ParseError> = ...

pub @parse_v2 (input: str, options: ParseOptions) -> Result<Ast, ParseError> = ...
```

### Conditional Compilation

```ori
#cfg(target = "wasm")
@platform_init () -> void = run(
    wasm_specific_setup()
)

#cfg(target = "native")
@platform_init () -> void = run(
    native_setup()
)
```

### Multiple Attributes

Attributes stack naturally:

```ori
#derive(Eq, Clone)
#deprecated("use NewPoint instead")
pub type Point = { x: int, y: int }

#inline
#cfg(feature = "fast-math")
@fast_sqrt (x: float) -> float = ...
```

---

## Comparison

### Before and After

```ori
// Before: Rust-style brackets
#[derive(Eq, Clone)]
#[deprecated("use NewType")]
pub type OldType = { value: int }

#[skip("wip")]
@test_old tests @old_function () -> void = ...

// After: simplified
#derive(Eq, Clone)
#deprecated("use NewType")
pub type OldType = { value: int }

#skip("wip")
@test_old tests @old_function () -> void = ...
```

### Character Count

| Syntax | Characters | Example |
|--------|-----------|---------|
| Rust-style | `#[name()]` = 4 extra | `#[derive(Eq)]` (14 chars) |
| Simplified | `#name()` = 2 extra | `#derive(Eq)` (12 chars) |

Small per-attribute, but meaningful when attributes are common.

### Visual Comparison

```ori
// Rust-style: bracket noise
#[derive(Eq, Clone)]
#[cfg(target = "wasm")]
#[inline]
pub @function () -> int = ...

// Simplified: cleaner
#derive(Eq, Clone)
#cfg(target = "wasm")
#inline
pub @function () -> int = ...
```

---

## Implementation Notes

### Lexer

Remove `HashBracket` token. The `#` character becomes its own token when not inside `[...]`.

```rust
// Before
HashBracket => "#["

// After
Hash => "#"  // Only at appropriate positions
```

### Migration

This is a breaking syntax change. Migration path:

1. Update lexer/parser to accept both `#[name()]` and `#name()`
2. Add deprecation warning for bracket syntax
3. Provide `ori fmt` auto-migration
4. Remove bracket syntax in next minor version

---

## Design Rationale

### Why Keep `#`?

Alternatives considered:

| Syntax | Issue |
|--------|-------|
| `@attr` | Conflicts with function ori |
| `$attr` | Conflicts with config ori |
| `attr:` | Ambiguous with named arguments |
| Bare `derive(...)` | Ambiguous with function calls |
| `#attr` | Distinct, no conflicts ✓ |

The `#` remains because:
1. Already used for attributes (just removing brackets)
2. Doesn't conflict with other oris
3. Visually lightweight
4. Familiar from other languages

### Why Not Keyword Modifiers?

An alternative was using keyword modifiers like `pub`:

```ori
derive(Eq, Clone) pub type Point = ...
```

Rejected because:
1. `derive` looks like a function call
2. Modifiers typically come after visibility: `pub derive(...)` vs `derive(...) pub`
3. No clear visual marker that it's metadata

### Why Not Postfix?

```ori
type Point = { x: int, y: int } derives(Eq, Clone)
```

Rejected because:
1. Inconsistent with other declaration syntax
2. Harder to scan for attributes
3. Order matters (derives before skip?) becomes confusing

---

## Summary

Simplifying attribute syntax from `#[name(...)]` to `#name(...)`:

1. **Reduces visual noise** — No bracket clutter
2. **Maintains clarity** — `#` still marks attributes distinctly
3. **Requires minimal changes** — Lexer/parser updates are straightforward
4. **Aligns with Ori philosophy** — Oris for discoverability, clean syntax otherwise
5. **No ambiguity** — Context distinguishes `#` as attribute vs length operator

The change is purely syntactic with no semantic impact on how attributes behave.
