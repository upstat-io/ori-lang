use ori_vm::{CompileOptions, ExitValue, VerifiedProgram};

/// One source program admitted to the mixed VM benchmark and correctness corpus.
pub struct VmBenchmarkCase {
    /// Stable Criterion parameter name.
    pub name: &'static str,
    /// Canonical path relative to the compiler repository root.
    pub path: &'static str,
    /// Complete Ori source text.
    pub source: &'static str,
    /// Exact observable result required from every execution mode.
    pub expected_value: ExitValue,
    /// Exact bytes captured from interpreted output.
    pub expected_output: &'static [u8],
}

/// Workloads currently admitted by the source-to-VM prototype path.
pub const VM_BENCHMARK_CASES: &[VmBenchmarkCase] = &[
    VmBenchmarkCase {
        name: "return_nonzero",
        path: "compiler/oric/tests/fixtures/vm_benchmark/vm_main_nonzero_returns_exact.ori",
        source: include_str!("../fixtures/vm_benchmark/vm_main_nonzero_returns_exact.ori"),
        expected_value: ExitValue::Int(37),
        expected_output: b"",
    },
    VmBenchmarkCase {
        name: "print_output",
        path: "compiler/oric/tests/fixtures/vm_benchmark/vm_print_output_captured_exact.ori",
        source: include_str!("../fixtures/vm_benchmark/vm_print_output_captured_exact.ori"),
        expected_value: ExitValue::Int(41),
        expected_output: b"vm output pin\n",
    },
    VmBenchmarkCase {
        name: "aggregate_escape_loop",
        path: "compiler/oric/tests/fixtures/vm_benchmark/vm_aggregate_escape_loop_exact.ori",
        source: include_str!("../fixtures/vm_benchmark/vm_aggregate_escape_loop_exact.ori"),
        expected_value: ExitValue::Int(37),
        expected_output: b"",
    },
    VmBenchmarkCase {
        name: "nested_aggregate_escape",
        path: "compiler/oric/tests/fixtures/vm_benchmark/vm_nested_aggregate_escape_exact.ori",
        source: include_str!("../fixtures/vm_benchmark/vm_nested_aggregate_escape_exact.ori"),
        expected_value: ExitValue::Int(73),
        expected_output: b"",
    },
    VmBenchmarkCase {
        name: "nested_list_inner_alias_release",
        path:
            "compiler/oric/tests/fixtures/vm_benchmark/vm_nested_list_inner_alias_release_exact.ori",
        source: include_str!(
            "../fixtures/vm_benchmark/vm_nested_list_inner_alias_release_exact.ori"
        ),
        expected_value: ExitValue::Int(61),
        expected_output: b"",
    },
    VmBenchmarkCase {
        name: "nested_string_cow_forks",
        path: "compiler/oric/tests/fixtures/vm_benchmark/vm_nested_string_cow_forks_exact.ori",
        source: include_str!("../fixtures/vm_benchmark/vm_nested_string_cow_forks_exact.ori"),
        expected_value: ExitValue::Int(67),
        expected_output: b"",
    },
    VmBenchmarkCase {
        name: "string_split_release",
        path: "compiler/oric/tests/fixtures/vm_benchmark/vm_string_split_release_exact.ori",
        source: include_str!("../fixtures/vm_benchmark/vm_string_split_release_exact.ori"),
        expected_value: ExitValue::Int(71),
        expected_output: b"",
    },
    VmBenchmarkCase {
        name: "utf8_length_bytes",
        path: "compiler/oric/tests/fixtures/vm_benchmark/vm_utf8_length_bytes_exact.ori",
        source: include_str!("../fixtures/vm_benchmark/vm_utf8_length_bytes_exact.ori"),
        expected_value: ExitValue::Int(37),
        expected_output: b"",
    },
    VmBenchmarkCase {
        name: "doors_list_iterator",
        path: "compiler/oric/tests/fixtures/vm_benchmark/vm_doors_list_iterator_exact.ori",
        source: include_str!("../fixtures/vm_benchmark/vm_doors_list_iterator_exact.ori"),
        expected_value: ExitValue::Int(100),
        expected_output: b"",
    },
    VmBenchmarkCase {
        name: "arithmetic_calls",
        path: "tests/benchmarks/bench_small.ori",
        source: include_str!("../../../../tests/benchmarks/bench_small.ori"),
        expected_value: ExitValue::Int(0),
        expected_output: b"",
    },
    VmBenchmarkCase {
        name: "strings",
        path: "tests/benchmarks/bench_string.ori",
        source: include_str!("../../../../tests/benchmarks/bench_string.ori"),
        expected_value: ExitValue::Int(0),
        expected_output: b"",
    },
    VmBenchmarkCase {
        name: "cow_compare",
        path: "tests/benchmarks/cow/compare.ori",
        source: include_str!("../../../../tests/benchmarks/cow/compare.ori"),
        expected_value: ExitValue::Int(0),
        expected_output: b"",
    },
    VmBenchmarkCase {
        name: "list_push_shared",
        path: "tests/benchmarks/cow/list_push_shared.ori",
        source: include_str!("../../../../tests/benchmarks/cow/list_push_shared.ori"),
        expected_value: ExitValue::Int(0),
        expected_output: b"",
    },
    VmBenchmarkCase {
        name: "nested_lists",
        path: "tests/benchmarks/cow/nested_list_crash.ori",
        source: include_str!("../../../../tests/benchmarks/cow/nested_list_crash.ori"),
        expected_value: ExitValue::Int(0),
        expected_output: b"",
    },
    VmBenchmarkCase {
        name: "file_pipeline",
        path: "tests/benchmarks/cow/macro/file_pipeline.ori",
        source: include_str!("../../../../tests/benchmarks/cow/macro/file_pipeline.ori"),
        expected_value: ExitValue::Int(0),
        expected_output: b"",
    },
    VmBenchmarkCase {
        name: "json_builder",
        path: "tests/benchmarks/cow/macro/json_builder.ori",
        source: include_str!("../../../../tests/benchmarks/cow/macro/json_builder.ori"),
        expected_value: ExitValue::Int(0),
        expected_output: b"",
    },
    VmBenchmarkCase {
        name: "sharing_and_functions",
        path: "tests/valgrind/sharing_and_functions.ori",
        source: include_str!("../../../../tests/valgrind/sharing_and_functions.ori"),
        expected_value: ExitValue::Int(0),
        expected_output: b"",
    },
];

/// Stable real-fixture prefix used for physical-planning measurements.
pub const VM_PHYSICAL_PLANNING_CASE_COUNT: usize = 8;

/// Compile and verify one case with an explicit primitive-lowering mode.
pub fn compile_case(case: &VmBenchmarkCase, typed_primitives: bool) -> VerifiedProgram {
    let executable = oric::test_support::compile_to_executable(
        case.path,
        case.source,
        ori_repr::NarrowingPolicy::Aggressive,
    )
    .unwrap_or_else(|error| panic!("{} should realize: {error}", case.path));
    let bytecode = ori_vm::compile_with_options(&executable, CompileOptions { typed_primitives })
        .unwrap_or_else(|error| panic!("{} should lower to bytecode: {error}", case.path));
    ori_vm::verify(bytecode)
        .unwrap_or_else(|error| panic!("{} bytecode should verify: {error}", case.path))
}
