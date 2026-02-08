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

**Status:** Planned
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

**Replacement:** `MethodTable` checks `user_methods` hash first (highest priority, matching current behavior where user methods override builtins), then `derived_methods` hash, then `dispatch_builtin_method()` (pattern-match, O(1)), then `generic_methods` hash lookup. No chain, no trait objects, no resolver registration.

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
    /// Uses Arc<RwLock> for interior mutability — allows registration during evaluation
    /// (see Section 06.3 for rationale).
    user_methods: Arc<RwLock<FxHashMap<(Name, Name), Value>>>,
    /// Derived methods from `#[derive(...)]`: (type_name, method_name) → DerivedMethodInfo.
    /// Uses Arc<RwLock> for the same reason as user_methods.
    /// Stored separately from user_methods because they carry DerivedMethodInfo
    /// (field_names, trait_kind) rather than a callable Value.
    /// In the current codebase, UserRegistryResolver checks user methods first,
    /// then derived methods — both at priority 0. V2 preserves this: user_methods
    /// are checked before derived_methods within the same priority tier.
    derived_methods: Arc<RwLock<FxHashMap<(Name, Name), DerivedMethodInfo>>>,
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
- **User-defined methods** use hash table lookup in `user_methods` — checked FIRST (highest priority, matching current behavior). User methods can override builtins.
- **Derived methods** use hash table lookup in `derived_methods` — checked after user methods but before builtins (matching current `UserRegistryResolver` which checks both at priority 0)
- **Builtin methods** use pattern-match dispatch via `dispatch_builtin_method()` (already O(1) jump table via Rust match; no hash map — pattern matching is faster and more maintainable for a fixed set of builtins)
- **Generic/higher-order methods** (map, filter, fold, etc.) remain pattern-match based in `generic_methods`, checked after builtins
- Explicit priority (matching current behavior): user methods first (hash), then derived methods (hash), then builtins (`dispatch_builtin_method` pattern match), then generic methods (hash + applicability check). User methods can override builtins.
- Easy to add new types/methods without modifying dispatch chain
- Clear error messages: "method `foo` not found on type `Bar`" (can enumerate available methods)

- [ ] Define `MethodTable` struct
  - [ ] `user_methods: Arc<RwLock<FxHashMap<(Name, Name), Value>>>` (shared across parent/child interpreters)
  - [ ] `derived_methods: Arc<RwLock<FxHashMap<(Name, Name), DerivedMethodInfo>>>` (derived methods from `#[derive(...)]`, shared like user_methods)
  - [ ] `generic_methods: FxHashMap<Name, GenericMethodHandler>`
- [ ] Define `MethodContext` trait for handler callbacks
  - [ ] Minimal surface: `call_function` (for higher-order methods), `interner`
  - [ ] No `eval(ExprId)` — handlers must not evaluate arbitrary expressions
  - [ ] Interpreter implements this trait
- [ ] Implement lookup algorithm (priority order matches current codebase: user methods override builtins)
  - [ ] 1. Check `user_methods[(type_name, method_name)]` — hash lookup for user-defined methods (highest priority)
  - [ ] 2. Check `derived_methods[(type_name, method_name)]` — hash lookup for `#[derive(...)]` methods
  - [ ] 3. Call `dispatch_builtin_method(value, method_name, args)` — pattern-match dispatch for builtins (O(1) via Rust match)
  - [ ] 4. Check `generic_methods[method_name]` where `applicable_to(&value)` — pattern-match + applicability
  - [ ] 5. Return error with suggestions (similar method names)

---

## 06.2 Method Dispatch Architecture

Organize method dispatch into the hybrid pattern-match + hash-table approach.

**Pre-dispatch special cases** (checked BEFORE the MethodTable lookup, matching current `eval_method_call`):

1. **TypeRef / associated function dispatch:** When the receiver is `Value::TypeRef { type_name }`, the call targets an associated function (static method), not an instance method. Example: `Duration.from_seconds(s: 10)`. The current code checks `user_method_registry` for user-defined associated functions first, then falls back to `dispatch_associated_function()` for builtins (Duration, Size). In V2, this becomes: check `user_methods[(type_name, method_name)]` first (which stores associated functions the same way as instance methods but without `self`), then fall back to `dispatch_associated_function()`. This special case returns early — it never enters the normal method dispatch path.

2. **Callable struct field dispatch:** When the receiver is `Value::Struct(s)` and the struct has a field matching the method name that holds a callable value (Function, MemoizedFunction, MultiClauseFunction, FunctionVal), call the field value directly instead of dispatching as a method. Example: `Handler { callback: fn }.callback(arg)` calls the function stored in the `callback` field. This is checked BEFORE method dispatch and returns early if the field exists and is callable (non-callable fields fall through to normal method dispatch).

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
///
/// **Migration note:** The current signature is:
///   `dispatch_builtin_method(receiver: Value, method: &str, args: Vec<Value>, interner: &StringInterner) -> EvalResult`
/// It returns EvalResult directly (errors for unknown methods via `no_such_method`).
///
/// The V2 signature is adapted to return `Option<EvalResult>` so the MethodTable
/// can fall through to user/generic methods when no builtin matches. The `interner`
/// parameter is replaced by `MethodContext` which provides interner access. The
/// `method` parameter changes from `&str` to `Name` (interned) for consistency
/// with the rest of V2. The `args` parameter changes from owned `Vec<Value>` to
/// borrowed `&[Value]` to avoid unnecessary allocation at call sites.
///
/// Covers: Int, Float, Bool, Char, Byte, Str, List, Map, Option, Result,
/// Duration, Size, Range, Ordering, and Newtype methods.
fn dispatch_builtin_method(
    ctx: &mut dyn MethodContext,
    receiver: Value,
    method: Name,
    args: &[Value],
) -> Option<EvalResult> {
    // Pattern match on receiver type and method name
    // Returns None if no builtin method matches (falls through to user/generic)
    // Current `no_such_method()` error arms become `None` returns
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
    - [ ] `Newtype` (unwrap — returns the wrapped inner value)
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
- [ ] Migrate internal `dispatch_*_method` functions from `&str` to `Name` for method name comparison
  - **Scope:** All `dispatch_*_method` functions in `methods/` currently take `method: &str`. The V2 outer dispatch passes `Name` (interned). This is a mechanical but widespread change affecting: `dispatch_int_method`, `dispatch_float_method`, `dispatch_bool_method`, `dispatch_char_method`, `dispatch_byte_method`, `dispatch_list_method`, `dispatch_string_method`, `dispatch_map_method`, `dispatch_range_method`, `dispatch_option_method`, `dispatch_result_method`, `dispatch_newtype_method`, `dispatch_duration_method`, `dispatch_size_method`, `dispatch_ordering_method`, plus `dispatch_associated_function` and `dispatch_duration_associated`/`dispatch_size_associated`.
  - **Approach (two options):**
    1. **Full conversion (preferred):** Change all internal functions to accept `Name` and compare against pre-interned method name constants (similar to `TypeNames` pattern). This avoids interner lookups on every method call but requires a `MethodNames` struct interned at startup.
    2. **Bridge conversion (incremental):** Have the outer `MethodTable::dispatch` convert `Name` → `&str` via `interner.lookup(method)` once, then pass `&str` to all internal functions unchanged. Simpler migration but keeps the string comparison cost. Suitable as a transitional step during incremental migration.
  - **Recommendation:** Start with option 2 (bridge) for the initial V2 migration, then convert to option 1 as a follow-up optimization when `MethodNames` constants are established.
- [ ] Migrate pre-dispatch special cases from `eval_method_call`
  - [ ] TypeRef/associated function dispatch: preserve current early-return path for `Value::TypeRef` receivers; check `user_methods` then `dispatch_associated_function()` for builtins
  - [ ] Callable struct field dispatch: preserve current early-return path for `Value::Struct` receivers where the field name matches the method and holds a callable value
  - [ ] Both special cases must be checked BEFORE the MethodTable lookup (matching current behavior)
- [ ] Migrate `DerivedMethodInfo` and `eval_derived_method` from current `derived_methods.rs`
  - [ ] `DerivedMethodInfo` (field_names, trait_kind) stored in `derived_methods` hash map
  - [ ] `eval_derived_method` preserved as-is — dispatches by `DerivedTrait` kind (Eq, Clone, Hashable, Printable, Default)
  - [ ] Registration during `#[derive(...)]` processing calls `table.register_derived_method(type_name, method_name, info)`
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
        self.user_methods.write().insert((type_name, method_name), func);
    }
}
```

**Integration with current `UserMethodRegistry`**:
- The current `SharedMutableRegistry<UserMethodRegistry>` uses interior mutability to allow methods to be registered after the dispatcher is created
- With `MethodTable`, user methods are registered during module evaluation (when `impl` blocks are processed)
- **Registration-during-evaluation timing:** The `MethodTable` uses `Arc<RwLock>` internally for `user_methods` to allow registration during evaluation. This matches the current codebase pattern where `SharedMutableRegistry<UserMethodRegistry>` uses `Arc<parking_lot::RwLock<T>>` to share method registries across parent/child interpreters. Using `Arc<RwLock>` (via `parking_lot::RwLock`) preserves this sharing — the `MethodTable` can be cloned cheaply (Arc clone) when creating child interpreters for function/method calls, and all interpreters see the same method registrations. The `generic_methods` map is immutable after construction and does not need `Arc<RwLock>`.

```rust
pub struct MethodTable {
    /// User-defined methods — Arc<RwLock> allows registration during evaluation
    /// and sharing across parent/child interpreters (matching current SharedMutableRegistry pattern).
    /// Locks are always short-lived (single insert or lookup, never held across eval calls).
    user_methods: Arc<RwLock<FxHashMap<(Name, Name), Value>>>,
    /// Derived methods — same sharing pattern as user_methods.
    derived_methods: Arc<RwLock<FxHashMap<(Name, Name), DerivedMethodInfo>>>,
    /// Generic methods — immutable after with_generics() construction.
    generic_methods: FxHashMap<Name, GenericMethodHandler>,
}
```

- [ ] Replace `UserMethodRegistry` with direct `MethodTable` registration
  - [ ] `impl` block evaluation calls `table.register_user_method(type_name, method_name, func)`
  - [ ] Use `Arc<RwLock>` internally for `user_methods` to support registration during evaluation
  - [ ] Remove `SharedMutableRegistry` wrapper (replaced by `Arc<RwLock>` on the map itself)
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
        self.user_methods.write().insert((for_type, method_name), func);
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
- [ ] User method registration replaces `SharedMutableRegistry` (using `Arc<RwLock>` internally)
- [ ] Derived method dispatch (`DerivedMethodInfo`, `eval_derived_method`) migrated to `MethodTable.derived_methods`
- [ ] Pre-dispatch special cases preserved (TypeRef/associated functions, callable struct fields)
- [ ] Internal `dispatch_*_method` functions migrated from `&str` to `Name` (or bridged via interner lookup)
- [ ] `MethodContext` trait implemented by Interpreter
- [ ] Trait method dispatch designed (placeholder for Section 3)
- [ ] Error messages include method suggestions (fuzzy matching)
- [ ] All method call tests pass unchanged
- [ ] Benchmark: method dispatch speed vs. current chain

**Exit Criteria:** Method resolution is O(1) via hash lookup (user/derived methods) and pattern-match dispatch (builtins), with a clear priority chain: user methods → derived methods → builtins → generic methods. User methods can override builtins, matching current behavior. Error messages include method suggestions when methods aren't found.
