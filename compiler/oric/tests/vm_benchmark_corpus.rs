#[path = "support/vm_benchmark_cases.rs"]
mod vm_benchmark_cases;

use ori_vm::{ExecutionConfig, ExecutionMetrics, ExitValue, VerifiedProgram};

use vm_benchmark_cases::{
    compile_case, VmBenchmarkCase, VM_BENCHMARK_CASES, VM_PHYSICAL_PLANNING_CASE_COUNT,
};

#[derive(Debug, PartialEq)]
struct RunObservation {
    value: ExitValue,
    output: Vec<u8>,
    metrics: ExecutionMetrics,
}

fn execute_exact(
    case: &VmBenchmarkCase,
    program: &VerifiedProgram,
    mode: &str,
    run: usize,
) -> RunObservation {
    let report = ori_vm::execute_report(program, ExecutionConfig::default());
    let value = report.result.unwrap_or_else(|error| {
        panic!(
            "{} ({}) {mode} run {run} should execute: {error}",
            case.path, case.name,
        )
    });

    assert_eq!(
        value, case.expected_value,
        "{} {mode} run {run} returned the wrong value",
        case.path,
    );
    assert_eq!(
        report.output.as_slice(),
        case.expected_output,
        "{} {mode} run {run} produced the wrong output",
        case.path,
    );
    assert_eq!(
        report.metrics.exit_live_heap_objects, 0,
        "{} {mode} run {run} leaked logical VM heap objects before session cleanup",
        case.path,
    );
    assert_eq!(
        report.metrics.exit_live_heap_payload_bytes, 0,
        "{} {mode} run {run} leaked logical VM heap payload before session cleanup",
        case.path,
    );
    assert_eq!(
        report.metrics.final_live_heap_objects, 0,
        "{} {mode} run {run} retained VM heap objects after cleanup",
        case.path,
    );
    assert_eq!(
        report.metrics.final_live_heap_payload_bytes, 0,
        "{} {mode} run {run} retained VM heap payload bytes after cleanup",
        case.path,
    );

    RunObservation {
        value,
        output: report.output,
        metrics: report.metrics,
    }
}

#[test]
fn mixed_corpus_typed_and_untyped_repeats_match_exact_outcomes() {
    let mut visited = 0;
    for case in VM_BENCHMARK_CASES {
        let typed = compile_case(case, true);
        let untyped = compile_case(case, false);

        let typed_first = execute_exact(case, &typed, "typed", 1);
        let typed_second = execute_exact(case, &typed, "typed", 2);
        assert_eq!(
            typed_first, typed_second,
            "{} typed repeated runs leaked session state",
            case.path,
        );

        let untyped_first = execute_exact(case, &untyped, "untyped", 1);
        let untyped_second = execute_exact(case, &untyped, "untyped", 2);
        assert_eq!(
            untyped_first, untyped_second,
            "{} untyped repeated runs leaked session state",
            case.path,
        );
        assert_eq!(
            typed_first.value, untyped_first.value,
            "{} typed and untyped results diverged",
            case.path,
        );
        assert_eq!(
            typed_first.output, untyped_first.output,
            "{} typed and untyped output diverged",
            case.path,
        );

        visited += 1;
    }
    assert_eq!(visited, VM_BENCHMARK_CASES.len());
}

#[test]
fn physical_planning_real_fixture_prefix_reports_separate_scratch_and_retained_bytes() {
    for case in VM_BENCHMARK_CASES
        .iter()
        .take(VM_PHYSICAL_PLANNING_CASE_COUNT)
    {
        let verified = compile_case(case, true);
        let plan = ori_vm::prepare(&verified, ori_vm::PhysicalOptions::default())
            .unwrap_or_else(|error| panic!("{} should prepare physically: {error}", case.path));
        let metrics = plan.metrics();
        assert_eq!(metrics.planning_scratch_current_payload_bytes, 0);
        assert_eq!(metrics.validation_scratch_current_payload_bytes, 0);
        eprintln!(
            "physical_planning case={} retained_plan_bytes={} scratch_current_payload_bytes={} scratch_peak_payload_bytes_lower_bound={} scratch_cumulative_allocation_bytes_lower_bound={} validation_scratch_current_payload_bytes={} validation_scratch_peak_payload_bytes_lower_bound={} validation_scratch_cumulative_allocation_bytes_lower_bound={}",
            case.name,
            metrics.owned_plan_bytes,
            metrics.planning_scratch_current_payload_bytes,
            metrics.planning_scratch_peak_payload_bytes_lower_bound,
            metrics.planning_scratch_cumulative_allocation_bytes_lower_bound,
            metrics.validation_scratch_current_payload_bytes,
            metrics.validation_scratch_peak_payload_bytes_lower_bound,
            metrics.validation_scratch_cumulative_allocation_bytes_lower_bound,
        );
    }
}
