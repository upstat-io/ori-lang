use super::support::*;

#[test]
fn concrete_and_implicit_generic_extensions_publish_exact_producers() {
    let result = check_source(fixture_without_trailing_newline(include_str!(
        "../fixtures/integration/extensions_concrete_and_generic.ori"
    )));
    assert!(
        !result.has_errors(),
        "concrete and generic extensions must type-check: {:?}",
        result.error_kinds()
    );
    assert_eq!(result.function_body_type("call_shout"), Some(Idx::STR));
    assert_eq!(result.function_body_type("call_tally"), Some(Idx::INT));

    let generic_method = &result.parsed.module.extends[1].methods[0];
    let expected_id =
        crate::ImplMethodId::new(result.parsed.module.impls.len() + 1, generic_method.body);
    let instances = result.mono_instances_for("item_tally");
    assert_eq!(
        instances.len(),
        1,
        "generic extension call must publish one concrete method demand: {instances:?}"
    );
    assert_eq!(
        instances[0].method_producer,
        Some(crate::MethodProducer::Impl(expected_id))
    );
    assert_eq!(
        instances[0].impl_args,
        vec![crate::GenericArg::Type(Idx::INT)]
    );
    assert_eq!(instances[0].concrete_return_type, Idx::INT);
    assert!(result
        .result
        .typed
        .impl_sigs
        .iter()
        .any(|sig| sig.id == expected_id));
}

#[test]
fn inherent_and_trait_methods_precede_extensions() {
    let result = check_source(fixture_without_trailing_newline(include_str!(
        "../fixtures/integration/extensions_precedence.ori"
    )));
    assert!(
        !result.has_errors(),
        "inherent and trait methods must win over same-named extensions: {:?}",
        result.error_kinds()
    );
    assert_eq!(result.function_body_type("inherent_rank"), Some(Idx::INT));
    assert_eq!(result.function_body_type("trait_rank"), Some(Idx::INT));
}

#[test]
fn conflicting_extensions_fail_closed_as_ambiguous() {
    let result = check_source(fixture_without_trailing_newline(include_str!(
        "../fixtures/integration/extensions_ambiguous.ori"
    )));
    let clash = result.interner.intern("clash");
    assert!(result.error_kinds().iter().any(|kind| matches!(
        kind,
        TypeErrorKind::AmbiguousMethod { method, .. } if *method == clash
    )));
}

#[test]
fn no_self_extension_method_is_not_an_instance_candidate() {
    let result = check_source(fixture_without_trailing_newline(include_str!(
        "../fixtures/integration/extensions_static_not_instance.ori"
    )));
    let factory = result.interner.intern("factory");
    assert!(result.error_kinds().iter().any(|kind| matches!(
        kind,
        TypeErrorKind::UnknownMethod { method, .. } if *method == factory
    )));
    assert!(result.result.typed.impl_sigs.is_empty());
}
