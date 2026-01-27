# Proposal: Remove `dyn` Keyword for Trait Objects

**Status:** Draft
**Author:** Eric
**Created:** 2026-01-25

---

## Summary

Remove the `dyn` keyword for trait objects. Use the trait name directly as a type to indicate dynamic dispatch.

```ori
// Current (Rust-style)
@process (item: dyn Printable) -> void = ...
@handle (items: [dyn Serializable]) -> void = ...

// Proposed
@process (item: Printable) -> void = ...
@handle (items: [Serializable]) -> void = ...
```

---

## Motivation

### The Problem

The `dyn` keyword is Rust jargon. It exists in Rust to distinguish:
- `impl Trait` — static dispatch, monomorphization
- `dyn Trait` — dynamic dispatch, vtable

This distinction matters in Rust because:
1. Performance implications (inlining, code size)
2. `impl Trait` in return position is existential
3. Object safety rules differ

**In Ori, this distinction is unnecessary.**

Ori doesn't expose monomorphization vs dynamic dispatch as a user-facing choice. The compiler decides the dispatch mechanism. Users shouldn't need to think about vtables.

### Prior Art

| Language | Trait/Interface as Type | Keyword Needed? |
|----------|------------------------|-----------------|
| Go | `func process(w Writer)` | No |
| TypeScript | `function process(x: Printable)` | No |
| Java | `void process(Serializable x)` | No |
| C# | `void Process(IPrintable x)` | No |
| Swift | `func process(_ x: Printable)` | No |
| Rust | `fn process(x: dyn Printable)` | Yes (`dyn`) |

Every mainstream language except Rust uses the trait/interface name directly. Ori should follow the common pattern.

### The Ori Philosophy

"Explicit everything" doesn't mean "expose implementation details." It means:
- Clear intent in code
- No hidden behavior
- Predictable semantics

Using `Printable` as a type is explicit about *what* — "this accepts anything Printable." The *how* (dispatch mechanism) is an implementation detail.

---

## Design

### Trait as Type

When a trait name appears as a type (not a bound), it means "any value implementing this trait":

```ori
@print_all (items: [Printable]) -> void =
    for item in items do
        print(item.to_str())
```

The compiler handles dispatch. The user doesn't specify `dyn`.

### Generic Bounds (Unchanged)

Trait bounds on generics remain the same:

```ori
// T must implement Printable — this is a bound, not a type
@print_item<T: Printable> (item: T) -> void =
    print(item.to_str())
```

The difference:
- `item: Printable` — accepts any Printable, dynamic dispatch
- `item: T` where `T: Printable` — generic, compiler chooses dispatch

### When to Use Which

| Syntax | Meaning | Use When |
|--------|---------|----------|
| `item: Printable` | Any Printable value | Heterogeneous collections, simple APIs |
| `item: T` where `T: Printable` | Generic over Printable types | Homogeneous collections, return same type |

```ori
// Heterogeneous — different types in one list
let items: [Printable] = [point, user, "hello"]

// Homogeneous — all same type
@sort<T: Comparable> (items: [T]) -> [T] = ...
```

### Function Parameters

```ori
// Accepts any Printable
@display (item: Printable) -> void =
    print(item.to_str())

// Equivalent to Rust's `fn display(item: &dyn Printable)`
// But without the `dyn` noise
```

### Return Types

```ori
// Returns some Printable (caller doesn't know concrete type)
@make_printable () -> Printable = ...

// Equivalent to Rust's `fn make_printable() -> Box<dyn Printable>`
// But without Box or dyn
```

### Collections

```ori
// List of different Printable things
let items: [Printable] = [
    Point { x: 1, y: 2 },
    User { name: "Alice" },
    "a string",
]

for item in items do
    print(item.to_str())
```

---

## Examples

### Visitor Pattern

```ori
trait Visitor {
    @visit_file (self, file: File) -> void
    @visit_dir (self, dir: Directory) -> void
}

@walk (root: Node, visitor: Visitor) -> void =
    match(root,
        File(f) -> visitor.visit_file(f),
        Directory(d) -> run(
            visitor.visit_dir(d),
            for child in d.children do
                walk(child, visitor),
        ),
    )
```

No `dyn` needed — `visitor: Visitor` is clear.

### Plugin System

```ori
trait Plugin {
    @name (self) -> str
    @execute (self, ctx: Context) -> Result<void, Error>
}

@run_plugins (plugins: [Plugin], ctx: Context) -> Result<void, Error> = try(
    for plugin in plugins do
        plugin.execute(ctx)?,
    Ok(()),
)
```

### Event Handlers

```ori
trait EventHandler {
    @handle (self, event: Event) -> void
}

type EventBus = { handlers: [EventHandler] }

impl EventBus {
    @dispatch (self, event: Event) -> void =
        for handler in self.handlers do
            handler.handle(event)
}
```

---

## Migration

### Spec Changes

| File | Change |
|------|--------|
| `06-types.md` | Remove `dyn` from type syntax |
| `03-lexical-elements.md` | Remove `dyn` from keywords |

### Code Migration

```ori
// Before
@process (item: dyn Printable) -> void = ...
let items: [dyn Serializable] = ...

// After
@process (item: Printable) -> void = ...
let items: [Serializable] = ...
```

Simple find-and-replace: remove `dyn `.

---

## Rationale

### Why Not Keep `dyn` for Explicitness?

"Explicit everything" means explicit *intent*, not explicit *implementation*.

When you write `item: Printable`, the intent is clear: "this accepts anything Printable." Whether that's implemented via vtable, monomorphization, or magic is not the user's concern.

Rust needs `dyn` because it exposes the dispatch choice to users. Ori doesn't.

### Why Not `impl Trait` Syntax?

Rust uses `impl Trait` for existential types in return position. Ori doesn't need this distinction:

```ori
// Ori: just use the trait name
@make_printable () -> Printable = ...

// Rust: needs `impl Trait` or `Box<dyn Trait>`
fn make_printable() -> impl Printable { ... }
fn make_printable() -> Box<dyn Printable> { ... }
```

Ori's approach is simpler. The compiler figures out boxing/allocation.

### Object Safety

Rust has complex "object safety" rules determining which traits can be `dyn`. Ori should have similar rules but doesn't need to expose them via a keyword. If a trait isn't usable as a type, that's a compile error with a clear message.

---

## Summary

| Current | Proposed |
|---------|----------|
| `dyn Printable` | `Printable` |
| `[dyn Serializable]` | `[Serializable]` |
| `{str: dyn Handler}` | `{str: Handler}` |

Remove `dyn` from the language. Trait names used as types implicitly mean "any value implementing this trait."

One less piece of Rust jargon. One more step toward a clean, high-level language.
