use ori_vm::{ExecutionConfig, ExitValue};

#[test]
fn source_unit_main_returns_unit_without_aggregate_materialization() {
    let source = "@main () -> void = ();";
    let executable = oric::test_support::compile_to_executable(
        "vm_unit_runtime.ori",
        source,
        ori_repr::NarrowingPolicy::Aggressive,
    )
    .unwrap_or_else(|error| panic!("unit fixture should realize: {error}"));

    for typed_primitives in [false, true] {
        let bytecode =
            ori_vm::compile_with_options(&executable, ori_vm::CompileOptions { typed_primitives })
                .unwrap_or_else(|error| panic!("unit fixture should lower to bytecode: {error}"));
        let verified = ori_vm::verify(bytecode)
            .unwrap_or_else(|error| panic!("unit fixture bytecode should verify: {error}"));
        let report = ori_vm::execute_report(&verified, ExecutionConfig::default());
        let value = report.result.unwrap_or_else(|error| {
            panic!("unit fixture should execute with typed_primitives={typed_primitives}: {error}")
        });

        assert_eq!(value, ExitValue::Unit);
        assert_eq!(report.metrics.cumulative_value_arena_allocations, 0);
        assert_eq!(report.metrics.exit_value_arena_entries, 0);
    }
}
