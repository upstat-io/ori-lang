# Proposal: Mutable Self

**Status:** Approved
**Author:** Eric (with Claude)
**Created:** 2026-03-05
**Approved:** 2026-03-05

---

## Summary

Make `self` a mutable binding in method bodies, consistent with Ori's mutable-by-default philosophy. Methods that mutate `self` implicitly propagate the modified value back to the caller through desugaring — the same mechanism used by field assignment and index assignment.

```ori
impl Cursor {
    @advance (self) -> void = {
        self.pos += 1
    }

    @eat_whitespace (self) -> void = {
        loop {
            match self.buf[self.pos] {
                ' ' | '\t' -> self.pos += 1,
                _ -> break,
            }
        }
    }
}

cursor.advance()          // cursor is updated
cursor.eat_whitespace()   // cursor is updated again
```

---

## Motivation

### The Inconsistency

Ori's binding model is mutable by default:

```ori
let x = 0;
x = x + 1;       // OK: mutable by default

let $y = 0;
$y = 1;           // error: immutable binding
```

But function parameters — including `self` — are immutable (spec 13.2.1, 13.5):

```ori
@advance (self) -> void = {
    self.pos += 1   // error: cannot assign to parameter
}
```

Every binding in Ori is mutable unless you opt out with `$`. `self` is the one place where this principle breaks down. This proposal eliminates the inconsistency.

### Where It Hurts

The current workaround forces a functional return-new-value style for every stateful method:

```ori
// Current: must return modified copy
@advance (self) -> Cursor = { ...self, pos: self.pos + 1 };

// Caller must reassign explicitly
cursor = cursor.advance()
```

This becomes painful for imperative algorithms with loops:

```ori
// Current: contortion to avoid mutating self
@eat_whitespace (self) -> Cursor = {
    let pos = self.pos;
    loop {
        match self.buf[pos] {
            ' ' | '\t' -> { pos += 1 },
            _ -> break,
        }
    }
    { ...self, pos }
}
```

And for method sequences that thread state:

```ori
// Current: manual threading of modified self
let cursor = cursor.advance();
let (ch, cursor) = cursor.read_char();
let cursor = cursor.skip_whitespace();

// Proposed: natural imperative style
cursor.advance()
let ch = cursor.read_char()
cursor.skip_whitespace()
```

### Consistency with Existing Desugaring

Ori already supports mutation-through-desugaring for fields and indices:

| Syntax | Desugars to | Spec |
|--------|------------|------|
| `x.field = v` | `x = { ...x, field: v }` | 13.6.1 |
| `x[i] = v` | `x = x.updated(key: i, value: v)` | 13.6.2 |
| `x += 1` | `x = x + 1` | 13.6.3 |

Mutable `self` extends this pattern to method calls: mutation of `self` inside a method propagates back to the caller through implicit reassignment.

---

## Design

### Core Rule

**`self` is a mutable binding in method bodies.** Unlike other parameters, `self` can be reassigned and its fields can be mutated.

```ori
impl Point {
    @translate (self, dx: int, dy: int) -> void = {
        self.x += dx;
        self.y += dy;
    }
}
```

Other parameters remain immutable. Only `self` receives this treatment, because `self` is already special (keyword, method dispatch, receiver syntax).

### Mutation Propagation

When a method mutates `self`, the modified value propagates back to the caller. Conceptually, calling a self-mutating method is like performing a compound assignment on the receiver:

```ori
point.translate(dx: 5, dy: 10)
// Conceptually equivalent to:
point = point.translated(dx: 5, dy: 10)
```

The caller's binding is implicitly reassigned. The caller does not need to capture the return value or write explicit reassignment.

### Mutation Visibility

Self-mutation is fully implicit — inferred from the method body with no annotation at either the declaration or call site. This is consistent with Ori's mutable-by-default philosophy: just as `let x` needs no `mut` keyword, methods need no `mutating` keyword.

The IDE/LSP may display mutation hints, but the language itself requires no markers. This mirrors how field assignment (`x.field = v`) and index assignment (`x[i] = v`) are already desugared implicitly.

### Desugaring Rules

The compiler transforms self-mutating methods at compile time using the same reassignment-desugaring strategy as field and index assignment.

#### Void-returning methods

```ori
// User writes:
@advance (self) -> void = {
    self.pos += 1
}

// Compiler desugars method to:
@__advance (self) -> Self = {
    self.pos += 1;
    self
}

// Call site desugaring:
cursor.advance()
// becomes:
cursor = cursor.__advance()
```

#### Value-returning methods that mutate self

```ori
// User writes:
@read_char (self) -> char = {
    let ch = self.buf[self.pos];
    self.pos += 1;
    ch
}

// Compiler desugars method to:
@__read_char (self) -> (Self, char) = {
    let ch = self.buf[self.pos];
    self.pos += 1;
    (self, ch)
}

// Call site desugaring:
let ch = cursor.read_char()
// becomes:
let (cursor', ch) = cursor.__read_char()
cursor = cursor'
```

#### Non-mutating methods (no transformation)

```ori
@position (self) -> int = self.pos;
// No desugaring — self is not mutated
// Works on both mutable and immutable bindings
```

#### Methods that already return Self explicitly

```ori
@with_position (self, pos: int) -> Self = {
    self.pos = pos;
    self
}
// No additional desugaring needed — already returns Self
// Call site: cursor = cursor.with_position(pos: 10) (explicit reassignment)
```

Methods with return type `Self` are not subject to implicit propagation. The caller captures and reassigns the return value explicitly. This avoids ambiguity: `-> Self` already signals "this produces a new value."

### Mutation Detection

The compiler performs dataflow analysis to classify each method as **mutating** or **non-mutating**. A method is mutating if any of these occur in its body:

- Direct field assignment: `self.field = v`
- Compound field assignment: `self.field += v`
- Self reassignment: `self = expr`
- Calling a self-mutating method on self: `self.advance()`
- Index assignment on self: `self[i] = v`
- Nested field mutation: `self.inner.field = v`
- Calling a self-mutating method on a field of self: `self.inner.advance()`

If none of these occur, the method is non-mutating.

### Caller Requirements

| Method type | Mutable receiver (`let x`) | Immutable receiver (`let $x`) |
|-------------|---------------------------|-------------------------------|
| Non-mutating | OK | OK |
| Self-mutating | OK | Error |

```ori
let cursor = Cursor.new(buf: data);
cursor.advance()            // OK: mutable binding

let $frozen = Cursor.new(buf: data);
$frozen.advance()           // error: cannot call self-mutating method on immutable binding
$frozen.position()          // OK: non-mutating method
```

### Interaction with COW

Self-mutation integrates naturally with Ori's Copy-on-Write optimization:

- **Uniquely referenced (refcount 1):** Fields mutated in place. Zero copy.
- **Shared (refcount > 1):** COW copies, then mutates. Value semantics preserved.

```ori
let a = Cursor.new(buf: data);
let b = a;                    // a and b share (refcount 2)
a.advance()                   // COW: a gets its own copy, mutates in place
// b is unaffected — value semantics preserved
```

This is identical to how list and map COW already works. The ARC pipeline (`ori_arc`) can analyze self-mutating methods for uniqueness just as it does for collection mutations.

---

## Interaction with Traits

### Mutation Classification for Traits

Self-mutation classification is inferred from method bodies, for both `impl` blocks and trait implementations. The compiler checks consistency across all implementations of a trait method.

**Rules:**

1. If **any** implementation of a trait method mutates `self`, the method is classified as mutating for the trait.
2. Non-mutating implementations are compatible with a mutating classification — they simply don't happen to mutate.
3. If an implementation mutates `self` but a prior classification marked the method as non-mutating, the compiler reports an error on the new implementation.

```ori
trait Stepper {
    @step (self) -> void;   // classification determined from impls
}

// This impl mutates self — step() is classified as mutating
impl Stepper for Cursor {
    @step (self) -> void = {
        self.pos += 1
    }
}

// Non-mutating impl is compatible with mutating classification
impl Stepper for Fixed {
    @step (self) -> void = { }
}
```

**Trait objects:** Since trait objects require knowing the mutation classification at the trait level, the compiler must have seen at least one implementation before constructing a trait object. If no implementations are available (e.g., the trait is defined in a library with no local impls), the method is conservatively classified as mutating.

### Iterator Simplification

The Iterator trait currently requires explicit self-return because `self` is immutable:

```ori
// Current
trait Iterator {
    type Item;
    @next (self) -> (Option<Self.Item>, Self);
}

// Usage:
let (value, iter) = iter.next()
```

With mutable self, this simplifies to:

```ori
// Proposed
trait Iterator {
    type Item;
    @next (self) -> Option<Self.Item>;
}

// Usage:
let value = iter.next()
```

This is a significant ergonomic improvement but also a breaking change. It should be tracked in a **separate proposal** for Iterator trait revision.

---

## Interaction with Extensions

Extension methods follow the same rules. Extensions that mutate `self` propagate mutations back to the caller:

```ori
extend Cursor {
    @skip_n (self, n: int) -> void = {
        self.pos += n
    }
}

cursor.skip_n(n: 5)   // cursor is updated
```

---

## Edge Cases

### Self Reassignment

Methods may reassign `self` entirely:

```ori
@reset (self) -> void = {
    self = Cursor { pos: 0, buf: self.buf }
}
```

### Chained Mutating Calls

Void-returning mutating methods cannot be chained (they return `void`):

```ori
cursor.advance().read_char()   // error: void has no method read_char
```

Use sequential statements:

```ori
cursor.advance()
let ch = cursor.read_char()
```

### Nested Self Mutation

Each self-mutating call within a method body updates `self` before the next statement:

```ori
impl Parser {
    @parse_token (self) -> Token = {
        self.eat_whitespace()     // self updated
        let start = self.pos;
        self.advance_to_boundary() // self updated again
        Token { text: self.buf.slice(start:, end: self.pos) }
    }
}
```

### Nested Mutating Method Calls on Fields

When a self-mutating method is called on a field of `self`, the mutation cascades through field-assignment desugaring:

```ori
// User writes:
self.inner.advance()

// Desugars to (method propagation):
self.inner = self.inner.__advance()

// Which further desugars to (field assignment):
self = { ...self, inner: self.inner.__advance() }
```

This cascading desugaring composes naturally with the existing field-assignment and index-assignment rules. Arbitrarily nested chains work:

```ori
self.parser.lexer.advance()
// → self.parser.lexer = self.parser.lexer.__advance()
// → self.parser = { ...self.parser, lexer: ... }
// → self = { ...self, parser: ... }
```

### Argument Evaluation Order

When mutating method calls appear in argument positions, arguments are evaluated left-to-right (per Ori's existing evaluation model). Each mutating call updates `self` before the next argument is evaluated:

```ori
f(a: self.read_char(), b: self.read_char())
// Evaluates as:
// 1. self.read_char() → reads char at pos 0, advances self to pos 1
// 2. self.read_char() → reads char at pos 1, advances self to pos 2
// 3. f(a: char_0, b: char_1)
```

### Closures Capturing Self

Closures capture `self` by value (per Ori's capture-by-value model). The closure receives a mutable copy of `self`. Mutations inside the closure operate on the closure's copy and do not affect the outer `self`:

```ori
@example (self) -> void = {
    let f = () -> {
        self.pos += 1   // mutates closure's copy
    };
    f()
    // self.pos is unchanged here
}
```

### Recursive Methods

Each recursive call operates on its own copy of `self`. Mutations propagate level by level:

```ori
@skip_all_whitespace (self) -> void = {
    if self.pos < self.buf.len() && self.buf[self.pos] == ' ' then {
        self.pos += 1;
        self.skip_all_whitespace()   // recursive — self already advanced
    }
}
```

---

## Comparison with Other Languages

| Language | Mechanism | Annotation | Value Semantics | COW |
|----------|-----------|------------|-----------------|-----|
| **Ori (proposed)** | Mutable self + desugaring | None (mutable by default) | Yes | Yes |
| Swift | `mutating func` | Explicit `mutating` keyword | Yes | Yes |
| Rust | `&mut self` | Explicit `&mut` | No (references) | No |
| Zig | `*Self` pointer | Explicit pointer type | No (references) | No |
| Koka | Algebraic effects | Effect types | Yes (functional) | N/A |

**Swift** is the closest model. Swift value types use `mutating` to mark methods that modify `self`. Under the hood, `self` is passed as `inout`. Ori's approach achieves the same result without a keyword, consistent with Ori's mutable-by-default philosophy where immutability (`$`) is the opt-in.

**Rust** uses `&mut self` as an explicit mutable borrow. This is part of the borrow checker model. Ori has no borrow checker — COW desugaring provides the same ergonomics through value semantics.

**Koka** uses algebraic effects to model state. A mutable state variable is accessed through a `state` effect handler. This is theoretically clean but verbose for common patterns. Ori's approach is more pragmatic.

**Visibility trade-off:** All comparable languages use explicit annotations. Ori's fully implicit approach trades declaration-site clarity for consistency with the mutable-by-default philosophy. The IDE/LSP is expected to provide mutation hints to compensate.

---

## Migration / Compatibility

### Non-breaking for existing code

Existing methods that don't assign to `self` are completely unaffected. Non-mutating methods continue to work on both mutable and immutable bindings.

### Gradual adoption

Methods that currently return modified `Self` explicitly can be gradually migrated:

```ori
// Before: explicit self-return
@advance (self) -> Cursor = { ...self, pos: self.pos + 1 };
cursor = cursor.advance()

// After: implicit propagation
@advance (self) -> void = { self.pos += 1 }
cursor.advance()
```

Both styles coexist. The explicit `-> Self` style remains valid.

---

## Open Questions

*(All resolved during review.)*

1. ~~**Trait annotations:**~~ Resolved: infer from method bodies; check consistency across implementations.
2. ~~**Trait objects:**~~ Resolved: conservative mutating classification when no implementations are available.
3. **Iterator migration:** Should be tracked in a **separate proposal** for Iterator trait revision.
4. ~~**Chaining ergonomics:**~~ Resolved: void-returning methods cannot chain. Use sequential statements.
5. ~~**Error messages:**~~ See Diagnostics section below.

---

## Diagnostics

| Code | Message |
|------|---------|
| E____ | cannot call self-mutating method `{method}` on immutable binding `${name}` |
| E____ | inconsistent mutation: `{method}` is non-mutating in trait `{trait}` but mutates `self` in this impl |

Error codes will be assigned during implementation.

---

## References

- [Spec 11.11 — Self and self](../../v2026/spec/11-blocks-and-scope.md)
- [Spec 13.2 — Mutability](../../v2026/spec/13-variables.md)
- [Spec 13.5 — Function Parameters](../../v2026/spec/13-variables.md)
- [Spec 13.6 — Assignment Semantics](../../v2026/spec/13-variables.md)
- [Spec 21 — Memory Model](../../v2026/spec/21-memory-model.md)
- [Index Assignment Proposal](../approved/index-assignment-proposal.md)
- [Custom Subscripting Proposal](../approved/custom-subscripting-proposal.md)
- [Compound Assignment Proposal](../approved/compound-assignment-proposal.md)

---

## Changelog

- 2026-03-05: Initial draft
- 2026-03-05: Approved — resolved trait annotations (infer from body), added nested field mutation desugaring, added argument evaluation order specification, added mutation visibility rationale, added diagnostics section
