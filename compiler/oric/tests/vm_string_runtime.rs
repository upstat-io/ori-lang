use ori_vm::{ExecutionConfig, ExitValue};

fn execute(source: &str, typed_primitives: bool) -> ExitValue {
    let executable = oric::test_support::compile_to_executable(
        "vm_string_runtime.ori",
        source,
        ori_repr::NarrowingPolicy::Aggressive,
    )
    .unwrap_or_else(|error| panic!("string fixture should realize: {error}"));
    let bytecode =
        ori_vm::compile_with_options(&executable, ori_vm::CompileOptions { typed_primitives })
            .unwrap_or_else(|error| panic!("string fixture should lower to bytecode: {error}"));
    let verified = ori_vm::verify(bytecode)
        .unwrap_or_else(|error| panic!("string fixture bytecode should verify: {error}"));
    ori_vm::execute(&verified, ExecutionConfig::default())
        .unwrap_or_else(|error| panic!("string fixture should execute: {error}"))
        .value
}

fn assert_both_execution_modes(source: &str, expected: &ExitValue) {
    for typed_primitives in [false, true] {
        let actual = execute(source, typed_primitives);
        assert_eq!(
            &actual, expected,
            "string semantics diverged with typed_primitives={typed_primitives}",
        );
    }
}

#[test]
fn executes_closed_string_runtime_surface() {
    let source = r#"
@main () -> int = {
    let text = " Alpha,beta,GAMMA ";
    let trimmed = text.trim();
    let parts = trimmed.split(",");
    let upper = "MiXeD".to_uppercase();
    let lower = "MiXeD".to_lowercase();
    let predicates = text.contains("beta")
        && !text.contains("delta")
        && text.starts_with(" ")
        && !text.starts_with("Alpha")
        && text.ends_with(" ")
        && !text.ends_with("GAMMA")
        && "".is_empty()
        && !text.is_empty();
    let transforms = upper.starts_with("MIX")
        && upper.ends_with("ED")
        && lower.starts_with("mix")
        && lower.ends_with("ed");
    let split_ok = parts.len() == 3
        && parts[0].starts_with("Alpha")
        && parts[1].starts_with("beta")
        && parts[2].starts_with("GAMMA");

    if predicates && transforms && split_ok then 73 else 0
}
"#;

    assert_both_execution_modes(source, &ExitValue::Int(73));
}

#[test]
fn executes_string_operators_and_conversion() {
    let source = r#"
@main () -> int = {
    let joined = "alpha" + "beta";
    let converted = str(42);
    let equality = joined == "alphabeta"
        && joined != "alpha"
        && converted == "42";
    let ordering = "alpha" < "beta"
        && "alpha" <= "alpha"
        && "beta" > "alpha"
        && "beta" >= "beta";

    if equality && ordering then 79 else 0
}
"#;

    assert_both_execution_modes(source, &ExitValue::Int(79));
}
