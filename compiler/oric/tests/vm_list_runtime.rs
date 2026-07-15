use ori_vm::{ExecutionConfig, ExitValue};
use std::path::Path;
use std::process::Command;

const LIST_PROGRAM: &str = include_str!("fixtures/vm_list_runtime.ori");
const CURRIED_LIST_PROGRAM: &str = include_str!("fixtures/vm_curried_list_runtime.ori");

#[test]
fn evaluator_matches_combined_list_mutation_fixture() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("vm_list_runtime.ori");
    let output = Command::new(env!("CARGO_BIN_EXE_ori"))
        .arg("run")
        .arg(fixture)
        .output()
        .unwrap_or_else(|error| panic!("evaluator fixture should launch: {error}"));

    assert_eq!(output.status.code(), Some(121), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
}

#[test]
fn source_list_mutations_preserve_shared_input_and_release_all_values() {
    let executable = oric::test_support::compile_to_executable(
        "vm_list_runtime.ori",
        LIST_PROGRAM,
        ori_repr::NarrowingPolicy::Aggressive,
    )
    .unwrap_or_else(|error| panic!("list fixture should realize: {error}"));

    for typed_primitives in [false, true] {
        let bytecode =
            ori_vm::compile_with_options(&executable, ori_vm::CompileOptions { typed_primitives })
                .unwrap_or_else(|error| panic!("list fixture should lower: {error}"));
        let verified = ori_vm::verify(bytecode)
            .unwrap_or_else(|error| panic!("list fixture bytecode should verify: {error}"));
        let report = ori_vm::execute_report(&verified, ExecutionConfig::default());
        let value = report.result.unwrap_or_else(|error| {
            panic!("list fixture should execute with typed_primitives={typed_primitives}: {error}")
        });

        assert_eq!(value, ExitValue::Int(121));
        assert_eq!(report.metrics.exit_live_heap_objects, 0);
        assert_eq!(report.metrics.final_live_heap_objects, 0);
        assert_eq!(report.metrics.final_value_arena_entries, 0);
    }
}

#[test]
fn evaluator_executes_curried_list_concat() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("vm_curried_list_runtime.ori");
    let output = Command::new(env!("CARGO_BIN_EXE_ori"))
        .arg("run")
        .arg(fixture)
        .output()
        .unwrap_or_else(|error| panic!("curried-list evaluator fixture should launch: {error}"));

    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
}

#[test]
fn vm_executes_curried_list_concat_without_leaks() {
    let executable = oric::test_support::compile_to_executable(
        "vm_curried_list_runtime.ori",
        CURRIED_LIST_PROGRAM,
        ori_repr::NarrowingPolicy::Aggressive,
    )
    .unwrap_or_else(|error| panic!("curried-list fixture should realize: {error}"));

    for typed_primitives in [false, true] {
        let bytecode =
            ori_vm::compile_with_options(&executable, ori_vm::CompileOptions { typed_primitives })
                .unwrap_or_else(|error| panic!("curried-list fixture should lower: {error}"));
        let verified = ori_vm::verify(bytecode)
            .unwrap_or_else(|error| panic!("curried-list bytecode should verify: {error}"));
        let report = ori_vm::execute_report(&verified, ExecutionConfig::default());
        let value = report.result.unwrap_or_else(|error| {
            panic!("curried-list fixture should execute in both modes: {error}")
        });

        assert_eq!(value, ExitValue::Int(0));
        assert_eq!(report.metrics.exit_live_heap_objects, 0);
        assert_eq!(report.metrics.final_live_heap_objects, 0);
        assert_eq!(report.metrics.final_value_arena_entries, 0);
    }
}
