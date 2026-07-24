use ori_vm::{ExecutionConfig, ExitValue};

fn execute(source: &str, typed_primitives: bool) -> ExitValue {
    let executable = oric::test_support::compile_to_executable(
        "vm_collection_runtime.ori",
        source,
        ori_repr::NarrowingPolicy::Aggressive,
    )
    .unwrap_or_else(|error| panic!("collection fixture should realize: {error}"));
    let bytecode =
        ori_vm::compile_with_options(&executable, ori_vm::CompileOptions { typed_primitives })
            .unwrap_or_else(|error| panic!("collection fixture should lower to bytecode: {error}"));
    let verified = ori_vm::verify(bytecode)
        .unwrap_or_else(|error| panic!("collection fixture bytecode should verify: {error}"));
    ori_vm::execute(&verified, ExecutionConfig::default())
        .unwrap_or_else(|error| panic!("collection fixture should execute: {error}"))
        .value
}

#[test]
fn persistent_list_push_preserves_shared_snapshot() {
    let source = r"
@main () -> int = {
    let values: [int] = [];
    values = values.push(11);
    let snapshot = values;
    values = values.push(29);
    let unique: [int] = [];
    unique = unique.push(3);
    unique = unique.push(5);
    let ok = snapshot.len() == 1
        && snapshot[0] == 11
        && values.len() == 2
        && values[0] == 11
        && values[1] == 29
        && unique.len() == 2
        && unique[0] == 3
        && unique[1] == 5;

    if ok then 83 else 0
}
";

    for typed_primitives in [false, true] {
        assert_eq!(
            execute(source, typed_primitives),
            ExitValue::Int(83),
            "list push semantics diverged with typed_primitives={typed_primitives}",
        );
    }
}

#[test]
fn indexed_heap_element_has_an_owned_reference() {
    let source = r#"
@main () -> int = {
    let parts = "  alpha  , beta ".split(",");
    let first = parts[0].trim();
    let second = parts[1].trim();
    let source_survived = parts[0].starts_with("  alpha")
        && parts[1].starts_with(" beta");

    if first == "alpha" && second == "beta" && source_survived then 89 else 0
}
"#;

    for typed_primitives in [false, true] {
        assert_eq!(
            execute(source, typed_primitives),
            ExitValue::Int(89),
            "indexed heap ownership diverged with typed_primitives={typed_primitives}",
        );
    }
}
