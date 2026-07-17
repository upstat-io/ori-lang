use super::compile_to_executable;

#[test]
fn executable_parse_error_names_cause_and_fix() {
    let result = compile_to_executable(
        "broken.ori",
        "@main () -> void = {",
        ori_repr::NarrowingPolicy::Aggressive,
    );
    let Err(error) = result else {
        panic!("the incomplete function body should fail to parse");
    };
    let message = error.to_string();

    assert!(message.contains("could not parse `broken.ori`"));
    assert!(message.contains("fix the syntax at the reported location"));
    assert!(message.contains("then rerun the same command"));
}

#[test]
fn executable_type_error_names_cause_location_and_fix() {
    let result = compile_to_executable(
        "broken_types.ori",
        "@main () -> int = true;",
        ori_repr::NarrowingPolicy::Aggressive,
    );
    let Err(error) = result else {
        panic!("the mismatched return type should fail type checking");
    };
    let message = error.to_string();

    assert!(
        message.contains("could not type-check `broken_types.ori`"),
        "{message}"
    );
    assert!(
        message.contains("type mismatch: expected `int`, found `bool`"),
        "{message}"
    );
    assert!(
        message.contains("at its reported source location"),
        "{message}"
    );
    assert!(message.contains("then rerun the same command"), "{message}");
}

#[test]
fn aggregate_escape_loop_closes_parallel_realization_metadata() {
    let source =
        include_str!("../../tests/fixtures/vm_benchmark/vm_aggregate_escape_loop_exact.ori");
    let executable = compile_to_executable(
        "compiler/oric/tests/fixtures/vm_benchmark/vm_aggregate_escape_loop_exact.ori",
        source,
        ori_repr::NarrowingPolicy::Aggressive,
    )
    .unwrap_or_else(|error| panic!("aggregate escape loop should realize: {error}"));

    for function in executable.functions() {
        assert_eq!(function.var_reprs.len(), function.var_types.len());
        assert_eq!(function.var_rc_strategies.len(), function.var_types.len());
    }
}

#[test]
fn sequential_executable_compiles_freeze_each_programs_primitive_facts() {
    let cases = [
        (
            "compiler/oric/tests/fixtures/vm_benchmark/vm_main_nonzero_returns_exact.ori",
            include_str!("../../tests/fixtures/vm_benchmark/vm_main_nonzero_returns_exact.ori"),
        ),
        (
            "compiler/oric/tests/fixtures/vm_benchmark/vm_print_output_captured_exact.ori",
            include_str!("../../tests/fixtures/vm_benchmark/vm_print_output_captured_exact.ori"),
        ),
        (
            "compiler/oric/tests/fixtures/vm_benchmark/vm_aggregate_escape_loop_exact.ori",
            include_str!("../../tests/fixtures/vm_benchmark/vm_aggregate_escape_loop_exact.ori"),
        ),
    ];

    for (path, source) in cases {
        compile_to_executable(path, source, ori_repr::NarrowingPolicy::Aggressive)
            .unwrap_or_else(|error| panic!("{path} should realize in sequence: {error}"));
    }
}

#[test]
fn generic_derived_eq_operator_realizes_its_concrete_method_body() {
    let source = r"
#[derive(Eq)]
type GenericDerived<T: Eq> = { value: T }

@main () -> bool = {
    let left: GenericDerived<int> = GenericDerived { value: 42 };
    let right: GenericDerived<int> = GenericDerived { value: 42 };
    left == right
}
";

    compile_to_executable(
        "generic_derived_eq.ori",
        source,
        ori_repr::NarrowingPolicy::Aggressive,
    )
    .unwrap_or_else(|error| panic!("generic derived Eq operator should realize: {error}"));
}
