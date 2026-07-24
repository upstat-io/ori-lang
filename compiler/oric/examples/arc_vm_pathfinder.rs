//! Thin prototype harness for the production-shaped bytecode VM seams.

use std::env;
use std::fs;
use std::io::{self, Write};
use std::process;
use std::time::Instant;

fn main() {
    let (path, typed_primitives) = source_arguments();
    let source = fs::read_to_string(&path).unwrap_or_else(|error| {
        report_status("failed", 0, 0, 0, 1);
        eprintln!("error: cannot read {path}: {error}");
        process::exit(2);
    });

    let frontend_start = Instant::now();
    let executable = oric::test_support::compile_to_executable(
        &path,
        &source,
        ori_repr::NarrowingPolicy::Aggressive,
    )
    .unwrap_or_else(|error| {
        report_status("failed", 0, 0, 0, 1);
        eprintln!("error: {error}");
        process::exit(1);
    });
    let frontend_elapsed = frontend_start.elapsed();
    let bytecode_start = Instant::now();
    let bytecode =
        ori_vm::compile_with_options(&executable, ori_vm::CompileOptions { typed_primitives })
            .unwrap_or_else(|error| {
                report_status("unsupported", 0, 0, 1, 0);
                eprintln!("error: {error}");
                process::exit(3);
            });
    if env::var_os("ORI_VM_DUMP_BYTECODE").is_some() {
        eprintln!("{bytecode:#?}");
    }
    let bytecode = ori_vm::verify(bytecode).unwrap_or_else(|error| {
        report_status("failed", 0, 0, 0, 1);
        eprintln!("bytecode verifier error: {error}");
        process::exit(1);
    });
    let bytecode_elapsed = bytecode_start.elapsed();
    if env::var_os("ORI_VM_PHYSICAL_PLAN").is_some() {
        let physical = ori_vm::prepare(&bytecode, ori_vm::PhysicalOptions::default())
            .unwrap_or_else(|error| {
                report_status("failed", 0, 0, 0, 1);
                eprintln!("physical plan error: {error}");
                process::exit(1);
            });
        report_physical_plan(&physical);
    }

    let vm_start = Instant::now();
    let (report, profile) = if env::var_os("ORI_VM_PROFILE").is_some() {
        let profiled = ori_vm::execute_profiled(&bytecode, ori_vm::ExecutionConfig::default());
        (profiled.execution, Some(profiled.profile))
    } else {
        (
            ori_vm::execute_report(&bytecode, ori_vm::ExecutionConfig::default()),
            None,
        )
    };
    let vm_elapsed = vm_start.elapsed();

    let value = match report.result {
        Ok(value) => value,
        Err(error) => {
            report_vm_metrics("failed", &report.metrics);
            eprintln!("error: {error}");
            process::exit(1);
        }
    };

    io::stdout()
        .write_all(&report.output)
        .unwrap_or_else(|error| {
            eprintln!("error: cannot write VM output: {error}");
            process::exit(1);
        });
    report_success_metrics(
        typed_primitives,
        bytecode.metrics(),
        &value,
        &report.metrics,
        [frontend_elapsed, bytecode_elapsed, vm_elapsed],
    );
    if let Some(profile) = &profile {
        report_dispatch_profile(profile);
    }
}

fn report_physical_plan(plan: &ori_vm::PhysicalVmPlan<'_>) {
    let sizes = plan.element_sizes();
    eprintln!(
        "physical_element_sizes canonical_op={} physical_function_plan={} physical_op_plan={} physical_pc_plan={} physical_read={} physical_write={} physical_lane={}",
        sizes.canonical_op,
        sizes.physical_function_plan,
        sizes.physical_op_plan,
        sizes.physical_pc_plan,
        sizes.physical_read,
        sizes.physical_write,
        sizes.physical_lane,
    );
    let metrics = plan.metrics();
    eprintln!(
        "physical_plan functions={} canonical_ops={} physical_ops={} reads={} writes={} lanes={} immediate_bindings={} coalesced_copies={} canonical_op_bytes={} owned_plan_bytes={} retained_canonical_and_plan_bytes={} planning_scratch_current_payload_bytes={} planning_scratch_peak_payload_bytes_lower_bound={} planning_scratch_cumulative_allocation_bytes_lower_bound={} validation_scratch_current_payload_bytes={} validation_scratch_peak_payload_bytes_lower_bound={} validation_scratch_cumulative_allocation_bytes_lower_bound={}",
        plan.function_count(),
        metrics.canonical_ops,
        metrics.physical_ops,
        metrics.read_bindings,
        metrics.write_bindings,
        metrics.physical_lanes,
        metrics.immediate_bindings,
        metrics.coalesced_copies,
        metrics.canonical_op_bytes,
        metrics.owned_plan_bytes,
        metrics.retained_canonical_and_plan_bytes,
        metrics.planning_scratch_current_payload_bytes,
        metrics.planning_scratch_peak_payload_bytes_lower_bound,
        metrics.planning_scratch_cumulative_allocation_bytes_lower_bound,
        metrics.validation_scratch_current_payload_bytes,
        metrics.validation_scratch_peak_payload_bytes_lower_bound,
        metrics.validation_scratch_cumulative_allocation_bytes_lower_bound,
    );
    for function_index in 0..plan.function_count() {
        let metrics = plan
            .function_storage_metrics(function_index)
            .unwrap_or_else(|error| {
                panic!("physical function {function_index} must have metrics: {error}")
            });
        eprintln!(
            "physical_function function={} canonical_ops={} physical_ops={} physical_pcs={} reads={} writes={} lanes={} canonical_op_bytes={} physical_function_plan_bytes={} physical_op_bytes={} physical_pc_bytes={} physical_read_bytes={} physical_write_bytes={} physical_lane_bytes={} owned_plan_bytes={} retained_canonical_and_plan_bytes={}",
            function_index,
            metrics.canonical_ops,
            metrics.physical_ops,
            metrics.physical_pcs,
            metrics.reads,
            metrics.writes,
            metrics.lanes,
            metrics.canonical_op_bytes,
            metrics.physical_function_plan_bytes,
            metrics.physical_op_bytes,
            metrics.physical_pc_bytes,
            metrics.physical_read_bytes,
            metrics.physical_write_bytes,
            metrics.physical_lane_bytes,
            metrics.owned_plan_bytes,
            metrics.retained_canonical_and_plan_bytes,
        );
    }
}

fn report_success_metrics(
    typed_primitives: bool,
    bytecode: ori_vm::BytecodeMetrics,
    value: &ori_vm::ExitValue,
    metrics: &ori_vm::ExecutionMetrics,
    elapsed: [std::time::Duration; 3],
) {
    eprintln!(
        "status=full_vm entered_vm=1 fallback=0 unsupported=0 failed=0 typed_primitives={} functions={} bytecode_ops={} result={value:?} steps={} frames_peak={} heap_allocations={} heap_payload_allocated_lower_bound={} heap_exit_live={} heap_exit_bytes={} heap_final_live={} heap_final_bytes={} heap_peak={} heap_peak_bytes={} heap_table_peak_bytes={} value_arena_allocations={} value_arena_aggregate_allocations={} value_arena_iterator_allocations={} value_arena_collections={} value_arena_reclaimed={} value_arena_reused={} value_arena_exit_entries={} value_arena_exit_slots={} value_arena_exit_bytes={} value_arena_final_entries={} value_arena_final_slots={} value_arena_final_bytes={} value_arena_peak_entries={} value_arena_peak_slots={} value_arena_peak_bytes={} frame_peak_bytes={} register_peak_bytes={} scratch_peak_bytes={} ownership_scratch_peak_bytes={} output_peak_bytes={} frontend_ms={:.3} bytecode_ms={:.3} vm_ms={:.3}",
        u8::from(typed_primitives),
        bytecode.function_count,
        bytecode.instruction_count,
        metrics.steps,
        metrics.peak_frames,
        metrics.cumulative_heap_allocations,
        metrics.cumulative_heap_payload_bytes_lower_bound,
        metrics.exit_live_heap_objects,
        metrics.exit_live_heap_payload_bytes,
        metrics.final_live_heap_objects,
        metrics.final_live_heap_payload_bytes,
        metrics.peak_heap_objects,
        metrics.peak_heap_payload_bytes,
        metrics.peak_heap_table_owned_bytes,
        metrics.cumulative_value_arena_allocations,
        metrics.cumulative_value_arena_aggregate_allocations,
        metrics.cumulative_value_arena_iterator_allocations,
        metrics.cumulative_value_arena_collections,
        metrics.cumulative_value_arena_reclaimed_entries,
        metrics.cumulative_value_arena_reused_entries,
        metrics.exit_value_arena_entries,
        metrics.exit_value_arena_slots,
        metrics.exit_value_arena_owned_bytes,
        metrics.final_value_arena_entries,
        metrics.final_value_arena_slots,
        metrics.final_value_arena_owned_bytes,
        metrics.peak_value_arena_entries,
        metrics.peak_value_arena_slots,
        metrics.peak_value_arena_owned_bytes,
        metrics.peak_frame_owned_bytes,
        metrics.peak_register_owned_bytes,
        metrics.peak_scratch_owned_bytes,
        metrics.peak_ownership_scratch_owned_bytes,
        metrics.peak_output_owned_bytes,
        elapsed[0].as_secs_f64() * 1000.0,
        elapsed[1].as_secs_f64() * 1000.0,
        elapsed[2].as_secs_f64() * 1000.0,
    );
}

fn source_arguments() -> (String, bool) {
    let mut arguments = env::args().skip(1).peekable();
    let typed_primitives = if arguments.peek().is_some_and(|value| value == "--untyped") {
        arguments.next();
        false
    } else {
        true
    };
    let Some(path) = arguments.next() else {
        eprintln!("usage: arc_vm_pathfinder [--untyped] <source.ori>");
        process::exit(2);
    };
    if arguments.next().is_some() {
        eprintln!("error: unexpected extra argument");
        process::exit(2);
    }
    (path, typed_primitives)
}

fn report_status(status: &str, entered_vm: u8, fallback: u8, unsupported: u8, failed: u8) {
    eprintln!(
        "status={status} entered_vm={entered_vm} fallback={fallback} unsupported={unsupported} failed={failed}"
    );
}

fn report_vm_metrics(status: &str, metrics: &ori_vm::ExecutionMetrics) {
    eprintln!(
        "status={status} entered_vm=1 fallback=0 unsupported=0 failed=1 steps={} heap_exit_live={} heap_exit_bytes={} heap_final_live={} heap_final_bytes={} value_arena_allocations={} value_arena_aggregate_allocations={} value_arena_iterator_allocations={} value_arena_collections={} value_arena_reclaimed={} value_arena_reused={} value_arena_exit_entries={} value_arena_exit_slots={} value_arena_exit_bytes={} value_arena_final_entries={} value_arena_final_slots={} value_arena_final_bytes={} value_arena_peak_entries={} value_arena_peak_slots={} value_arena_peak_bytes={} ownership_scratch_peak_bytes={}",
        metrics.steps,
        metrics.exit_live_heap_objects,
        metrics.exit_live_heap_payload_bytes,
        metrics.final_live_heap_objects,
        metrics.final_live_heap_payload_bytes,
        metrics.cumulative_value_arena_allocations,
        metrics.cumulative_value_arena_aggregate_allocations,
        metrics.cumulative_value_arena_iterator_allocations,
        metrics.cumulative_value_arena_collections,
        metrics.cumulative_value_arena_reclaimed_entries,
        metrics.cumulative_value_arena_reused_entries,
        metrics.exit_value_arena_entries,
        metrics.exit_value_arena_slots,
        metrics.exit_value_arena_owned_bytes,
        metrics.final_value_arena_entries,
        metrics.final_value_arena_slots,
        metrics.final_value_arena_owned_bytes,
        metrics.peak_value_arena_entries,
        metrics.peak_value_arena_slots,
        metrics.peak_value_arena_owned_bytes,
        metrics.peak_ownership_scratch_owned_bytes,
    );
}

fn report_dispatch_profile(profile: &ori_vm::ExecutionProfile<'_>) {
    eprintln!("profile_dispatches={}", profile.dispatches);

    let mut opcodes = profile.opcodes.clone();
    opcodes.sort_by(|left, right| {
        right
            .dispatches
            .cmp(&left.dispatches)
            .then_with(|| left.opcode.cmp(&right.opcode))
    });
    for row in opcodes {
        eprintln!(
            "profile_opcode opcode={} dispatches={}",
            row.opcode.name(),
            row.dispatches
        );
    }

    report_pairs("all", &profile.all_pairs);
    report_pairs("linear", &profile.linear_fallthrough_pairs);

    let mut regions = profile.regions.clone();
    regions.sort_by(|left, right| {
        right
            .dispatches
            .cmp(&left.dispatches)
            .then_with(|| left.function.cmp(&right.function))
            .then_with(|| left.start.cmp(&right.start))
    });
    for row in regions.into_iter().take(32) {
        eprintln!(
            "profile_region function={} start={} end={} entries={} dispatches={}",
            row.function.index(),
            row.start.index(),
            row.end.index(),
            row.entries,
            row.dispatches
        );
    }
}

fn report_pairs(kind: &str, pairs: &[ori_vm::OpcodePairCount]) {
    let mut pairs = pairs.to_vec();
    pairs.sort_by(|left, right| {
        right
            .dispatches
            .cmp(&left.dispatches)
            .then_with(|| left.first.cmp(&right.first))
            .then_with(|| left.second.cmp(&right.second))
    });
    for row in pairs.into_iter().take(32) {
        eprintln!(
            "profile_pair kind={kind} first={} second={} dispatches={}",
            row.first.name(),
            row.second.name(),
            row.dispatches
        );
    }
}
