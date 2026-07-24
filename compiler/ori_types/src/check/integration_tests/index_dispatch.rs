use super::support::*;

#[test]
fn index_sites_freeze_the_key_specific_impl_producer() {
    let result = check_source(include_str!(
        "../fixtures/integration/index_sites_freeze_the_key_specific_impl_producer.ori"
    ));
    assert!(
        !result.has_errors(),
        "multiple key-specific Index impls must type-check: {:?}",
        result.error_kinds()
    );

    let typed = &result.result.typed;
    assert_eq!(
        typed.index_dispatch_map.len(),
        2,
        "each custom index expression must retain its selected producer"
    );
    let selected: Vec<_> = typed
        .index_dispatch_map
        .iter()
        .map(|(_, dispatch)| {
            let ori_ir::canon::IndexDispatch::Selected(id) = dispatch else {
                panic!("custom index site must carry a selected producer: {dispatch:?}");
            };
            &typed.method_producers[id.index()]
        })
        .collect();
    let expected: Vec<_> = result
        .parsed
        .module
        .impls
        .iter()
        .enumerate()
        .map(|(impl_index, implementation)| {
            crate::MethodProducer::Impl(crate::ImplMethodId::new(
                impl_index,
                implementation.methods[0].body,
            ))
        })
        .collect();

    assert!(selected.contains(&&expected[0]), "int-key producer missing");
    assert!(selected.contains(&&expected[1]), "str-key producer missing");
    assert_ne!(selected[0], selected[1]);
}

#[test]
fn builtin_index_sites_reject_incompatible_key_types() {
    let cases = [
        "@bad_list () -> int = [1, 2][\"first\"];",
        "@bad_map () -> Option<int> = { \"first\": 1 }[0];",
        "@bad_str () -> str = \"value\"[\"first\"];",
    ];

    for source in cases {
        let result = check_source(source);
        assert!(
            result.has_errors(),
            "builtin index key mismatch must be diagnosed for `{source}`"
        );
    }
}
