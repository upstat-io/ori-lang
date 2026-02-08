---
section: "06"
title: Method Resolution V2
status: not-started
goal: Replace chain-of-responsibility with hash-based dispatch and trait method support
sections:
  - id: "06.1"
    title: MethodTable Design
    status: not-started
  - id: "06.2"
    title: Method Dispatch Architecture
    status: not-started
  - id: "06.3"
    title: User-Defined Method Integration
    status: not-started
  - id: "06.4"
    title: Trait Method Dispatch
    status: not-started
---

# Section 06: Method Resolution V2

**Status:** 📋 Planned
**Goal:** Replace the current chain-of-responsibility `MethodDispatcher` with a hash-based `MethodTable` that supports builtin, user-defined, and trait methods — improving lookup speed and extensibility.

---

## Prior Art Analysis

### Current Ori: Chain of Responsibility
The current system dispatches methods through a chain: `UserRegistry → Collection → Builtin`. Each resolver tries to handle the call; if it can't, it passes to the next. The `BuiltinResolver` at the end of the chain is a catch-all pass-through that delegates to `dispatch_builtin_method()`, which performs the real type-based dispatch via pattern matching on value type and method name. This is O(n) in the number of resolvers per call (though the final builtin dispatch is effectively O(1) via jump table), and the chain order determines priority. Adding trait methods would require yet another resolver in the chain.

**Infrastructure to remove completely:**
- `MethodResolver` trait — the chain-of-responsibility abstraction
- `MethodResolverKind` enum — resolver variant identification
- `MethodDispatcher` struct — the chain holder/orchestrator
- `BuiltinMethodResolver` — delegates to `dispatch_builtin_method()` (function itself is kept)
- `CollectionMethodResolver` — replaced by `generic_methods` hash map
- `UserMethodResolver` — replaced by `user_methods` hash map

**Replacement:** `MethodTable` calls `dispatch_builtin_method()` first (pattern-match, O(1)), then falls through to `user_methods` hash lookup, then `generic_methods` hash lookup. No chain, no trait objects, no resolver registration.

### TypeScript: Lookup via Symbol Properties
TypeScript resolves property access via `getPropertyOfType(apparentType, name)` — a hash map lookup on the type's property table. This is O(1) and handles inherited properties, indexed signatures, and mapped types uniformly.

### Roc: Ability Member Specialization
Roc resolves ability members (analogous to trait methods) by looking up the concrete type's implementation during monomorphization. At evaluation time, the specialized function is called directly — no dynamic dispatch.

---

## 06.1 MethodTable Design

```rust
/// A unified method table that replaces the chain-of-responsibility dispatcher.
/// Builtin methods are NOT stored in the table — they use pattern-match dispatch
/// via `dispatch_builtin_method()` (already O(1) via Rust match jump table).
/// Only user-defined and trait methods use hash lookup.
pub struct MethodTable {
    /// User-defined methods from impl blocks: (type_name, method_name) → FunctionValue.
    /// Uses RefCell for interior mutability — allows registration during evaluation
    /// (see Section 06.3 for rationale).
    user_methods: RefCell<FxHashMap<(Name, Name), Value>>,
    /// Collection/generic methods that work on multiple types: method_name → handler.
    /// Immutable after construction.
    generic_methods: FxHashMap<Name, GenericMethodHandler>,
}

/// NOTE: TypeTag has been removed — it was defined but had no consumer in the
/// dispatch path. Type identification uses the existing `TypeNames` constants
/// and `get_value_type_name()` function, which already provide O(1) type name
/// lookup via interned `Name` values.

/// NOTE: MethodTable is runtime-only — it is never stored in Salsa queries.
/// This makes fn pointers and dyn Trait acceptable here, since Salsa requires
/// Clone + Eq + Hash + Debug which fn pointers and dyn Trait cannot satisfy.
/// The MethodTable lives on the Interpreter and is constructed once at startup.
/// Receiver is owned Value (not &Value) to match current codebase convention.
/// Callers clone at the call site if they need to retain the receiver.
pub type MethodHandler = fn(&mut dyn MethodContext, Value, &[Value]) -> EvalResult;

pub struct GenericMethodHandler {
    /// Method implementation
    handler: MethodHandler,
    /// Which types support this method
    applicable_to: fn(&Value) -> bool,
}

/// Context trait that the method handler can use to interact with the interpreter.
/// Deliberately minimal — handlers should not evaluate arbitrary expressions.
pub trait MethodContext {
    /// Invoke a function-value with arguments (for higher-order methods like map/filter/fold).
    fn call_function(&mut self, func: &Value, args: &[Value]) -> EvalResult;
    /// Access the string interner for name resolution.
    fn interner(&self) -> &StringInterner;
}
```

**Hybrid dispatch approach:**
- **Builtin methods** use pattern-match dispatch via `dispatch_builtin_method()` (already O(1) jump table via Rust match; no hash map — pattern matching is faster and more maintainable for a fixed set of builtins)
- **User-defined and trait methods** use hash table lookup in `user_methods` (O(1) instead of chain traversal)
- **Generic/higher-order methods** (map, filter, fold, etc.) remain pattern-match based in `generic_methods`, checked after builtins and user methods
- Explicit priority: builtins first (`dispatch_builtin_method` pattern match), then user methods (hash), then generic methods (hash + applicability check)
- Easy to add new types/methods without modifying dispatch chain
- Clear error messages: "method `foo` not found on type `Bar`" (can enumerate available methods)

- [ ] Define `MethodTable` struct
  - [ ] `user_methods: FxHashMap<(Name, Name), Value>`
  - [ ] `generic_methods: FxHashMap<Name, GenericMethodHandler>`
- [ ] Define `MethodContext` trait for handler callbacks
  - [ ] Minimal surface: `call_function` (for higher-order methods), `interner`
  - [ ] No `eval(ExprId)` — handlers must not evaluate arbitrary expressions
  - [ ] Interpreter implements this trait
- [ ] Implement lookup algorithm
  - [ ] 1. Call `dispatch_builtin_method(value, method_name, args)` — pattern-match dispatch for builtins (O(1) via Rust match)
  - [ ] 2. Check `user_methods[(type_name, method_name)]` — hash lookup for user/trait methods
  - [ ] 3. Check `generic_methods[method_name]` where `applicable_to(&value)` — pattern-match + applicability
  - [ ] 4. Return error with suggestions (similar method names)

---

## 06.2 Method Dispatch Architecture

Organize method dispatch into the hybrid pattern-match + hash-table approach:

Builtin methods are dispatched via `dispatch_builtin_method()` using pattern matching -- they are NOT registered into the `MethodTable`. Only generic/higher-order methods are registered:

```rust
impl MethodTable {
    pub fn with_generics() -> Self {
        let mut table = Self::new();

        // Collection methods (generic — work on List, Map, Str)
        table.register_generic("map", is_iterable, generic_map);
        table.register_generic("filter", is_iterable, generic_filter);
        table.register_generic("fold", is_iterable, generic_fold);
        table.register_generic("any", is_iterable, generic_any);
        table.register_generic("all", is_iterable, generic_all);
        table.register_generic("find", is_iterable, generic_find);
        table.register_generic("collect", is_iterable, generic_collect);

        table
    }
}

/// Builtin method dispatch — pattern match on (value type, method name).
/// This is the existing dispatch_builtin_method() function, preserved as-is.
/// It covers Int, Float, Bool, Char, Byte, Str, List, Map, Option, Result,
/// Duration, Size, Range, Ordering methods.
fn dispatch_builtin_method(
    ctx: &mut dyn MethodContext,
    receiver: Value,
    method: Name,
    args: &[Value],
) -> Option<EvalResult> {
    // Pattern match on receiver type and method name
    // Returns None if no builtin method matches (falls through to user/generic)
    // ...
}
```

- [ ] Migrate all methods from current resolvers to new dispatch architecture
  - [ ] `methods/numeric.rs` → pattern-match dispatch for `Int` (abs, clamp, to_float, to_string, ...) and `Float` (round, ceil, floor, to_int, ...)
  - [ ] `methods/variants.rs` → pattern-match dispatch for:
    - [ ] `Option` (unwrap, map, unwrap_or, is_some, is_none, ...)
    - [ ] `Result` (unwrap, map, map_err, unwrap_or, is_ok, is_err, ...)
    - [ ] `Bool` (to_string, ...)
    - [ ] `Char` (to_string, is_alphabetic, is_digit, to_upper, to_lower, ...)
    - [ ] `Byte` (to_int, to_char, ...)
    - [ ] Newtype (inner, ...)
  - [ ] `methods/collections.rs` → pattern-match dispatch for:
    - [ ] `List` (len, push, pop, get, contains, reverse, sort, slice, ...)
    - [ ] `Str` (len, contains, split, trim, starts_with, ends_with, to_upper, to_lower, replace, chars, ...)
    - [ ] `Map` (len, get, insert, remove, keys, values, contains_key, ...)
    - [ ] `Range` (contains, len, to_list, ...)
  - [ ] `methods/units.rs` → pattern-match dispatch for `Duration` (to_secs, to_millis, ...) and `Size` (to_bytes, ...)
  - [ ] `methods/ordering.rs` → pattern-match dispatch for `Ordering` (is_lt, is_eq, is_gt, ...)
  - [ ] `interpreter/method_dispatch.rs` → `generic_methods` for higher-order methods:
    - [ ] `map`, `filter`, `fold`, `find`, `any`, `all`, `collect` (work on List, Str, Map, Range)
- [ ] Ensure method handler signatures are uniform
  - [ ] All handlers: `fn(&mut dyn MethodContext, Value, &[Value]) -> EvalResult`
  - [ ] Receiver is owned `Value` (matching current codebase convention; callers clone at call site if retention needed)
  - [ ] Additional arguments in `&[Value]` slice
- [ ] Keep method implementations in separate files (current organization is good)
  - [ ] `methods/numeric.rs`, `methods/collections.rs`, etc. remain
  - [ ] Only the dispatch mechanism changes

---

## 06.3 User-Defined Method Integration

User-defined methods from `impl` blocks register into the method table:

```rust
impl MethodTable {
    /// Register a user-defined method (from impl block evaluation)
    pub fn register_user_method(
        &self,
        type_name: Name,
        method_name: Name,
        func: Value,
    ) {
        self.user_methods.borrow_mut().insert((type_name, method_name), func);
    }
}
```

**Integration with current `UserMethodRegistry`**:
- The current `SharedMutableRegistry<UserMethodRegistry>` uses interior mutability to allow methods to be registered after the dispatcher is created
- With `MethodTable`, user methods are registered during module evaluation (when `impl` blocks are processed)
- **Registration-during-evaluation timing:** The `MethodTable` uses `RefCell` internally for `user_methods` to allow registration during evaluation. This is safe because: (a) registration and lookup never occur simultaneously on the same method entry, (b) `RefCell` panics on aliasing violations rather than silently corrupting, (c) the borrow is always short-lived (insert or lookup, never held across eval calls). The `generic_methods` map is immutable after construction and does not need `RefCell`.

```rust
pub struct MethodTable {
    /// User-defined methods — RefCell allows registration during evaluation.
    /// Borrows are always short-lived (single insert or lookup).
    user_methods: RefCell<FxHashMap<(Name, Name), Value>>,
    /// Generic methods — immutable after with_generics() construction.
    generic_methods: FxHashMap<Name, GenericMethodHandler>,
}
```

- [ ] Replace `UserMethodRegistry` with direct `MethodTable` registration
  - [ ] `impl` block evaluation calls `table.register_user_method(type_name, method_name, func)`
  - [ ] Use `RefCell` internally for `user_methods` to support registration during evaluation
  - [ ] Remove `SharedMutableRegistry` wrapper (replaced by `RefCell` on the map itself)
  - [ ] Methods are visible immediately after registration (current behavior preserved)
- [ ] Handle method override/shadowing
  - [ ] User methods override builtin methods for the same (type, name) pair
  - [ ] Last registration wins (for multiple impl blocks on same type)
  - [ ] Emit warning for shadowing builtin methods

---

## 06.4 Trait Method Dispatch

Prepare for future trait/ability methods (roadmap Section 3):

```rust
/// Trait method resolution (future — placeholder design)
pub struct TraitMethodEntry {
    /// The trait defining this method
    pub trait_name: Name,
    /// The method name
    pub method_name: Name,
    /// Implementation for a specific type
    pub implementations: FxHashMap<Name, Value>, // type_name → impl function
}

impl MethodTable {
    /// Register a trait method implementation
    pub fn register_trait_impl(
        &self,
        trait_name: Name,
        method_name: Name,
        for_type: Name,
        func: Value,
    ) {
        // Store as user method: trait dispatch resolved at call site
        // Key: (for_type, method_name) — same as user methods
        self.user_methods.borrow_mut().insert((for_type, method_name), func);
    }
}
```

**Design note**: Trait methods are resolved by the same mechanism as user methods — `(type_name, method_name) → function`. The type checker ensures that the correct implementation is selected. This aligns with Roc's approach where ability members are specialized at monomorphization time.

- [ ] Design trait method storage in MethodTable
  - [ ] Same (type_name, method_name) key as user methods
  - [ ] Type checker ensures correct dispatch (no runtime trait resolution)
- [ ] Document trait method resolution flow
  - [ ] Type checker resolves trait → concrete impl
  - [ ] Evaluator sees concrete function, not abstract method
  - [ ] No vtable needed for interpreter mode (may need vtable for AOT/LLVM)
- [ ] Placeholder for future: `dyn Trait` support
  - [ ] Would require vtable-like dispatch
  - [ ] Defer until roadmap Section 3 (Traits) is implemented

---

## 06.5 Completion Checklist

- [ ] `MethodTable` implemented with hash-based lookup
- [ ] Chain-of-responsibility infrastructure fully removed (`MethodResolver`, `MethodResolverKind`, `MethodDispatcher`, all resolver impls)
- [ ] All builtin methods migrated from current resolvers
- [ ] User method registration replaces `SharedMutableRegistry` (using `RefCell` internally)
- [ ] `MethodContext` trait implemented by Interpreter
- [ ] Trait method dispatch designed (placeholder for Section 3)
- [ ] Error messages include method suggestions (fuzzy matching)
- [ ] All method call tests pass unchanged
- [ ] Benchmark: method dispatch speed vs. current chain

**Exit Criteria:** Method resolution is O(1) via pattern-match dispatch (builtins) and hash lookup (user/trait methods), with a clear priority chain. Error messages include method suggestions when methods aren't found.
