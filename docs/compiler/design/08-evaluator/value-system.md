---
title: "Value System"
description: "Ori Compiler Design — Runtime Value Representation"
order: 804
section: "Evaluator"
---

# Value System

## What Is a Value System?

A language's **value system** defines how runtime data is represented in memory. This is a deeper question than it appears: the choice of representation determines what operations are cheap, what are expensive, and what are possible at all. A value system must handle primitives (integers, floats, booleans), compound data (strings, lists, maps, tuples), user-defined types (structs, enums), functions (as first-class values with closures), and special values (Option, Result, iterators). It must also support comparison, hashing, display, and type introspection — all without knowing at compile time which concrete types will be present.

### The Representation Design Space

Language implementations use several strategies for runtime values, each with distinct tradeoffs:

**Tagged unions** (discriminated unions, algebraic data types) store a type tag alongside the value. The tag says "this is an integer" or "this is a string," and the value is stored inline. This is the approach used by most dynamically-typed and interpreted languages: Lua's `TValue` has a tag byte and a `Value` union; Ruby's `VALUE` uses tagged pointers; CPython's `PyObject` has a type pointer. Ori uses this approach.

**Object hierarchies** use inheritance or interfaces to represent values. Java's `Object` base class, C#'s `System.Object`, and JavaScript's prototype chain all follow this pattern. Values are heap-allocated objects with vtables for polymorphic dispatch. This provides unlimited extensibility but every value requires a heap allocation and every method call goes through a vtable.

**Unboxed representations** store values as raw bits with type information maintained statically (at compile time) or through out-of-band metadata. C stores integers as raw `int`, structs as raw byte sequences. Rust's enums use tag + inline payload without heap allocation. This eliminates indirection but requires the runtime to know the exact type of every value.

**NaN-boxing** exploits the IEEE 754 float format's NaN space to encode non-float values inside a 64-bit float. LuaJIT, SpiderMonkey (Firefox's JS engine), and some Scheme implementations use this. The advantage is that all values fit in a single 64-bit word; the disadvantage is that the encoding is platform-specific and limits the number of distinct types.

### Ori's Position

Ori uses a **Rust enum** (`Value`) — a tagged union that stores primitives inline and heap-allocates larger values through `Arc`. This gives O(1) type dispatch (match on discriminant), efficient primitive operations (no indirection for int, float, bool), and shared ownership for compound values (lists, maps, strings share data through reference counting). The `Value` enum lives in the `ori_patterns` crate, making it available to both the evaluator and pattern system.

## The Value Enum

The `Value` type groups its variants by allocation strategy:

### Primitives (Inline, No Heap Allocation)

```rust
Value::Int(ScalarInt)           // Newtype over i64 — checked arithmetic
Value::Float(f64)
Value::Bool(bool)
Value::Char(char)
Value::Byte(u8)
Value::Void                     // Unit type
Value::Duration(i64)            // Nanoseconds (signed, supports negative)
Value::Size(u64)                // Bytes (unsigned, cannot be negative)
Value::Ordering(OrderingValue)  // Less | Equal | Greater
```

These variants store their values directly in the enum — no pointer, no heap allocation. On 64-bit architectures, the `Value` enum is 3 machine words (24 bytes): 1 byte for the discriminant (padded to 8), plus 16 bytes for the largest inline payload. Primitive operations (integer arithmetic, float comparison, boolean logic) operate directly on these inline values with no indirection.

**`ScalarInt`** deserves special attention. It is a `#[repr(transparent)]` newtype over `i64` that deliberately does **not** implement `Add`, `Sub`, `Mul`, `Div`, `Rem`, or `Neg`. All arithmetic goes through checked methods (`checked_add`, `checked_mul`, etc.) that return `Option<ScalarInt>`, making integer overflow impossible to miss at the Rust level. Bitwise operations (`BitAnd`, `BitOr`, `BitXor`, `Not`) are implemented because they cannot overflow. This mirrors the `ScalarInt` approach used by the Rust compiler (`rustc`).

### Heap Types (Arc-Wrapped via `Heap<T>`)

```rust
Value::Str(Heap<Cow<'static, str>>)         // Cow for zero-copy interned strings
Value::List(ListData)                         // Zero-copy slicing via offset/length
Value::Map(Heap<BTreeMap<String, Value>>)     // Deterministic iteration order
Value::Set(Heap<BTreeMap<String, Value>>)     // BTreeMap, not HashSet
Value::Tuple(Heap<Vec<Value>>)
Value::Range(Heap<RangeValue>)
```

Heap types use `Arc` internally (wrapped by `Heap<T>`) for shared ownership. When a list is passed to a function, only the `Arc` pointer is cloned — the list data itself is shared. This makes value passing O(1) regardless of collection size.

**Strings** use `Cow<'static, str>` for a critical optimization: string literals from the source code are interned in the `StringInterner`, and `Value::string_static()` creates a `Cow::Borrowed` pointing directly at the interner's storage — zero copy, zero allocation. Only runtime-created strings (from concatenation, formatting, or user input) use `Cow::Owned` with a heap allocation.

**Maps and Sets** use `BTreeMap<String, Value>` rather than `HashMap`. This provides **deterministic iteration order**, which is required for Salsa compatibility (hash-based ordering can vary across runs, breaking Salsa's memoization) and consistent test output. Keys are converted to `String` via `to_map_key()` — a unified string representation of each value's identity.

**Lists** use `ListData`, a specialized type that supports zero-copy slicing through offset/length windowing into an `Arc`-backed `Vec<Value>`. Taking a slice of a list shares the underlying storage without copying elements.

### The `Heap<T>` Wrapper

The `Heap<T>` type enforces controlled heap allocation:

```rust
#[repr(transparent)]
pub struct Heap<T: ?Sized>(Arc<T>);

impl<T> Heap<T> {
    pub(super) fn new(value: T) -> Self {
        Heap(Arc::new(value))
    }
}
```

The constructor is `pub(super)` — only code within the `value` module can create `Heap` values directly. External code must use factory methods on `Value`:

```rust
Value::string("hello")
Value::list(vec![Value::int(1), Value::int(2)])
Value::map(BTreeMap::new())
Value::some(Value::int(42))
```

This channeling ensures consistent memory management: every heap allocation goes through a known factory method, and there is no way to accidentally bypass the `Arc` wrapper. The `#[repr(transparent)]` annotation ensures `Heap<T>` has the exact same memory layout as `Arc<T>` — zero overhead.

### Algebraic Types

```rust
Value::Some(Heap<Value>)
Value::None
Value::Ok(Heap<Value>)
Value::Err(Heap<Value>)
```

Option and Result are represented as **split variants** rather than wrapper types. There is no `Value::Option(Option<Value>)` — instead, `Some` and `None` are direct variants of `Value`. This eliminates one level of indirection for the most common algebraic types and makes pattern matching in the evaluator simpler (one match arm per variant rather than nested matching).

### User-Defined Types

```rust
Value::Struct(StructValue)
Value::Variant { type_name: Name, variant_name: Name, fields: Heap<Vec<Value>> }
Value::VariantConstructor { type_name: Name, variant_name: Name, field_count: usize }
Value::Newtype { type_name: Name, inner: Heap<Value> }
Value::NewtypeConstructor { type_name: Name }
```

**Structs** carry a `StructLayout` for O(1) field access by name — the layout maps field names to positional indices, so `point.x` compiles to an index lookup rather than a hash map search.

**Variants** carry their type name and variant name as `Name` values (interned indices), plus positional fields. Unit variants (no fields) have an empty field list. Variant constructors are registered as callable values in the environment — calling `Some(42)` evaluates the `VariantConstructor` with one argument.

**Newtypes** are nominally distinct wrappers — `type UserId = int` creates a new type that wraps an `int`. At runtime, `Newtype` carries the type name and the wrapped value. The `.inner` field (always public) and `.unwrap()` method provide access to the wrapped value.

### Callable Values

```rust
Value::Function(FunctionValue)
Value::MemoizedFunction(MemoizedFunctionValue)
Value::FunctionVal(FunctionValFn, &'static str)
```

**`FunctionValue`** is the runtime representation of user-defined functions and lambdas. It carries parameter names, captured environment (as `Arc<FxHashMap<Name, Value>>`), a `SharedArena` reference (for arena threading), canonical body (`CanId`), default parameter expressions, and required capabilities.

A key design detail: `FunctionValue::new()` does **not** take a body parameter. The canonical body is set post-construction via `set_canon()` once canonicalization completes. This two-phase construction is necessary because functions are registered in the environment during module loading (before canonicalization), and the canonical body is attached later when the canonical IR is ready.

**`MemoizedFunction`** wraps a function with a cache for the `recurse(memo: true)` pattern — memoized recursion with automatic result caching.

**`FunctionVal`** represents built-in type conversions like `int()`, `str()`, `float()`. Each is a function pointer with a name string for error messages.

### Other Values

```rust
Value::Iterator(IteratorValue)
Value::ModuleNamespace(Heap<BTreeMap<Name, Value>>)
Value::Error(Heap<ErrorValue>)
Value::TypeRef { type_name: Name }
```

**Iterators** are lazy — `IteratorValue` is an enum of iterator states (list iterator, range iterator, mapped, filtered, zipped, chained, etc.) that produce values one at a time via `next()`. Adapter methods like `map()` and `filter()` create new iterator states that wrap the source iterator.

**Module namespaces** represent `use std.math as math` — a `BTreeMap` from exported names to their values, enabling qualified access like `math.sqrt(x:)`.

**Type references** represent type names used as receivers for associated function calls: `Point.origin()` evaluates `Point` to `Value::TypeRef { type_name }`, and method dispatch then looks up `origin` as an associated function rather than an instance method.

## Type Name Resolution

Values need to report their types for error messages and method dispatch. Two methods handle this:

**`type_name()`** returns a `&'static str` for primitive types (`"int"`, `"float"`, `"str"`, etc.). For user-defined types, it returns the generic category (`"struct"`) because resolving the actual type name requires the string interner.

**`type_name_with_interner()`** takes a `StringLookup` reference and returns the actual type name — e.g., `"Point"` instead of `"struct"`. The `StringLookup` trait is defined in `ori_ir` and implemented by `StringInterner`, avoiding a circular dependency between `ori_patterns` and the interner.

## Equality and Hashing

Values implement `Eq` and `Hash` to support collections (`HashSet<Value>`, `HashMap<Value, _>`) and derived trait evaluation:

- **Structural equality**: two structs are equal if they have the same type name and their fields are pairwise equal
- **Map equality**: same keys with pairwise-equal values (O(n) comparison)
- **Float hashing**: uses `to_bits()` for consistency — `0.0` and `-0.0` hash differently, and NaN hashes consistently
- **Discriminant hashing**: the enum discriminant is hashed first, preventing cross-type collisions

## Prior Art

**Lua's `TValue`** is a tagged union with roughly 10 types (nil, boolean, number, string, table, function, userdata, thread, lightuserdata, C function). Ori's `Value` has more variants (~30) because it represents algebraic types, iterators, and module namespaces as distinct variants rather than encoding them in tables.

**CPython's `PyObject`** uses a type pointer and reference count in every object, with all values heap-allocated. Ori avoids heap allocation for primitives (stored inline in the enum) and uses `Arc` only for compound values, reducing allocation pressure.

**V8's tagged pointers** use the low bit to distinguish between Smis (small integers, stored inline) and heap objects (pointer). This is more memory-efficient than Ori's approach (8 bytes vs 24 bytes per value) but limits the integer range and requires careful pointer arithmetic. LuaJIT's NaN-boxing achieves similar density.

**Roc's value representation** during interpretation uses a Rust enum similar to Ori's `Value`. Roc also uses split variants for Option/Result (`RocResult::Ok` / `RocResult::Err`). The key difference is that Roc's enum is more closely tied to its type representation, while Ori's `Value` is independent of the type system — it lives in `ori_patterns`, not `ori_types`.

**Haskell's GHC runtime** uses a uniform representation where every value is a pointer to a heap-allocated closure. Even integers are boxed (though GHC has unboxed primitives as an optimization). This uniformity simplifies the runtime at the cost of pervasive heap allocation. Ori's hybrid approach — inline primitives, heap-allocated compounds — avoids this cost for the common case.

## Design Tradeoffs

**Split Option/Result variants vs wrapper types.** Having `Some`, `None`, `Ok`, and `Err` as direct variants of `Value` (rather than `Value::Option(Option<Value>)`) saves one level of indirection and simplifies pattern matching in the evaluator. The cost is that Option and Result are special-cased — they cannot be treated as ordinary user-defined types within the value system. This is acceptable because Option and Result are fundamental to Ori's error handling model and appear in virtually every program.

**`BTreeMap` vs `HashMap` for maps.** `BTreeMap` is slower for lookup (O(log n) vs O(1) amortized) but provides deterministic iteration order. For an interpreter where maps are typically small (10-100 entries), the performance difference is negligible. The determinism benefit — consistent output across runs, Salsa compatibility — is significant.

**String keys via `to_map_key()` vs typed keys.** Map keys are converted to `String` representations for storage. This means `{1: "a"}` stores the key as `"1"`. The alternative — supporting arbitrary `Value` keys directly — would require `Value` to implement `Ord` (for `BTreeMap`) with a total ordering, which is problematic for floats (NaN) and functions (no natural ordering). The string-key approach is simpler and sufficient for Ori's use cases.

**24-byte enum vs pointer-tagged values.** The `Value` enum is 24 bytes on 64-bit platforms — larger than the 8-byte tagged pointers used by V8 or LuaJIT. The tradeoff is clarity over density: the enum is idiomatic Rust, trivially safe, and easy to extend. For an interpreter where value manipulation is not the bottleneck (interpretation overhead dominates), the 3x size increase is acceptable.

**`Arc` vs `Rc` for heap values.** `Heap<T>` wraps `Arc<T>`, which uses atomic reference counting. Since the interpreter is single-threaded, `Rc` would suffice and be slightly faster. The `Arc` choice is forward-looking — it allows `Value` to be sent across threads if concurrent evaluation is ever added, and it allows values to be used in Salsa queries (which require `Send + Sync`). The performance difference (~2-5 ns per inc/dec) is insignificant in an interpreter where method dispatch and environment lookup dominate.

## Related Documents

- [Evaluator Overview](index.md) — How the value system fits into the evaluator architecture
- [Tree Walking](tree-walking.md) — How values are produced by expression evaluation
- [Environment](environment.md) — How values are stored in variable bindings
- [Type Representation](../02-intermediate-representation/type-representation.md) — The compile-time analog (type pool vs runtime values)
