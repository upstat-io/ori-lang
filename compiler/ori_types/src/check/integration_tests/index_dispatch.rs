use super::support::*;

#[test]
fn index_sites_freeze_the_key_specific_impl_producer() {
    let result = check_source(
        r#"
type JsonValue = { data: str }

trait Index<Key, Value> {
    @index (self, key: Key) -> Value
}

impl JsonValue: Index<int, str> {
    @index (self, key: int) -> str = self.data;
}

impl JsonValue: Index<str, str> {
    @index (self, key: str) -> str = self.data;
}

@both_keys () -> str = {
    let value = JsonValue { data: "value" };
    value[0] + value["key"]
}
"#,
    );
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
        .map(|(_, id)| &typed.method_producers[id.index()])
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
