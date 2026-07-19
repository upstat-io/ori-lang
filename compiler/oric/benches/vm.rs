#[path = "../tests/support/vm_benchmark_cases.rs"]
mod vm_benchmark_cases;

use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use ori_vm::ExecutionConfig;

use vm_benchmark_cases::{compile_case, VM_BENCHMARK_CASES, VM_PHYSICAL_PLANNING_CASE_COUNT};

fn benchmark_vm(c: &mut Criterion) {
    let mut group = c.benchmark_group("vm/execution");
    for case in VM_BENCHMARK_CASES {
        let verified = compile_case(case, true);
        let generic = ori_vm::execute_report(&verified, ExecutionConfig::default());
        let generic_value = generic
            .result
            .as_ref()
            .unwrap_or_else(|error| panic!("{} generic baseline failed: {error}", case.path));
        assert_eq!(generic_value, &case.expected_value);
        assert_eq!(generic.output.as_slice(), case.expected_output);
        assert_eq!(generic.metrics.final_live_heap_objects, 0);
        assert_eq!(generic.metrics.final_live_heap_payload_bytes, 0);
        assert_eq!(generic.metrics.final_value_arena_entries, 0);
        assert_eq!(generic.metrics.final_value_arena_slots, 0);
        assert_eq!(generic.metrics.final_value_arena_owned_bytes, 0);

        let physical_plan = ori_vm::prepare(&verified, ori_vm::PhysicalOptions::default())
            .unwrap_or_else(|error| panic!("{} should prepare physically: {error}", case.path));
        let physical = ori_vm::execute_physical_report(&physical_plan, ExecutionConfig::default());
        let physical_value = physical
            .result
            .as_ref()
            .unwrap_or_else(|error| panic!("{} physical baseline failed: {error}", case.path));
        assert_eq!(physical_value, generic_value);
        assert_eq!(physical.output, generic.output);
        assert_eq!(physical.metrics, generic.metrics);

        group.bench_with_input(
            BenchmarkId::new("generic", case.name),
            &verified,
            |b, program| {
                b.iter(|| {
                    black_box(ori_vm::execute_report(
                        black_box(program),
                        black_box(ExecutionConfig::default()),
                    ))
                });
            },
        );
        group.bench_with_input(
            BenchmarkId::new("physical", case.name),
            &physical_plan,
            |b, plan| {
                b.iter(|| {
                    let report = ori_vm::execute_physical_report(
                        black_box(plan),
                        black_box(ExecutionConfig::default()),
                    );
                    black_box(report)
                });
            },
        );
    }
    group.finish();
}

fn benchmark_physical_planning(c: &mut Criterion) {
    let mut group = c.benchmark_group("vm/physical_planning");
    for case in VM_BENCHMARK_CASES
        .iter()
        .take(VM_PHYSICAL_PLANNING_CASE_COUNT)
    {
        let verified = compile_case(case, true);
        let baseline = ori_vm::prepare(&verified, ori_vm::PhysicalOptions::default())
            .unwrap_or_else(|error| panic!("{} should prepare physically: {error}", case.path));
        assert_eq!(baseline.metrics().planning_scratch_current_payload_bytes, 0);
        assert_eq!(
            baseline.metrics().validation_scratch_current_payload_bytes,
            0
        );

        group.bench_with_input(
            BenchmarkId::from_parameter(case.name),
            &verified,
            |b, program| {
                b.iter(|| {
                    ori_vm::prepare(
                        black_box(program),
                        black_box(ori_vm::PhysicalOptions::default()),
                    )
                    .unwrap_or_else(|error| {
                        panic!("{} physical planning benchmark failed: {error}", case.path)
                    })
                });
            },
        );
    }
    group.finish();
}

fn benchmark_branded_plan_read(c: &mut Criterion) {
    // Keep the historical group identity so Criterion compares the branded
    // read directly with the former whole-plan revalidation cost.
    let mut group = c.benchmark_group("vm/physical_revalidation_upper_bound");
    for case in VM_BENCHMARK_CASES
        .iter()
        .take(VM_PHYSICAL_PLANNING_CASE_COUNT)
    {
        let verified = compile_case(case, true);
        let plan = ori_vm::prepare(&verified, ori_vm::PhysicalOptions::default())
            .unwrap_or_else(|error| panic!("{} should prepare physically: {error}", case.path));
        // A branded immutable plan computes this constant-size report without
        // repeating construction validation.
        plan.function_storage_metrics(0).unwrap_or_else(|error| {
            panic!("{} branded plan storage read failed: {error}", case.path)
        });

        group.bench_with_input(BenchmarkId::from_parameter(case.name), &plan, |b, plan| {
            b.iter(|| {
                black_box(plan.function_storage_metrics(black_box(0))).unwrap_or_else(|error| {
                    panic!("{} branded plan storage read failed: {error}", case.path)
                })
            });
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    benchmark_vm,
    benchmark_physical_planning,
    benchmark_branded_plan_read
);
criterion_main!(benches);
