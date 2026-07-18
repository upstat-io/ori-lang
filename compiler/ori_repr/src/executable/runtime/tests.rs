use ori_registry::{MethodKind, MethodRuntime, OptionRuntime, ResultRuntime, TypeTag};

use super::RuntimeCall;

#[test]
fn unwrap_or_resolution_preserves_receiver_semantics() {
    assert_eq!(
        RuntimeCall::resolve("unwrap_or", Some(TypeTag::Option)),
        Some(RuntimeCall::RegisteredMethod(MethodRuntime::Option(
            OptionRuntime::UnwrapOr
        )))
    );
    assert_eq!(
        RuntimeCall::resolve("unwrap_or", Some(TypeTag::Result)),
        Some(RuntimeCall::RegisteredMethod(MethodRuntime::Result(
            ResultRuntime::UnwrapOr
        )))
    );
    assert_eq!(
        RuntimeCall::RegisteredMethod(MethodRuntime::Option(OptionRuntime::UnwrapOr)).arity(),
        2
    );
    assert_eq!(
        RuntimeCall::RegisteredMethod(MethodRuntime::Result(ResultRuntime::UnwrapOr)).arity(),
        2
    );
}

#[test]
fn registered_wrapper_resolution_is_receiver_exact() {
    let option_cases = [
        ("unwrap", OptionRuntime::Unwrap),
        ("map", OptionRuntime::Map),
        ("and_then", OptionRuntime::AndThen),
        ("clone", OptionRuntime::Clone),
        ("debug", OptionRuntime::Debug),
        ("equals", OptionRuntime::Equals),
        ("hash", OptionRuntime::Hash),
    ];
    for (name, operation) in option_cases {
        assert_eq!(
            RuntimeCall::resolve(name, Some(TypeTag::Option)),
            Some(RuntimeCall::RegisteredMethod(MethodRuntime::Option(
                operation
            ))),
            "Option.{name}"
        );
    }

    let result_cases = [
        ("unwrap", ResultRuntime::Unwrap),
        ("map", ResultRuntime::Map),
        ("and_then", ResultRuntime::AndThen),
        ("clone", ResultRuntime::Clone),
        ("debug", ResultRuntime::Debug),
        ("equals", ResultRuntime::Equals),
        ("hash", ResultRuntime::Hash),
    ];
    for (name, operation) in result_cases {
        assert_eq!(
            RuntimeCall::resolve(name, Some(TypeTag::Result)),
            Some(RuntimeCall::RegisteredMethod(MethodRuntime::Result(
                operation
            ))),
            "Result.{name}"
        );
    }

    assert_eq!(RuntimeCall::resolve("unwrap", None), None);
    assert_eq!(RuntimeCall::resolve("unwrap", Some(TypeTag::Int)), None);
}

#[test]
fn backend_required_registry_method_resolves_without_textual_fallback() {
    let call = RuntimeCall::resolve("is_less", Some(TypeTag::Ordering))
        .unwrap_or_else(|| panic!("Ordering.is_less should resolve"));
    let RuntimeCall::RegistryMethod(method) = call else {
        panic!("Ordering.is_less should preserve its registry identity");
    };
    assert_eq!(method.receiver(), TypeTag::Ordering);
    assert_eq!(method.arity(), 1);
}

#[test]
fn every_registered_method_has_an_exact_callable_identity() {
    for receiver in [
        TypeTag::Unit,
        TypeTag::List,
        TypeTag::Map,
        TypeTag::Set,
        TypeTag::Tuple,
        TypeTag::Ordering,
    ] {
        for method in ori_registry::methods_for(receiver) {
            let call = RuntimeCall::resolve(method.name, Some(receiver))
                .unwrap_or_else(|| panic!("{receiver:?}.{} should resolve", method.name));
            let receiver_arity = usize::from(method.kind == MethodKind::Instance);
            assert_eq!(call.arity(), method.params.len() + receiver_arity);
        }
    }
}

#[test]
fn registered_prelude_resolution_is_exact_and_arity_checked() {
    let call = RuntimeCall::resolve("hash_combine", None)
        .unwrap_or_else(|| panic!("hash_combine should resolve"));
    let RuntimeCall::RegistryPrelude(function) = call else {
        panic!("hash_combine should preserve its prelude registry identity");
    };
    assert_eq!(function.arity(), 2);

    assert_eq!(RuntimeCall::resolve("missing_prelude", None), None);
}

#[test]
fn compiler_protocols_close_with_typed_identities() {
    let cases = [
        ("__cast", 1),
        ("__collect_set", 1),
        ("ori_catch_recover", 0),
        ("__ori_inject_trace", 1),
        ("ori_list_slice_drop", 2),
        ("ori_format_int", 2),
    ];
    for (symbol, arity) in cases {
        let call = RuntimeCall::resolve(symbol, None)
            .unwrap_or_else(|| panic!("{symbol} should have a closed identity"));
        assert_eq!(call.arity(), arity, "{symbol}");
    }
}

#[test]
fn primitive_print_symbols_share_one_closed_semantic_identity() {
    let symbols = [
        "ori_print",
        "ori_print_int",
        "ori_print_float",
        "ori_print_bool",
    ];
    let mut visited = 0;
    for symbol in symbols {
        assert_eq!(
            RuntimeCall::resolve(symbol, None),
            Some(RuntimeCall::Print),
            "{symbol}"
        );
        visited += 1;
    }
    assert_eq!(visited, symbols.len());

    assert_eq!(RuntimeCall::resolve("ori_print_char", None), None);
}
