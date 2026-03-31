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
