# Proposal: Colon Syntax for Trait Implementations

**Status:** Approved
**Author:** Eric (with Claude)
**Created:** 2026-03-05
**Approved:** 2026-03-05
**Affects:** Parser, spec, grammar, all `impl Trait for Type` syntax, capability-unification-generics-proposal

---

## Summary

Replace `impl Trait for Type` with `impl Type: Trait`, putting the subject before the predicate and unifying impl syntax with every other `:` conformance position in the language.

| Before | After |
|--------|-------|
| `impl Eq for Point { ... }` | `impl Point: Eq { ... }` |
| `impl<T: Eq> Eq for [T] { ... }` | `impl<T: Eq> [T]: Eq { ... }` |
| `impl Add<int> for Vector2 { ... }` | `impl Vector2: Add<int> { ... }` |
| `impl Point { ... }` (inherent) | `impl Point { ... }` (unchanged) |
| `pub def impl Trait { ... }` | `pub def impl Trait { ... }` (unchanged) |

---

## Motivation

### The Problem: One Relationship, Two Syntaxes

After the capability unification proposal (approved 2026-02-20, revised 2026-03-04), `:` means "conforms to" everywhere in the language:

```ori
type Point: Eq, Hashable = { x: int, y: int }   // type declaration
T: Comparable                                      // generic bound
where T: Eq                                        // where clause
trait Comparable: Eq                                // supertrait
```

But trait implementations break the pattern:

```ori
impl Eq for Point { ... }    // "for" instead of ":"
```

This is the only place where the conformance relationship uses a different keyword. A developer who has internalized "`:` means conforms to" must remember that `impl` blocks are the exception.

### Subject-First Ordering

Every other conformance position puts the subject first:

| Context | Syntax | Reading |
|---------|--------|---------|
| Type declaration | `type Point: Eq` | "Point conforms to Eq" |
| Generic bound | `T: Eq` | "T conforms to Eq" |
| Where clause | `where T: Eq` | "where T conforms to Eq" |
| Supertrait | `trait Comparable: Eq` | "Comparable requires Eq" |
| **Impl (current)** | `impl Eq for Point` | "implement Eq for Point" |
| **Impl (proposed)** | `impl Point: Eq` | "implement Point conforms to Eq" |

The current `impl Eq for Point` puts the trait first, then the type — the opposite of every other position. The proposed syntax aligns impl with the universal pattern.

### Prior Art

Languages that use `:` for conformance in implementations:

| Language | Syntax | Notes |
|----------|--------|-------|
| **Swift** | `extension Type: Protocol { ... }` | Exact analogue |
| **Kotlin** | `class Type : Interface { ... }` | On class declarations |
| **C#** | `class Type : Interface { ... }` | On class declarations |
| **C++** | `class Type : public Base { ... }` | Inheritance |

Languages that use keywords:

| Language | Syntax |
|----------|--------|
| **Rust** | `impl Trait for Type { ... }` |
| **Haskell** | `instance Class Type where ...` |

The `:` approach is more common in the broader language landscape. Rust's `for` is somewhat unusual — inherited from Haskell's `instance` pattern but with different keyword choice.

### Keyword Simplification

`for` currently has two meanings:
1. Loop iteration: `for x in items do ...`
2. Impl target: `impl Trait for Type`

After this change, `for` is purely a loop keyword. One keyword, one meaning. `for` remains a reserved keyword.

---

## Design

### Grammar Change

Current (`grammar.ebnf` line 299):
```ebnf
trait_impl = "impl" [ generics ] type_path "for" type [ where_clause ] "{" { method } "}" .
```

Proposed:
```ebnf
trait_impl = "impl" [ generics ] type ":" type_path [ where_clause ] "{" { method } "}" .
```

Two changes:
1. `"for"` becomes `":"`
2. The target type moves before the trait (subject-first ordering)

The `inherent_impl` and `def_impl` productions are unchanged:
```ebnf
inherent_impl = "impl" [ generics ] type_path [ where_clause ] "{" { method } "}" .
def_impl      = "def" "impl" identifier "{" { def_impl_method } "}" .
```

### Parsing

Parsing becomes slightly simpler. After `impl` + optional generics, the parser reads a type expression:
- If `:` follows: trait impl — parse the trait name, then `{`
- If `{` follows: inherent impl

The current grammar requires the parser to determine whether the first type expression after `impl` is a trait (if `for` follows) or the target type (if `{` follows). The proposed grammar always reads the target type first — no ambiguity about what the first type expression represents.

### Examples

**Simple trait impl:**
```ori
impl Point: Eq {
    @equals (self, other: Self) -> bool = self.x == other.x && self.y == other.y
}
```

**Generic trait impl:**
```ori
impl<T: Eq> [T]: Eq {
    @equals (self, other: Self) -> bool = {
        if self.len() != other.len() then false
        else self.iter().zip(iter: other.iter()).all(predicate: (a, b) -> a == b)
    }
}
```

**Parameterized trait:**
```ori
impl Vector2: Add<int> {
    type Output = Vector2
    @add (self, other: int) -> Vector2 = Vector2 { x: self.x + other, y: self.y + other }
}
```

**Multiple impls for same type (visual grouping):**
```ori
impl Point: Eq { ... }
impl Point: Comparable { ... }
impl Point: Hashable { ... }
impl Point: Printable { ... }
```

Note how the repeated `impl Point:` prefix makes it immediately clear these all belong to the same type — compared to the current syntax where the type appears at the end of each line.

**Inherent impl (unchanged):**
```ori
impl Point {
    @new (x: int, y: int) -> Point = Point { x, y }
    @distance (self, other: Point) -> float = { ... }
}
```

**Default impl (unchanged):**
```ori
pub def impl Printable {
    @to_str (self) -> str = self.debug()
}
```

### Full Consistency Table

After this change, every conformance relationship in Ori uses `:`:

| Context | Syntax | Subject | `:` | Predicate |
|---------|--------|---------|-----|-----------|
| Type declaration | `type Point: Eq = { ... }` | `Point` | `:` | `Eq` |
| Generic bound | `<T: Eq>` | `T` | `:` | `Eq` |
| Where clause | `where T: Eq` | `T` | `:` | `Eq` |
| Supertrait | `trait Comparable: Eq` | `Comparable` | `:` | `Eq` |
| Trait impl | `impl Point: Eq { ... }` | `Point` | `:` | `Eq` |

Five positions, one symbol, one meaning: "conforms to."

---

## Interaction with Existing Features

### `def impl` — No Change

Default impls have no target type, so no `:` applies:

```ori
pub def impl Debug {
    @debug (self) -> str = `{self}`
}
```

### Error Messages

Diagnostics that reference impl syntax should be updated:

```
// Before
= help: implement `Iterator` manually: `impl Iterator for MyIter { ... }`

// After
= help: implement `Iterator` manually: `impl MyIter: Iterator { ... }`
```

The common mistake detector in `ori_parse/src/error/mistakes.rs` currently suggests `impl Trait for Type { ... }` when detecting Java's `implements` keyword — this must be updated to suggest `impl Type: Trait { ... }`.

### `extend` — No Change

Extensions use a different syntax and are unaffected:

```ori
extend Point {
    @manhattan (self) -> int = self.x.abs() + self.y.abs()
}
```

### Relationship to Capability Unification Proposal

The capability-unification-generics-proposal (approved 2026-02-20) explicitly states that `impl Trait for Type` is "unaffected" (section 12.1) and marks it as "Unchanged" in its summary table. This proposal supersedes that claim — `impl` syntax is now part of the `:` unification. An errata will be added to the capability-unification proposal.

---

## Migration

### Scope

This is a syntactic change to the `impl` block header. The body of impl blocks is unchanged.

### Affected Files

- **Grammar**: `grammar.ebnf` — `trait_impl` production rule
- **Spec**: 22 spec files with 45+ code examples
- **Proposals**: 43 approved proposals with 166 references (errata for each)
- **Tests**: 34 spec test files with 120 occurrences
- **Stdlib**: `library/std/` impl blocks
- **Compiler**: Parser (`impl` parsing in `ori_parse/src/grammar/item/impl_def/`), type checker comments, error messages (`error/mistakes.rs` common mistake detector)
- **Rules**: `.claude/rules/ori-syntax.md`

### Transition

Since the compiler does not yet implement most trait impls (they are derived or built-in), the migration surface in actual `.ori` code is minimal. The primary work is updating spec, grammar, proposals, and error message strings.

---

## Alternatives Considered

### Keep `for`

Status quo. The only conformance position using a different keyword. The inconsistency is small but permanent — every new Ori developer must learn "`:` means conforms-to, except in impl blocks where you use `for`."

### Use `as` instead of `:`

`impl Point as Eq { ... }` — reads well ("implement Point as Eq") but introduces a third syntax for the same relationship. `as` also already means type conversion (`42 as float`), so this would overload it.

### Reverse order: `impl Trait: Type`

`impl Eq: Point { ... }` — puts the trait first like the current syntax but uses `:`. Rejected because `:` everywhere else means "left conforms to right," and `Eq: Point` reads as "Eq conforms to Point" — backwards.

---

## Summary Table

| Aspect | Detail |
|--------|--------|
| Keyword removed from impl | `for` (remains reserved for loop syntax) |
| Keyword added | None (`:` already exists) |
| Grammar change | One production rule (`trait_impl`) |
| Parsing complexity | Slightly reduced (no trait-vs-type ambiguity) |
| Consistency | Completes the `:` = "conforms to" unification |
| Breaking change | Yes (syntactic, all `impl Trait for Type` → `impl Type: Trait`) |
| Migration effort | Medium (spec/grammar/proposals/tests/compiler; minimal `.ori` code) |
