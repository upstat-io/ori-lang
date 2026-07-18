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

#[test]
fn immutable_const_let_capacity_records_exact_method_instance() {
    let source = include_str!(
        "../fixtures/integration/const_let_fixed_list_capacity_records_method_instance.ori"
    );
    let result = check_source(source);
    assert!(
        !result.has_errors(),
        "immutable const capacity program must type-check; kinds: {:?}",
        result.error_kinds()
    );

    let instances = result.mono_instances_for("first_n");
    assert_eq!(
        instances.len(),
        1,
        "const-generic method call must publish one exact instance: {instances:?}"
    );
    assert_eq!(
        instances[0].method_args,
        vec![crate::GenericArg::Const(crate::ConstValue::Int(5))]
    );
    assert_eq!(
        instances[0].const_bindings,
        vec![crate::MonoConstBinding {
            name: result.interner.intern("N"),
            value: crate::ConstValue::Int(5),
        }]
    );
}

#[test]
fn fixed_list_capacity_rejects_undeclared_const_with_actionable_error() {
    let result = check_source(fixture_without_trailing_newline(include_str!(
        "../fixtures/integration/fixed_list_capacity_unresolved_const.ori"
    )));
    assert_eq!(
        result.error_count(),
        1,
        "one undeclared capacity const must produce one error: {:?}",
        result.error_kinds()
    );
    let error = &result.result.typed.errors[0];
    assert_eq!(format!("{:?}", error.code()), "E2056");
    let message = error.format_with(&result.pool, &result.interner);
    assert!(
        message.contains("undeclared fixed-list capacity const `$M`")
            && message.contains("declare it in `<$M: int>` or use a declared const"),
        "diagnostic must state the cause and actionable fix: {message}"
    );
}

#[test]
fn fixed_list_capacity_rejects_zero_and_negative_literals() {
    let result = check_source(fixture_without_trailing_newline(include_str!(
        "../fixtures/integration/fixed_list_capacity_non_positive.ori"
    )));
    assert_eq!(
        result.error_count(),
        2,
        "zero and negative capacities must each produce one error: {:?}",
        result.error_kinds()
    );
    let messages: Vec<_> = result
        .result
        .typed
        .errors
        .iter()
        .map(|error| {
            assert_eq!(format!("{:?}", error.code()), "E2057");
            error.format_with(&result.pool, &result.interner)
        })
        .collect();
    assert!(messages
        .iter()
        .any(|message| message.contains("supplied 0")));
    assert!(messages
        .iter()
        .any(|message| message.contains("supplied -1")));
}

#[test]
fn fixed_list_capacity_accepts_positive_and_declared_consts() {
    let result = check_source(fixture_without_trailing_newline(include_str!(
        "../fixtures/integration/fixed_list_capacity_valid.ori"
    )));
    assert!(
        !result.has_errors(),
        "positive literals and declared consts must remain valid: {:?}",
        result.error_kinds()
    );
}

#[test]
fn fixed_list_capacity_rejects_runtime_and_mutable_bindings() {
    let result = check_source(fixture_without_trailing_newline(include_str!(
        "../fixtures/integration/fixed_list_capacity_runtime_binding.ori"
    )));
    assert_eq!(
        result.error_count(),
        2,
        "runtime and mutable bindings must each be rejected: {:?}",
        result.error_kinds()
    );
    for error in &result.result.typed.errors {
        assert_eq!(format!("{:?}", error.code()), "E2056");
        let message = error.format_with(&result.pool, &result.interner);
        assert!(
            message.contains("declare it in `<$capacity: int>` or use a declared const"),
            "diagnostic must explain how to introduce compile-time evidence: {message}"
        );
    }
}

#[test]
fn fixed_list_capacity_rejects_non_integer_and_unsupported_expressions() {
    let result = check_source(fixture_without_trailing_newline(include_str!(
        "../fixtures/integration/fixed_list_capacity_invalid_expression.ori"
    )));
    assert_eq!(
        result.error_count(),
        3,
        "bool capacities and unsupported operators must be rejected: {:?}",
        result.error_kinds()
    );
    for error in &result.result.typed.errors {
        assert_eq!(format!("{:?}", error.code()), "E2059");
        let message = error.format_with(&result.pool, &result.interner);
        assert!(
            message.contains("fixed-list capacity must be an evaluable integer expression")
                && message.contains("use a positive integer literal"),
            "diagnostic must state the integer-expression contract and fix: {message}"
        );
    }
}

#[test]
fn fixed_list_capacity_evaluates_dependent_module_consts() {
    let result = check_source(fixture_without_trailing_newline(include_str!(
        "../fixtures/integration/fixed_list_capacity_module_values.ori"
    )));
    assert_eq!(
        result.error_count(),
        2,
        "dependent zero and negative module consts must be rejected: {:?}",
        result.error_kinds()
    );
    let messages: Vec<_> = result
        .result
        .typed
        .errors
        .iter()
        .map(|error| {
            assert_eq!(format!("{:?}", error.code()), "E2057");
            error.format_with(&result.pool, &result.interner)
        })
        .collect();
    assert!(messages
        .iter()
        .any(|message| message.contains("supplied 0")));
    assert!(messages
        .iter()
        .any(|message| message.contains("supplied -1")));
}

#[test]
fn fixed_list_capacity_evaluates_forward_module_const_dependencies() {
    let result = check_source(fixture_without_trailing_newline(include_str!(
        "../fixtures/integration/fixed_list_capacity_forward_module_const.ori"
    )));
    assert!(
        !result.has_errors(),
        "forward module const dependencies must be declaration-order independent: {:?}",
        result.error_kinds()
    );
}

#[test]
fn fixed_list_capacity_rejects_module_const_cycles_once() {
    let result = check_source(fixture_without_trailing_newline(include_str!(
        "../fixtures/integration/fixed_list_capacity_module_const_cycle.ori"
    )));
    assert_eq!(
        result.error_count(),
        1,
        "a const cycle used as a capacity must produce one focused error: {:?}",
        result.error_kinds()
    );
    let error = &result.result.typed.errors[0];
    assert_eq!(format!("{:?}", error.code()), "E2059");
    let message = error.format_with(&result.pool, &result.interner);
    assert!(message.contains("fixed-list capacity must be an evaluable integer expression"));
}

#[test]
fn fixed_list_capacity_const_params_shadow_module_const_values() {
    let result = check_source(fixture_without_trailing_newline(include_str!(
        "../fixtures/integration/fixed_list_capacity_const_param_shadow.ori"
    )));
    assert!(
        !result.has_errors(),
        "function and method const params must shadow same-named module values: {:?}",
        result.error_kinds()
    );
}

#[test]
fn fixed_list_capacity_rejects_lexical_bindings_that_shadow_module_consts() {
    let result = check_source(fixture_without_trailing_newline(include_str!(
        "../fixtures/integration/fixed_list_capacity_module_shadowing.ori"
    )));
    assert_eq!(
        result.error_count(),
        2,
        "runtime and mutable lexical shadows must each produce one focused error: {:?}",
        result.error_kinds()
    );
    assert!(result
        .result
        .typed
        .errors
        .iter()
        .all(|error| format!("{:?}", error.code()) == "E2056"));
}

#[test]
fn fixed_list_capacity_rejects_non_positive_declared_type_positions() {
    let result = check_source(fixture_without_trailing_newline(include_str!(
        "../fixtures/integration/fixed_list_capacity_declared_types.ori"
    )));
    assert_eq!(
        result.error_count(),
        5,
        "newtype, function parameter/return, and method parameter/return capacities must all be validated: {:?}",
        result.error_kinds()
    );
    assert!(result
        .result
        .typed
        .errors
        .iter()
        .all(|error| format!("{:?}", error.code()) == "E2057"));
}

#[test]
fn method_const_capacity_rejects_non_positive_call_args_without_result_annotations() {
    let source = fixture_without_trailing_newline(include_str!(
        "../fixtures/integration/method_const_capacity_call_site_non_positive.ori"
    ));
    let result = check_source(source);
    assert_eq!(
        result.error_count(),
        3,
        "explicit zero, explicit negative, and default zero capacity args must each be rejected: {:?}",
        result.error_kinds()
    );
    let messages: Vec<_> = result
        .result
        .typed
        .errors
        .iter()
        .map(|error| {
            assert_eq!(
                format!("{:?}", error.code()),
                "E2057",
                "zero on an unrelated int const generic must remain valid"
            );
            error.format_with(&result.pool, &result.interner)
        })
        .collect();
    assert_eq!(
        messages
            .iter()
            .filter(|message| message.contains("supplied 0"))
            .count(),
        2,
        "explicit and default zero must both identify the concrete value: {messages:?}"
    );
    assert!(
        messages
            .iter()
            .any(|message| message.contains("supplied -1")),
        "negative explicit arg must identify the concrete value: {messages:?}"
    );
    let diagnostic_sources: Vec<_> = result
        .result
        .typed
        .errors
        .iter()
        .map(|error| &source[error.span().to_range()])
        .collect();
    assert_eq!(
        diagnostic_sources,
        [
            "values.explicit<0>()",
            "values.explicit<-1>()",
            "values.defaulted()",
        ],
        "explicit/default failures must remain call-site diagnostics"
    );
}

#[test]
fn method_const_capacity_inferred_from_invalid_annotation_reports_once_at_annotation() {
    let source = fixture_without_trailing_newline(include_str!(
        "../fixtures/integration/method_const_capacity_expected_annotation_non_positive.ori"
    ));
    let result = check_source(source);
    assert_eq!(
        result.error_count(),
        1,
        "the validated result annotation must not be duplicated at the method call: {:?}",
        result.error_kinds()
    );
    let error = &result.result.typed.errors[0];
    assert_eq!(format!("{:?}", error.code()), "E2057");
    assert_eq!(
        &source[error.span().to_range()],
        "0",
        "annotation-inferred capacity errors belong to the exact capacity expression"
    );
}

#[test]
fn method_const_body_capacity_rejects_non_positive_call_arg() {
    let result = check_source(fixture_without_trailing_newline(include_str!(
        "../fixtures/integration/method_const_body_capacity_call_site_non_positive.ori"
    )));
    assert_eq!(
        result.error_count(),
        1,
        "body-local fixed-list capacity must retain its call-site constraint: {:?}",
        result.error_kinds()
    );
    assert_eq!(
        format!("{:?}", result.result.typed.errors[0].code()),
        "E2057"
    );
}

#[test]
fn method_const_call_site_explicit_and_default_values_publish_exact_bindings() {
    let result = check_source(fixture_without_trailing_newline(include_str!(
        "../fixtures/integration/method_const_call_site_positive_bindings.ori"
    )));
    assert!(
        !result.has_errors(),
        "positive capacity args and unrelated zero must type-check: {:?}",
        result.error_kinds()
    );

    for (method, expected) in [("explicit", 3), ("defaulted", 2), ("unrelated", 0)] {
        let instances = result.mono_instances_for(method);
        assert_eq!(
            instances.len(),
            1,
            "{method} must publish exactly one annotation-free call instance: {instances:?}"
        );
        assert_eq!(
            instances[0].method_args,
            vec![crate::GenericArg::Const(crate::ConstValue::Int(expected))]
        );
        assert_eq!(
            instances[0].const_bindings,
            vec![crate::MonoConstBinding {
                name: result.interner.intern("N"),
                value: crate::ConstValue::Int(expected),
            }]
        );
    }
}

#[test]
fn method_const_call_site_rejects_extra_and_wrong_typed_arguments() {
    let result = check_source(fixture_without_trailing_newline(include_str!(
        "../fixtures/integration/method_const_call_site_argument_diagnostics.ori"
    )));
    assert_eq!(
        result.error_count(),
        2,
        "extra and wrong-typed method const args need focused diagnostics: {:?}",
        result.error_kinds()
    );
    let codes: Vec<_> = result
        .result
        .typed
        .errors
        .iter()
        .map(|error| format!("{:?}", error.code()))
        .collect();
    assert!(codes.iter().any(|code| code == "E2004"), "{codes:?}");
    assert!(codes.iter().any(|code| code == "E2001"), "{codes:?}");
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
