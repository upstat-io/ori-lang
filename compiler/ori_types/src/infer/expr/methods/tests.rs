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

    // Semantic pin: assert the exact set of Fresh-return methods.
    // If a new method with `ReturnTag::Fresh` is added, this test fails,
    // forcing the developer to either add a computed_returns entry or
    // explicitly document that a fresh var is the correct return.
    let names: Vec<String> = fresh_methods
        .iter()
        .map(|(tag, name)| format!("{tag:?}.{name}"))
        .collect();

    // Known Fresh-return methods (order matches BUILTIN_TYPES × sorted methods):
    let expected = [
        "Error.trace_entries",
        "List.all",
        "List.any",
        "List.chunk",
        "List.filter",
        "List.find",
        "List.flat_map",
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

    // Result.trace_entries — must return [TraceEntry] (Tag::List with Named element).
    // We set up a StringInterner so computed_trace_entries() takes the structured path
    // instead of falling back to fresh_var(). This pins the exact return type and
    // would detect deletion of the trace_entries branch in computed_returns.rs.
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

    // Unhandled Result method falls through to fresh
    let result_map_ret = resolve_computed_return(&mut engine, result_ok, Tag::Result, "map");
    assert_eq!(
        engine.pool().tag(result_map_ret),
        Tag::Var,
        "Result.map should return a fresh type variable"
    );
}
