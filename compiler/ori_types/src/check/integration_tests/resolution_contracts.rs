use super::support::*;

// Map-iterator lambda-parameter inference pins. `kvs.iter()` yields
// Iterator<(K, V)>, so `.map(...)` routes the `is_iterator()` branch of
// `unify_closure_param_with_iterator_elem`. Registry higher-order methods
// dispatch through the iterator receiver rather than a bare Map receiver.

#[test]
fn map_iterator_lambda_param_uses_iterator_element_tuple() {
    let result = check_source(fixture_without_trailing_newline(include_str!(
        "../fixtures/integration/lambda_param_from_map_iter_iterator_elem_unchanged.ori"
    )));
    assert!(
        !result.has_errors(),
        "map-iterator receiver must infer kv: (str, int); kinds: {:?}",
        result.error_kinds()
    );
}

#[test]
fn list_iterator_lambda_param_avoids_map_projection() {
    // Negative pin: a list-iterator receiver
    // still routes through the `is_iterator()` branch — widening the Map
    // arm to match `Tag::Iterator` would project map_key/map_value on a
    // non-Map idx and break this inference.
    let result = check_source(fixture_without_trailing_newline(include_str!(
        "../fixtures/integration/lambda_param_from_iterator_receiver_unchanged_by_map_arm.ori"
    )));
    assert!(
        !result.has_errors(),
        "iterator receiver inference must be unchanged by the Map arm; kinds: {:?}",
        result.error_kinds()
    );
}

#[test]
fn eager_direct_generic_param_records_complete_instance() {
    let source = include_str!(
        "../fixtures/integration/s09_2_eager_direct_param_records_complete_instance.ori"
    );
    let result = check_source(source);
    assert!(
        !result.has_errors(),
        "direct-param generic program must type-check; kinds: {:?}",
        result.error_kinds()
    );
    let instances = result.mono_instances_for("p1_id");
    assert_eq!(
        instances.len(),
        1,
        "eager direct-param `p1_id(x: 42)` must record exactly one instance, got: {instances:?}"
    );
    assert_eq!(instances[0].concrete_param_types, vec![Idx::INT]);
    assert_eq!(instances[0].concrete_return_type, Idx::INT);
}

// Pin 2 — eager indirect generic parameter (`T` occurs only inside
// `Pair<T, int>`) exercises `extract_indirect_scheme_var`.
#[test]
fn eager_nested_generic_param_records_complete_instance() {
    let source = include_str!(
        "../fixtures/integration/s09_2_eager_indirect_param_records_complete_instance.ori"
    );
    let result = check_source(source);
    assert!(
        !result.has_errors(),
        "indirect-param generic program must type-check; kinds: {:?}",
        result.error_kinds()
    );
    let instances = result.mono_instances_for("p2_firstof");
    assert_eq!(
        instances.len(),
        1,
        "eager indirect-param `p2_firstof` (T in Pair<T, int>) must record one instance, got: {instances:?}"
    );
    assert_eq!(
        instances[0].concrete_return_type,
        Idx::STR,
        "indirect T resolves to str"
    );
}

// Pin 3 — derived-Eq comparison on a GENERIC composite instantiation. Operator
// dispatch must publish the same concrete receiver-bearing method demand as an
// ordinary generic method call; ARC uses its receiver-qualified operator fact
// rather than a canonical call-dispatch entry to bind the realized body.
#[test]
fn derived_equality_on_generic_composite_typechecks_to_bool() {
    let source = include_str!(
        "../fixtures/integration/s09_2_derived_method_on_generic_composite_typechecks_to_bool.ori"
    );
    let result = check_source(source);
    assert!(
        !result.has_errors(),
        "derived-Eq generic-composite program must type-check; kinds: {:?}",
        result.error_kinds()
    );
    let body_ty = result.first_function_body_type().unwrap();
    assert_eq!(
        body_ty,
        Idx::BOOL,
        "derived-Eq `p == q` on P3Pair<int, str> must resolve to bool; got {body_ty:?}"
    );
    let receiver = result
        .find_applied("P3Pair", &[Idx::INT, Idx::STR])
        .expect("concrete P3Pair<int, str> must exist");
    let instances = result.mono_instances_for("eq");
    assert_eq!(
        instances.len(),
        1,
        "generic derived Eq operator must publish one deduplicated method instance: {instances:?}"
    );
    let instance = instances[0];
    assert_eq!(instance.receiver_type, Some(receiver));
    assert_eq!(
        instance.impl_args,
        vec![
            crate::GenericArg::Type(Idx::INT),
            crate::GenericArg::Type(Idx::STR),
        ]
    );
    assert_eq!(instance.concrete_param_types, vec![receiver]);
    assert_eq!(instance.concrete_return_type, Idx::BOOL);
}

#[test]
fn generic_derived_methods_share_exact_applied_self_identity() {
    let source = include_str!(
        "../fixtures/integration/generic_derived_methods_share_exact_applied_self_identity.ori"
    );
    let result = check_source(source);
    assert!(
        !result.has_errors(),
        "generic derived methods must resolve without poison; kinds: {:?}",
        result.error_kinds()
    );

    let clone_ty = result.function_body_type("p3_clone").unwrap();
    let expected = result
        .find_applied("P3Box", &[Idx::INT])
        .expect("concrete P3Box<int> must exist");
    assert_eq!(clone_ty, expected);
    let expected_str = result
        .find_applied("P3Box", &[Idx::STR])
        .expect("concrete P3Box<str> must exist");
    assert_eq!(
        result.function_body_type("p3_clone_str"),
        Some(expected_str)
    );
    assert_eq!(result.function_body_type("p3_hash"), Some(Idx::INT));

    let type_param = result.interner.intern("T");
    for accepted in &result.result.typed.accepted_derives {
        assert_eq!(result.tag(accepted.owner_type), Tag::Applied);
        assert_eq!(
            result.pool.applied_name(accepted.owner_type),
            result.interner.intern("P3Box")
        );
        let args = result.pool.applied_args(accepted.owner_type);
        assert_eq!(args.len(), 1);
        assert_eq!(result.tag(args[0]), Tag::Named);
        assert_eq!(result.pool.named_name(args[0]), type_param);
        assert_eq!(accepted.signature.type_params, vec![type_param]);
        assert!(accepted
            .signature
            .param_types
            .iter()
            .all(|&param| param == accepted.owner_type));
        if accepted.trait_kind == ori_ir::DerivedTrait::Clone {
            assert_eq!(accepted.signature.return_type, accepted.owner_type);
        }
    }
}

#[test]
fn concrete_generic_bound_seeds_exact_derived_method_body() {
    let source = include_str!(
        "../fixtures/integration/concrete_generic_bound_seeds_exact_derived_method_body.ori"
    );
    let result = check_source(source);
    assert!(
        !result.has_errors(),
        "bound-method generic program must type-check; kinds: {:?}",
        result.error_kinds()
    );

    let receiver = result
        .find_applied("BoundBox", &[Idx::INT])
        .expect("concrete BoundBox<int> must exist");
    let accepted = result
        .result
        .typed
        .accepted_derives
        .iter()
        .find(|accepted| accepted.trait_kind == ori_ir::DerivedTrait::Debug)
        .expect("Debug derive must be accepted");
    let producer = crate::MethodProducer::Derived(accepted.id);
    let instances: Vec<_> = result
        .result
        .typed
        .mono_instances
        .iter()
        .filter(|instance| {
            instance.method_producer.as_ref() == Some(&producer)
                && instance.receiver_type == Some(receiver)
        })
        .collect();

    assert_eq!(
        instances.len(),
        1,
        "the concrete bound must publish one exact derived Debug body: {instances:?}"
    );
    assert_eq!(
        instances[0].impl_args,
        vec![crate::GenericArg::Type(Idx::INT)]
    );
    assert!(result
        .result
        .typed
        .derived_call_plans
        .iter()
        .any(|plan| plan.derived == accepted.id && plan.binder_substitutions == [Idx::INT]));
}
