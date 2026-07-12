//! Tests for builtin method dispatch.

use ori_registry::{ReturnTag, TypeTag};

// Fresh return type audit

/// Collect all `(TypeTag, method_name)` pairs in the registry with `ReturnTag::Fresh`.
/// Verify each is handled by `resolve_computed_return` (or intentionally falls
/// back to a fresh type variable).
///
/// This test prevents new `Fresh`-return methods from silently producing
/// unconstrained type variables when they should have computed returns.
#[test]
fn fresh_return_methods_are_documented() {
    let mut fresh_methods: Vec<(TypeTag, &str)> = Vec::new();

    for type_def in ori_registry::BUILTIN_TYPES {
        for method in type_def.methods {
            if method.returns == ReturnTag::Fresh {
                fresh_methods.push((type_def.tag, method.name));
            }
        }
    }

    // Semantic pin on the exact Fresh-return method set: a new `ReturnTag::Fresh`
    // method fails this until it gets a computed_returns entry or documents the fresh var.
    let names: Vec<String> = fresh_methods
        .iter()
        .map(|(tag, name)| format!("{tag:?}.{name}"))
        .collect();

    // Known Fresh-return methods (order matches BUILTIN_TYPES × sorted methods):
    let expected = [
        "Error.trace_entries",
        "List.chunk",
        "List.filter",
        "List.find",
        "List.flat_map",
        "List.flatten",
        "List.fold",
        "List.for_each",
        "List.group_by",
        "List.map",
        "List.max",
        "List.max_by",
        "List.min",
        "List.min_by",
        "List.partition",
        "List.product",
        "List.reduce",
        "List.skip_while",
        "List.sort_by",
        "List.sum",
        "List.take_while",
        "List.window",
        "List.zip",
        "Set.fold",
        "Option.and_then",
        "Option.filter",
        "Option.flat_map",
        "Option.map",
        "Option.or_else",
        "Result.and_then",
        "Result.map",
        "Result.map_err",
        "Result.or_else",
        "Result.trace_entries",
        "Iterator.flat_map",
        "Iterator.flatten",
        "Iterator.fold",
        "Iterator.map",
        "Iterator.rfold",
        "Iterator.zip",
    ];

    assert_eq!(
        names, expected,
        "Fresh-return methods changed. Update this list and verify \
         computed_returns.rs handles new entries."
    );
}

/// Positive clamp: `List.all` / `List.any` and the `Iterator`
/// equivalents return `bool` regardless of the predicate, so the registry MUST
/// classify them `ReturnTag::Concrete(TypeTag::Bool)` — never `ReturnTag::Fresh`.
/// Pins the correct classification directly and rejects a regression that
/// re-adds them to the `Fresh`-return set (which would produce an unconstrained
/// type variable for a method that always returns bool).
#[test]
fn all_any_quantifiers_return_concrete_bool() {
    use ori_registry::find_method;

    for tag in [TypeTag::List, TypeTag::Iterator] {
        for name in ["all", "any"] {
            let def = find_method(tag, name)
                .unwrap_or_else(|| panic!("{tag:?}.{name} must be registered"));
            assert_eq!(
                def.returns,
                ReturnTag::Concrete(TypeTag::Bool),
                "{tag:?}.{name} must return Concrete(Bool), not Fresh"
            );
            assert_ne!(
                def.returns,
                ReturnTag::Fresh,
                "{tag:?}.{name} must NOT be ReturnTag::Fresh"
            );
        }
    }
}

/// Verify `range_method_requires_iteration` correctly identifies Range iteration methods.
#[test]
fn range_iteration_methods_derived_from_registry() {
    // These methods should require iteration (return types involve element projection)
    assert!(
        super::range_method_requires_iteration("iter"),
        "iter should require iteration"
    );
    assert!(
        super::range_method_requires_iteration("collect"),
        "collect should require iteration"
    );
    assert!(
        super::range_method_requires_iteration("to_list"),
        "to_list should require iteration"
    );

    // These methods should NOT require iteration
    assert!(
        !super::range_method_requires_iteration("len"),
        "len should not require iteration"
    );
    assert!(
        !super::range_method_requires_iteration("contains"),
        "contains should not require iteration"
    );
    assert!(
        !super::range_method_requires_iteration("count"),
        "count should not require iteration"
    );
    assert!(
        !super::range_method_requires_iteration("step_by"),
        "step_by should not require iteration"
    );
    assert!(
        !super::range_method_requires_iteration("is_empty"),
        "is_empty should not require iteration"
    );
}

// IndexSet `updated` resolution

/// Registry resolution of `updated` (`IndexSet` trait) succeeds for `[int]` and
/// `{str: int}` with `Self`-typed returns. `[T, max N]` erases to `Tag::List`
/// at type resolution, so the List-tag row below IS the fixed-list path —
/// there is no separate fixed-list registration to resolve.
#[test]
fn updated_resolves_on_list_and_map_with_self_return() {
    use super::resolve_builtin_method;
    use crate::{Idx, Pool, Tag};

    let mut pool = Pool::new();
    let mut engine = crate::InferEngine::new(&mut pool);

    // [int].updated -> [int] (SelfType return = receiver)
    let list_int = engine.pool_mut().list(Idx::INT);
    let list_ret = resolve_builtin_method(&mut engine, list_int, Tag::List, "updated");
    assert_eq!(
        list_ret,
        Some(list_int),
        "[int].updated must resolve to the receiver type (Self)"
    );

    // [str].updated -> [str] (element type flows through unchanged)
    let list_str = engine.pool_mut().list(Idx::STR);
    let list_str_ret = resolve_builtin_method(&mut engine, list_str, Tag::List, "updated");
    assert_eq!(
        list_str_ret,
        Some(list_str),
        "[str].updated must resolve to the receiver type (Self)"
    );

    // {str: int}.updated -> {str: int} (SelfType return = receiver)
    let map_str_int = engine.pool_mut().map(Idx::STR, Idx::INT);
    let map_ret = resolve_builtin_method(&mut engine, map_str_int, Tag::Map, "updated");
    assert_eq!(
        map_ret,
        Some(map_str_int),
        "{{str: int}}.updated must resolve to the receiver type (Self)"
    );
}

/// `updated` carries the correct key/value parameter types in the registry:
/// `(index: int, value: T)` for lists, `(key: K, value: V)` for maps.
#[test]
fn updated_registry_params_carry_key_and_value_types() {
    use ori_registry::find_method;

    let list_def = find_method(TypeTag::List, "updated")
        .unwrap_or_else(|| panic!("List.updated must be registered"));
    assert_eq!(list_def.trait_name, Some("IndexSet"));
    assert_eq!(list_def.params[0].ty, ReturnTag::Concrete(TypeTag::Int));
    assert_eq!(list_def.params[1].ty, ReturnTag::ElementType);
    assert_eq!(list_def.returns, ReturnTag::SelfType);

    let map_def = find_method(TypeTag::Map, "updated")
        .unwrap_or_else(|| panic!("Map.updated must be registered"));
    assert_eq!(map_def.trait_name, Some("IndexSet"));
    assert_eq!(map_def.params[0].ty, ReturnTag::KeyType);
    assert_eq!(map_def.params[1].ty, ReturnTag::ValueType);
    assert_eq!(map_def.returns, ReturnTag::SelfType);
}

/// Negative pin: `updated` does NOT resolve on types that implement `Index`
/// but not `IndexSet` (str), nor on non-indexable primitives (int).
#[test]
fn updated_does_not_resolve_on_str_or_int() {
    use super::resolve_builtin_method;
    use crate::{Idx, Pool, Tag};

    assert!(
        ori_registry::find_method(TypeTag::Str, "updated").is_none(),
        "str implements Index, NOT IndexSet — no registry entry"
    );
    assert!(
        ori_registry::find_method(TypeTag::Int, "updated").is_none(),
        "int is not indexable — no registry entry"
    );

    let mut pool = Pool::new();
    let mut engine = crate::InferEngine::new(&mut pool);
    assert_eq!(
        resolve_builtin_method(&mut engine, Idx::STR, Tag::Str, "updated"),
        None,
        "str.updated must not resolve"
    );
    assert_eq!(
        resolve_builtin_method(&mut engine, Idx::INT, Tag::Int, "updated"),
        None,
        "int.updated must not resolve"
    );
}

// Computed returns verification

/// Verify `resolve_computed_return` produces structured types (not bare `fresh_var`)
/// for methods that require specific type construction.
///
/// This catches regressions where `computed_returns.rs` branches are removed
/// or the dispatch is broken, even if the Fresh-return method list stays the same.
#[test]
fn computed_returns_produce_structured_types() {
    use super::computed_returns::resolve_computed_return;
    use crate::{Idx, Pool, Tag};

    let mut pool = Pool::new();
    let mut engine = crate::InferEngine::new(&mut pool);

    // List.zip should return List<(T, U)>, not bare fresh
    let list_int = engine.pool_mut().list(Idx::INT);
    let zip_ret = resolve_computed_return(&mut engine, list_int, Tag::List, "zip");
    assert_eq!(
        engine.pool().tag(zip_ret),
        Tag::List,
        "List.zip should return a List, not a bare type variable"
    );

    // Iterator.map should return Iterator<U>, not bare fresh
    let iter_int = engine.pool_mut().iterator(Idx::INT);
    let map_ret = resolve_computed_return(&mut engine, iter_int, Tag::Iterator, "map");
    assert_eq!(
        engine.pool().tag(map_ret),
        Tag::Iterator,
        "Iterator.map should return an Iterator, not a bare type variable"
    );

    // DEI.map should return DEI<U>, preserving DEI-ness
    let dei_int = engine.pool_mut().double_ended_iterator(Idx::INT);
    let dei_map_ret =
        resolve_computed_return(&mut engine, dei_int, Tag::DoubleEndedIterator, "map");
    assert_eq!(
        engine.pool().tag(dei_map_ret),
        Tag::DoubleEndedIterator,
        "DEI.map should return a DoubleEndedIterator"
    );

    // Iterator.zip should return Iterator<(T, U)>
    let zip_ret = resolve_computed_return(&mut engine, iter_int, Tag::Iterator, "zip");
    assert_eq!(
        engine.pool().tag(zip_ret),
        Tag::Iterator,
        "Iterator.zip should return an Iterator"
    );

    // Iterator.flatten should return Iterator<U>
    let flatten_ret = resolve_computed_return(&mut engine, iter_int, Tag::Iterator, "flatten");
    assert_eq!(
        engine.pool().tag(flatten_ret),
        Tag::Iterator,
        "Iterator.flatten should return an Iterator"
    );

    // Result.trace_entries pins [TraceEntry] (Tag::List, Named element) via the structured
    // computed_trace_entries() path (interner set); guards the trace_entries branch.
    let interner = ori_ir::StringInterner::new();
    engine.set_interner(&interner);

    let result_ok = engine.pool_mut().result(Idx::INT, Idx::STR);
    let trace_ret = resolve_computed_return(&mut engine, result_ok, Tag::Result, "trace_entries");
    assert_eq!(
        engine.pool().tag(trace_ret),
        Tag::List,
        "Result.trace_entries must return [TraceEntry] (List), not a bare type variable"
    );
    // Verify the element is Named (i.e., TraceEntry, not a fresh var)
    let trace_elem = engine.pool().list_elem(trace_ret);
    assert_eq!(
        engine.pool().tag(trace_elem),
        Tag::Named,
        "Result.trace_entries element must be Named (TraceEntry), not Var"
    );

    // Error.trace_entries exercises the same branch — must also return [TraceEntry]
    let error_ret = resolve_computed_return(&mut engine, Idx::ERROR, Tag::Error, "trace_entries");
    assert_eq!(
        engine.pool().tag(error_ret),
        Tag::List,
        "Error.trace_entries must return [TraceEntry] (List), not a bare type variable"
    );
    let error_trace_elem = engine.pool().list_elem(error_ret);
    assert_eq!(
        engine.pool().tag(error_trace_elem),
        Tag::Named,
        "Error.trace_entries element must be Named (TraceEntry), not Var"
    );

    // Verify unhandled methods still produce fresh Var — proves trace_entries
    // dispatch is reached (not a no-op that returns fresh for everything).
    let other_ret = resolve_computed_return(&mut engine, result_ok, Tag::Result, "unhandled_xyz");
    assert_eq!(
        engine.pool().tag(other_ret),
        Tag::Var,
        "Unhandled Result method should return fresh Var"
    );

    // Unhandled methods should return Var (fresh)
    let fold_ret = resolve_computed_return(&mut engine, list_int, Tag::List, "fold");
    assert_eq!(
        engine.pool().tag(fold_ret),
        Tag::Var,
        "List.fold should return a fresh type variable"
    );
}

/// `Error.trace_entries()` on a NAMED error-struct receiver (`Tag::Named`, the
/// interned `error_struct_idx`) MUST resolve to `[TraceEntry]` (a `Tag::List`
/// with a `Tag::Named` element), NOT poison to `Idx::ERROR`.
///
/// This drives the REAL production dispatch path: `resolve_builtin_method`
/// short-circuits every `Tag::Named` receiver to `resolve_named_type_method`,
/// which never consults the `Error` behavior table, so `trace_entries()` on a
/// Named-`Error` receiver returns `None` and codegen later poisons it. The
/// sibling test `computed_returns_produce_structured_types` exercises the
/// PRIMITIVE `Tag::Error` slot (`resolve_computed_return(.., Idx::ERROR,
/// Tag::Error, ..)`) — a path the production error-struct receiver never takes,
/// so it stays green while the real path poisons. This test pins the real path.
#[test]
fn trace_entries_on_named_error_struct_resolves_to_trace_entry_list() {
    use super::resolve_builtin_method;
    use crate::{Idx, Pool, Tag};

    let mut pool = Pool::new();
    let mut engine = crate::InferEngine::new(&mut pool);
    let interner = ori_ir::StringInterner::new();
    engine.set_interner(&interner);

    // Register the user-facing `Error` struct as a Named type and record it as
    // the error-struct SSOT — mirrors `register_error_type` (Pass 0a).
    let error_name = engine
        .intern_name("Error")
        .unwrap_or_else(|| panic!("interner set — Error must intern"));
    let error_struct_idx = engine.pool_mut().named(error_name);
    engine.pool_mut().set_error_struct_idx(error_struct_idx);
    assert!(
        engine.pool().is_error_struct(error_struct_idx),
        "error_struct_idx must be recorded as the SSOT error struct"
    );

    // The production receiver for `Err(e) -> e.trace_entries()`: a Named
    // error-struct receiver with Tag::Named.
    let ret = resolve_builtin_method(&mut engine, error_struct_idx, Tag::Named, "trace_entries")
        .unwrap_or_else(|| {
            panic!(
                "Error.trace_entries() on a Named error-struct receiver must resolve \
             to [TraceEntry]; got None — the Tag::Named short-circuit poisoned it \
             (resolve_named_type_method never consults the Error behavior table)"
            )
        });

    assert_eq!(
        engine.pool().tag(ret),
        Tag::List,
        "Error.trace_entries() must return [TraceEntry] (a List), not a poison/scalar type"
    );
    let elem = engine.pool().list_elem(ret);
    assert_eq!(
        engine.pool().tag(elem),
        Tag::Named,
        "Error.trace_entries() element must be Named (TraceEntry), not Var/Error"
    );
    let elem_name = engine.pool().named_name(elem);
    assert_eq!(
        engine.lookup_name(elem_name),
        Some("TraceEntry"),
        "Error.trace_entries() element must resolve to the interned name \
         \"TraceEntry\", not merely any Named type"
    );
    assert_ne!(
        ret,
        Idx::ERROR,
        "Error.trace_entries() must not poison to Idx::ERROR"
    );
}

/// Result closure-methods return a structured `Result<_, _>` (BD-2 propagation):
/// `map`/`and_then` transform Ok (Err preserved); `map_err`/`or_else` transform
/// Err (Ok preserved). A `Tag::Error` poison receiver falls through to a fresh
/// Var (the family helper guards on `Tag::Result`).
#[test]
fn result_closure_methods_produce_structured_returns() {
    use super::computed_returns::resolve_computed_return;
    use crate::{Idx, Pool, Tag};

    let mut pool = Pool::new();
    let mut engine = crate::InferEngine::new(&mut pool);
    // result_ok = Result<int, str>
    let result_ok = engine.pool_mut().result(Idx::INT, Idx::STR);

    let map_ret = resolve_computed_return(&mut engine, result_ok, Tag::Result, "map");
    assert_eq!(
        engine.pool().tag(map_ret),
        Tag::Result,
        "Result.map returns a structured Result<_, _>"
    );
    let map_ok = engine.pool().result_ok(map_ret);
    let map_err = engine.pool().result_err(map_ret);
    assert_eq!(
        engine.pool().tag(map_ok),
        Tag::Var,
        "Result.map Ok slot is fresh"
    );
    assert_eq!(map_err, Idx::STR, "Result.map preserves the Err type");

    let map_err_ret = resolve_computed_return(&mut engine, result_ok, Tag::Result, "map_err");
    assert_eq!(
        engine.pool().tag(map_err_ret),
        Tag::Result,
        "Result.map_err returns a structured Result<_, _>"
    );
    let merr_ok = engine.pool().result_ok(map_err_ret);
    let merr_err = engine.pool().result_err(map_err_ret);
    assert_eq!(merr_ok, Idx::INT, "Result.map_err preserves the Ok type");
    assert_eq!(
        engine.pool().tag(merr_err),
        Tag::Var,
        "Result.map_err Err slot is fresh"
    );

    let and_then_ret = resolve_computed_return(&mut engine, result_ok, Tag::Result, "and_then");
    assert_eq!(
        engine.pool().tag(and_then_ret),
        Tag::Result,
        "Result.and_then returns a structured Result<_, _>"
    );
    assert_eq!(
        engine.pool().result_err(and_then_ret),
        Idx::STR,
        "Result.and_then preserves the Err type"
    );

    let or_else_ret = resolve_computed_return(&mut engine, result_ok, Tag::Result, "or_else");
    assert_eq!(
        engine.pool().tag(or_else_ret),
        Tag::Result,
        "Result.or_else returns a structured Result<_, _>"
    );
    assert_eq!(
        engine.pool().result_ok(or_else_ret),
        Idx::INT,
        "Result.or_else preserves the Ok type"
    );

    // A Tag::Error poison receiver must NOT extract slots (result_ok/result_err
    // assert Tag::Result) — it falls through to a fresh Var.
    let err_map_ret = resolve_computed_return(&mut engine, Idx::ERROR, Tag::Error, "map");
    assert_eq!(
        engine.pool().tag(err_map_ret),
        Tag::Var,
        "Result-family method on a Tag::Error receiver falls through to fresh Var"
    );
}

/// Register the user-facing `Error` struct as a Named type and record it as
/// the error-struct SSOT, mirroring `register_error_type` (Pass 0a). Returns
/// `(pool, engine, error_struct_idx)` — callers pin `engine`/`pool` via a
/// local `let mut` per the borrow shape `InferEngine::new` requires.
fn setup_error_struct_pool() -> (crate::Pool, crate::Idx) {
    use crate::Pool;

    let mut pool = Pool::new();
    let error_name = {
        let interner = ori_ir::StringInterner::new();
        let mut engine = crate::InferEngine::new(&mut pool);
        engine.set_interner(&interner);
        engine
            .intern_name("Error")
            .unwrap_or_else(|| panic!("interner set — Error must intern"))
    };
    let error_struct_idx = pool.named(error_name);
    pool.set_error_struct_idx(error_struct_idx);
    (pool, error_struct_idx)
}

/// `Pool::is_error_struct_receiver` MUST chase a `Tag::Var` linked (via
/// unification) to `error_struct_idx` and accept it — the var-link-chase leg
/// of the dual-identity guard (fix-consensus refinement #2 + #8).
///
/// Drives `Pool::is_error_struct_receiver` DIRECTLY (not thru
/// `resolve_builtin_method`) — driving this thru `resolve_builtin_method`
/// would require an internally-inconsistent `(linked_idx, Tag::Named)` pair
/// no real call site ever produces (the sole production caller always
/// resolves the var BEFORE deriving `tag` from the post-resolve idx).
#[test]
fn is_error_struct_receiver_chases_var_link_to_error_struct_idx() {
    use crate::Tag;

    let (mut pool, error_struct_idx) = setup_error_struct_pool();
    let linked_idx = {
        let mut engine = crate::InferEngine::new(&mut pool);
        let fresh = engine.fresh_var();
        engine
            .unify()
            .unify(fresh, error_struct_idx)
            .unwrap_or_else(|e| panic!("unify(fresh_var, error_struct_idx) must succeed: {e:?}"));
        fresh
    };
    assert_eq!(
        pool.tag(linked_idx),
        Tag::Var,
        "linked_idx must still be a Tag::Var wrapper (VarState::Link) before chasing"
    );
    assert!(
        pool.is_error_struct_receiver(linked_idx),
        "is_error_struct_receiver must chase the Tag::Var link to error_struct_idx and accept it"
    );
}

/// A newtype over `Error` (`type MyError = Error`) MUST NOT be hijacked into
/// `ERROR_METHODS` — `trace_entries()` on the newtype's OWN distinct `Named`
/// idx must fall through to `resolve_named_type_method` unchanged (returns
/// `None`, exactly as pre-fix), preserving newtype nominal typing: a newtype
/// inherits zero trait/method behavior from its underlying type.
///
/// Registers the newtype via `Pool::set_resolution` — the SAME mechanism
/// `check/registration/user_types.rs` uses for `type N = Existing` — so this
/// pin clamps the guard against the REAL newtype-registration shape, not a
/// synthetic approximation.
#[test]
fn trace_entries_on_newtype_over_error_is_not_hijacked() {
    use super::resolve_builtin_method;
    use crate::Tag;

    let (mut pool, error_struct_idx) = setup_error_struct_pool();
    let myerror_name = {
        let interner = ori_ir::StringInterner::new();
        let mut engine = crate::InferEngine::new(&mut pool);
        engine.set_interner(&interner);
        engine
            .intern_name("MyError")
            .unwrap_or_else(|| panic!("interner set — MyError must intern"))
    };
    let myerror_idx = pool.named(myerror_name);
    pool.set_resolution(myerror_idx, error_struct_idx);
    assert!(
        !pool.is_error_struct_receiver(myerror_idx),
        "a newtype's own distinct Named idx must NOT match is_error_struct_receiver \
         (chase_var_links never crosses the resolutions map)"
    );

    let interner = ori_ir::StringInterner::new();
    let mut engine = crate::InferEngine::new(&mut pool);
    engine.set_interner(&interner);
    let ret = resolve_builtin_method(&mut engine, myerror_idx, Tag::Named, "trace_entries");
    assert_eq!(
        ret, None,
        "trace_entries() on a newtype-over-Error receiver must resolve via \
         resolve_named_type_method unchanged (None for the unknown method), \
         NOT via ERROR_METHODS — the newtype inherits zero Error methods"
    );
}

/// A different `Tag::Named` struct (NOT the registered error struct) calling
/// `trace_entries` (a method NOT in its own behavior table) MUST resolve via
/// `resolve_named_type_method` unchanged (`None`), exactly as pre-fix — the
/// guard specificity clamp proving the new registry route stays Error-only
/// and does not hijack every `Tag::Named` receiver.
#[test]
fn trace_entries_on_non_error_named_struct_is_not_hijacked() {
    use super::resolve_builtin_method;
    use crate::{Pool, Tag};

    let mut pool = Pool::new();
    let point_name = {
        let interner = ori_ir::StringInterner::new();
        let mut engine = crate::InferEngine::new(&mut pool);
        engine.set_interner(&interner);
        engine
            .intern_name("Point")
            .unwrap_or_else(|| panic!("interner set — Point must intern"))
    };
    let point_idx = pool.named(point_name);
    // NOTE: no set_error_struct_idx call — Point is a plain user struct.

    let interner = ori_ir::StringInterner::new();
    let mut engine = crate::InferEngine::new(&mut pool);
    engine.set_interner(&interner);
    let ret = resolve_builtin_method(&mut engine, point_idx, Tag::Named, "trace_entries");
    assert_eq!(
        ret, None,
        "trace_entries() on a non-Error Named struct must resolve via \
         resolve_named_type_method unchanged (None) — the guard is gated on \
         is_error_struct_receiver, not on bare Tag::Named"
    );
}

/// `message` (method-CALL syntax `e.message()`, a `MethodCall` AST node —
/// DISTINCT from field access `e.message`, which routes via `infer_field` and
/// never reaches `resolve_builtin_method`) on the Named error-struct receiver
/// MUST STAY poisoned to `None` on BOTH sides of this fix: `message`'s
/// registry entry carries `backend_required: false` (no codegen accessor
/// exists), so the allow-list — derived dynamically from that SAME field —
/// correctly excludes it. Clamps the allow-list boundary; the poison itself
/// is a genuinely separate, orthogonal gap tracked as BUG-02-093.
#[test]
fn message_call_on_named_error_struct_stays_poisoned() {
    use super::resolve_builtin_method;
    use crate::Tag;

    let (mut pool, error_struct_idx) = setup_error_struct_pool();
    let interner = ori_ir::StringInterner::new();
    let mut engine = crate::InferEngine::new(&mut pool);
    engine.set_interner(&interner);

    let method_def = ori_registry::find_method(ori_registry::TypeTag::Error, "message")
        .unwrap_or_else(|| panic!("Error.message must exist in the registry"));
    assert!(
        !method_def.backend_required,
        "Error.message must stay backend_required: false (no codegen accessor) \
         for this pin's allow-list-exclusion claim to hold"
    );

    let ret = resolve_builtin_method(&mut engine, error_struct_idx, Tag::Named, "message");
    assert_eq!(
        ret, None,
        "message()-CALL on a Named error-struct receiver must stay poisoned \
         (None) — backend_required: false excludes it from the allow-list, \
         identical to pre-fix"
    );
}

/// `Error.trace()` on the Named error-struct receiver MUST resolve to `str`
/// (`Idx::STR`), NOT `Idx::ERROR` — the non-scalar (fat-pointer) Traceable
/// accessor sibling of `trace_entries`, at genuine risk of the SAME poison
/// class this bug's root cause produces.
#[test]
fn trace_on_named_error_struct_resolves_to_str() {
    use super::resolve_builtin_method;
    use crate::{Idx, Tag};

    let (mut pool, error_struct_idx) = setup_error_struct_pool();
    let interner = ori_ir::StringInterner::new();
    let mut engine = crate::InferEngine::new(&mut pool);
    engine.set_interner(&interner);

    let ret = resolve_builtin_method(&mut engine, error_struct_idx, Tag::Named, "trace")
        .unwrap_or_else(|| panic!("Error.trace() on a Named error-struct receiver must resolve"));
    assert_eq!(
        ret,
        Idx::STR,
        "Error.trace() must resolve to str (Idx::STR), not Idx::ERROR"
    );
}

/// `Error.with_trace(entry)` on the Named error-struct receiver MUST resolve
/// to `Self` (the error-struct `Idx`), NOT `Idx::ERROR`.
#[test]
fn with_trace_on_named_error_struct_resolves_to_self() {
    use super::resolve_builtin_method;
    use crate::Tag;

    let (mut pool, error_struct_idx) = setup_error_struct_pool();
    let interner = ori_ir::StringInterner::new();
    let mut engine = crate::InferEngine::new(&mut pool);
    engine.set_interner(&interner);

    let ret = resolve_builtin_method(&mut engine, error_struct_idx, Tag::Named, "with_trace")
        .unwrap_or_else(|| {
            panic!("Error.with_trace() on a Named error-struct receiver must resolve")
        });
    assert_eq!(
        ret, error_struct_idx,
        "Error.with_trace() must resolve to Self (the error-struct Idx), not Idx::ERROR"
    );
}

/// `Error.clone()` on the Named error-struct receiver MUST resolve to `Self`
/// (the error-struct `Idx`) — registry-first routing gives `clone` correct
/// resolution for the FIRST time (pre-fix it poisons to `Idx::ERROR`
/// identically to `trace_entries`, since `resolve_named_type_method` handles
/// only `unwrap`/`inner`/`value`/`to_str`/`debug` and `register_error_type`
/// registers no derive/impl for `clone`). Positive+negative pairing: this
/// pin's pre-fix RED state (asserted here as the POST-fix GREEN) documents
/// the corrected divergence (broken -> correct), not a regression.
#[test]
fn clone_on_named_error_struct_resolves_to_self() {
    use super::resolve_builtin_method;
    use crate::Tag;

    let (mut pool, error_struct_idx) = setup_error_struct_pool();
    let interner = ori_ir::StringInterner::new();
    let mut engine = crate::InferEngine::new(&mut pool);
    engine.set_interner(&interner);

    let ret = resolve_builtin_method(&mut engine, error_struct_idx, Tag::Named, "clone")
        .unwrap_or_else(|| panic!("Error.clone() on a Named error-struct receiver must resolve"));
    assert_eq!(
        ret, error_struct_idx,
        "Error.clone() must resolve to Self (the error-struct Idx), not Idx::ERROR"
    );
}
