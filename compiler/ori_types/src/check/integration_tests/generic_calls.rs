use super::support::*;

// Collect trait — bidirectional type inference

fn assert_collect_body_tag(source: &str, function: &str, expected: Tag) {
    let result = check_source(source);
    assert!(
        !result.has_errors(),
        "collect() should not error: {:?}",
        result.error_kinds()
    );
    let ty = result
        .function_body_type(function)
        .unwrap_or_else(|| panic!("missing body type for {function}"));
    assert_eq!(result.tag(ty), expected, "unexpected collect body type");
}

#[test]
fn collect_to_set_via_return_type() {
    // Return type `Set<int>` should guide `collect()` to produce Set
    assert_collect_body_tag(
        include_str!("../fixtures/integration/collect_to_set_via_return_type.ori"),
        "to_set",
        Tag::Set,
    );
}

#[test]
fn collect_to_list_by_default() {
    // No Set annotation — collect() should default to list
    assert_collect_body_tag(
        include_str!("../fixtures/integration/collect_to_list_by_default.ori"),
        "to_list",
        Tag::List,
    );
}

#[test]
fn collect_to_set_via_let_binding() {
    // Let binding with Set<int> annotation should guide collect()
    let source = include_str!("../fixtures/integration/collect_to_set_via_let_binding.ori");
    let result = check_source(source);
    assert!(
        !result.has_errors(),
        "collect() via let binding with Set<int> should not error: {:?}",
        result.error_kinds()
    );
}

#[test]
fn collect_chained_adapters_to_set() {
    // Chained adapters (filter) before collect should preserve Set inference
    assert_collect_body_tag(
        include_str!("../fixtures/integration/collect_chained_adapters_to_set.ori"),
        "filtered",
        Tag::Set,
    );
}

// Monomorphization Instance Recording

#[test]
fn generic_identity_records_mono_instance() {
    let source = include_str!("../fixtures/integration/generic_identity_records_mono_instance.ori");
    let result = check_source(source);
    assert!(
        !result.has_errors(),
        "should not error: {:?}",
        result.error_kinds()
    );

    let instances = result.mono_instances_for("identity");
    assert!(
        !instances.is_empty(),
        "identity called with int should record a mono instance"
    );
    // The concrete arg should be int.
    assert_eq!(instances[0].concrete_param_types.len(), 1);
    assert_eq!(instances[0].concrete_param_types[0], Idx::INT);
    assert_eq!(instances[0].concrete_return_type, Idx::INT);
}

#[test]
fn generic_two_param_records_mono_instance() {
    let source =
        include_str!("../fixtures/integration/generic_two_param_records_mono_instance.ori"); // Needs r#"..."# because of the " in "hello"
    let result = check_source(source);
    assert!(
        !result.has_errors(),
        "should not error: {:?}",
        result.error_kinds()
    );

    let instances = result.mono_instances_for("pair");
    assert!(
        !instances.is_empty(),
        "pair called with (int, str) should record a mono instance"
    );
    assert_eq!(instances[0].concrete_param_types.len(), 2);
    assert_eq!(instances[0].concrete_param_types[0], Idx::INT);
    assert_eq!(instances[0].concrete_param_types[1], Idx::STR);
}

#[test]
fn non_generic_call_records_nothing() {
    let source = include_str!("../fixtures/integration/non_generic_call_records_nothing.ori");
    let result = check_source(source);
    assert!(
        !result.has_errors(),
        "should not error: {:?}",
        result.error_kinds()
    );

    let instances = result.mono_instances_for("add");
    assert!(
        instances.is_empty(),
        "non-generic function should not produce mono instances"
    );
}

#[test]
fn same_generic_call_twice_deduplicates() {
    let source = include_str!("../fixtures/integration/same_generic_call_twice_deduplicates.ori");
    let result = check_source(source);
    assert!(
        !result.has_errors(),
        "should not error: {:?}",
        result.error_kinds()
    );

    let instances = result.mono_instances_for("identity");
    // Both calls use int, so dedup should give exactly one instance.
    assert_eq!(
        instances
            .iter()
            .filter(|m| m.concrete_param_types[0] == Idx::INT)
            .count(),
        1,
        "same generic args should dedup to one instance"
    );
}

#[test]
fn different_type_args_produce_separate_instances() {
    let source =
        include_str!("../fixtures/integration/different_type_args_produce_separate_instances.ori");
    let result = check_source(source);
    assert!(
        !result.has_errors(),
        "should not error: {:?}",
        result.error_kinds()
    );

    let instances = result.mono_instances_for("identity");
    // Should have exactly 2 distinct instances: one for int, one for str.
    assert_eq!(
        instances.len(),
        2,
        "identity<int> and identity<str> should produce 2 instances, got: {instances:?}"
    );
    let types: Vec<Idx> = instances
        .iter()
        .map(|m| m.concrete_param_types[0])
        .collect();
    assert!(types.contains(&Idx::INT), "should have int instance");
    assert!(types.contains(&Idx::STR), "should have str instance");
}

// Method Monomorphization — Inherent Methods on Generic Receivers
//
// An inherent method on a generic impl (`impl<T> Box<T> { @unwrap (self) -> T }`)
// called on a concretely-instantiated receiver (`Box<int>`) records a
// receiver-bearing MonoInstance so codegen can monomorphize it. The instance
// carries `receiver_type = Some(Box<int>)` and `impl_args = [int]`.

#[test]
fn inherent_method_on_generic_receiver_records_method_instance() {
    let source = include_str!(
        "../fixtures/integration/inherent_method_on_generic_receiver_records_method_instance.ori"
    );
    let result = check_source(source);
    assert!(
        !result.has_errors(),
        "should not error: {:?}",
        result.error_kinds()
    );

    let instances = result.mono_instances_for("unwrap");
    assert_eq!(
        instances.len(),
        1,
        "Box<int>.unwrap() should record exactly one method instance, got: {instances:?}"
    );
    let inst = instances[0];
    assert!(
        inst.receiver_type.is_some(),
        "method instance must carry receiver_type, got None"
    );
    assert_eq!(
        inst.impl_args,
        vec![crate::GenericArg::Type(Idx::INT)],
        "impl_args should be [int] for Box<int>.unwrap()"
    );
    assert!(
        inst.method_args.is_empty(),
        "unwrap has no method-level generics, got: {:?}",
        inst.method_args
    );
    assert_eq!(
        inst.concrete_return_type,
        Idx::INT,
        "unwrap on Box<int> returns int"
    );
}

#[test]
fn same_method_on_same_receiver_deduplicates() {
    let source =
        include_str!("../fixtures/integration/same_method_on_same_receiver_deduplicates.ori");
    let result = check_source(source);
    assert!(
        !result.has_errors(),
        "should not error: {:?}",
        result.error_kinds()
    );

    let instances = result.mono_instances_for("unwrap");
    assert_eq!(
        instances.len(),
        1,
        "two Box<int>.unwrap() calls should dedup to one instance, got: {instances:?}"
    );
}

#[test]
fn method_on_distinct_receivers_produces_separate_instances() {
    let source = include_str!(
        "../fixtures/integration/method_on_distinct_receivers_produces_separate_instances.ori"
    );
    let result = check_source(source);
    assert!(
        !result.has_errors(),
        "should not error: {:?}",
        result.error_kinds()
    );

    let instances = result.mono_instances_for("unwrap");
    assert_eq!(
        instances.len(),
        2,
        "Box<int>.unwrap() and Box<str>.unwrap() should produce 2 instances, got: {instances:?}"
    );
    let impl_args: Vec<&Vec<crate::GenericArg>> = instances.iter().map(|m| &m.impl_args).collect();
    assert!(
        impl_args.contains(&&vec![crate::GenericArg::Type(Idx::INT)]),
        "expected an [int] impl_args instance, got: {impl_args:?}"
    );
    assert!(
        impl_args.contains(&&vec![crate::GenericArg::Type(Idx::STR)]),
        "expected a [str] impl_args instance, got: {impl_args:?}"
    );
}

/// Regression: a generic-receiver inherent method on a nested-generic receiver
/// (`Box<[int]>`, whose type argument is itself a generic) records a `MonoInstance`
/// whose `impl_args` carries the full nested type — distinct from scalar `Box<int>`.
/// A recorder that collapsed the nested `[int]` to the generic shell would dedup
/// the two receivers into one instance, re-surfacing the missing-mono condition.
#[test]
fn method_on_nested_generic_receiver_records_distinct_instance() {
    let source = include_str!(
        "../fixtures/integration/method_on_nested_generic_receiver_records_distinct_instance.ori"
    );
    let result = check_source(source);
    assert!(
        !result.has_errors(),
        "should not error: {:?}",
        result.error_kinds()
    );

    let instances = result.mono_instances_for("unwrap");
    assert_eq!(
        instances.len(),
        2,
        "Box<int>.unwrap() and Box<[int]>.unwrap() should produce 2 distinct instances, got: {instances:?}"
    );
    // Exactly one instance is the scalar `Box<int>` ([int] impl_args); the other
    // is the nested-generic `Box<[int]>`, whose impl_args is NOT [int]. Equal
    // impl_args would mean the recorder collapsed the nested generic to the shell.
    assert_ne!(
        instances[0].impl_args,
        instances[1].impl_args,
        "nested-generic Box<[int]> and scalar Box<int> must record distinct impl_args, got: {:?}",
        instances.iter().map(|m| &m.impl_args).collect::<Vec<_>>()
    );
    let scalar_count = instances
        .iter()
        .filter(|m| m.impl_args == vec![crate::GenericArg::Type(Idx::INT)])
        .count();
    assert_eq!(
        scalar_count, 1,
        "exactly one instance is the scalar Box<int> ([int] impl_args); the nested Box<[int]> carries the list type, got: {:?}",
        instances.iter().map(|m| &m.impl_args).collect::<Vec<_>>()
    );
}

#[test]
fn inherent_method_on_non_generic_receiver_records_nothing() {
    // The impl is NOT generic over the receiver's type params, so the method
    // call must emit no method MonoInstance (the additive scope guard leaves
    // non-generic inherent dispatch untouched).
    let source = include_str!(
        "../fixtures/integration/inherent_method_on_non_generic_receiver_records_nothing.ori"
    );
    let result = check_source(source);
    assert!(
        !result.has_errors(),
        "should not error: {:?}",
        result.error_kinds()
    );

    let instances = result.mono_instances_for("get");
    assert!(
        instances.is_empty(),
        "non-generic-receiver inherent method should record no instance, got: {instances:?}"
    );
}
