//! Memory Safety Stress Tests
//!
//! Tests that push the ARC pipeline to its limits: high allocation counts,
//! complex ownership patterns, large collections, and deep recursion with
//! RC'd values. Every test runs with `ORI_CHECK_LEAKS=1` — any refcount
//! imbalance causes exit code 2 (leak detected).
//!
//! These tests complement `stress.rs` (which focuses on scale/throughput)
//! with tests specifically designed to catch ARC correctness bugs:
//! use-after-free, double-free, and leak scenarios.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

// ─── High allocation count (10,000+) ───

#[test]
fn test_mem_10k_struct_allocations() {
    assert_aot_success(
        include_str!("fixtures/memory_stress/mem_10k_struct_allocations.ori"),
        "mem_10k_struct_allocs",
    );
}

#[test]
fn test_mem_10k_nested_struct_allocations() {
    assert_aot_success(
        include_str!("fixtures/memory_stress/mem_10k_nested_struct_allocations.ori"),
        "mem_10k_nested_struct_allocs",
    );
}

#[test]
fn test_mem_10k_string_concat_and_discard() {
    // Allocates and discards 10,000 strings — tests that intermediate
    // string allocations from concat are properly freed each iteration.
    assert_aot_success(
        include_str!("fixtures/memory_stress/mem_10k_string_concat_and_discard.ori"),
        "mem_10k_string_discard",
    );
}

// ─── Large collections (10,000+ elements) ───

#[test]
fn test_mem_large_list_10k_elements() {
    assert_aot_success(
        include_str!("fixtures/memory_stress/mem_large_list_10k_elements.ori"),
        "mem_large_list_10k",
    );
}

#[test]
fn test_mem_large_list_10k_filter_collect() {
    // Build 10K list, filter to ~5K, collect. Two large allocations that
    // must both be freed. Tests iterator + collect allocation lifecycle.
    assert_aot_success(
        include_str!("fixtures/memory_stress/mem_large_list_10k_filter_collect.ori"),
        "mem_large_list_10k_filter",
    );
}

#[test]
fn test_mem_large_list_map_chain() {
    // Tests iterator adapter allocation at scale: each adapter wraps
    // the previous one. At 10K iterations all must be properly dropped.
    assert_aot_success(
        include_str!("fixtures/memory_stress/mem_large_list_map_chain.ori"),
        "mem_large_list_map_chain",
    );
}

// ─── Diamond sharing (multiple owners of same allocation) ───

#[test]
fn test_mem_diamond_sharing_struct() {
    // Classic diamond pattern: one allocation, multiple independent owners.
    // If RC is wrong, either use-after-free or leak.
    assert_aot_success(
        include_str!("fixtures/memory_stress/mem_diamond_sharing_struct.ori"),
        "mem_diamond_sharing",
    );
}

#[test]
fn test_mem_diamond_sharing_in_loop() {
    // Diamond sharing created and destroyed 1000 times. Each iteration
    // creates multiple refs to one allocation, then all go out of scope.
    assert_aot_success(
        include_str!("fixtures/memory_stress/mem_diamond_sharing_in_loop.ori"),
        "mem_diamond_in_loop",
    );
}

#[test]
fn test_mem_diamond_struct_in_struct() {
    // Shared allocation referenced from two different struct fields.
    // Tests that struct drop decrements the shared inner correctly.
    assert_aot_success(
        include_str!("fixtures/memory_stress/mem_diamond_struct_in_struct.ori"),
        "mem_diamond_in_struct",
    );
}

#[test]
fn test_mem_diamond_closure_capture() {
    // Two closures capture the same RC'd value. Both must inc on capture,
    // both must dec when the closure is dropped.
    assert_aot_success(
        include_str!("fixtures/memory_stress/mem_diamond_closure_capture.ori"),
        "mem_diamond_closure",
    );
}

// ─── Deep recursion with RC'd values ───

#[test]
fn test_mem_deep_recursion_200_with_strings() {
    // Each recursive call creates a new string-containing struct.
    // Tests that all 200 allocations are properly freed as the stack unwinds.
    assert_aot_success(
        include_str!("fixtures/memory_stress/mem_deep_recursion_200_with_strings.ori"),
        "mem_deep_recursion_strings",
    );
}

#[test]
fn test_mem_deep_recursion_100_shared_param() {
    // Passes a shared (RC'd) struct through 100 levels of recursion.
    // Each call borrows the struct — tests that RC is maintained correctly
    // across deep call stacks.
    assert_aot_success(
        include_str!("fixtures/memory_stress/mem_deep_recursion_100_shared_param.ori"),
        "mem_deep_recursion_shared",
    );
}

// ─── Multi-function RC passing chains ───

#[test]
fn test_mem_function_chain_10_deep() {
    // RC'd struct passed through 10 functions. Each function receives,
    // reads a field, and passes to the next. Tests transitive RC tracking.
    assert_aot_success(
        include_str!("fixtures/memory_stress/mem_function_chain_10_deep.ori"),
        "mem_chain_10_deep",
    );
}

#[test]
fn test_mem_function_chain_with_transform() {
    // Each function transforms the struct (creating new allocation) and
    // passes it on. Tests that intermediate allocations are freed.
    assert_aot_success(
        include_str!("fixtures/memory_stress/mem_function_chain_with_transform.ori"),
        "mem_chain_transform_500",
    );
}

// ─── Complex ownership: multiple structs sharing inner allocations ───

#[test]
fn test_mem_shared_inner_multiple_containers() {
    // One inner struct shared across multiple outer structs.
    // When outers are dropped, inner must survive until last ref dies.
    assert_aot_success(
        include_str!("fixtures/memory_stress/mem_shared_inner_multiple_containers.ori"),
        "mem_shared_inner_containers",
    );
}

#[test]
fn test_mem_reassignment_frees_old_value() {
    // Reassigning a variable holding an RC'd struct must free the old value.
    // 10,000 reassignments = 10,000 allocations that must be freed.
    assert_aot_success(
        include_str!("fixtures/memory_stress/mem_reassignment_frees_old_value.ori"),
        "mem_reassign_frees_old",
    );
}

// ─── Closure lifecycle stress ───

#[test]
fn test_mem_closure_capture_in_loop() {
    // Creates and destroys 1000 closures, each capturing an RC'd struct.
    // Tests that closure env + captured values are properly freed.
    assert_aot_success(
        include_str!("fixtures/memory_stress/mem_closure_capture_in_loop.ori"),
        "mem_closure_in_loop",
    );
}

#[test]
fn test_mem_closure_escapes_scope() {
    // Closure outlives the scope where its captures were created.
    // The captured value must be kept alive by the closure's RC.
    assert_aot_success(
        include_str!("fixtures/memory_stress/mem_closure_escapes_scope.ori"),
        "mem_closure_escapes",
    );
}

// ─── Mixed stress: combines multiple ownership patterns ───

#[test]
fn test_mem_combined_allocation_storm() {
    // Creates structs, closures, lists, strings, and shared references
    // all in one tight loop. Maximum allocation pressure.
    assert_aot_success(
        include_str!("fixtures/memory_stress/mem_combined_allocation_storm.ori"),
        "mem_combined_storm",
    );
}

#[test]
fn test_mem_interleaved_alloc_free() {
    // Alternating allocations and frees test that the allocator handles
    // fragmented free lists correctly.
    assert_aot_success(
        include_str!("fixtures/memory_stress/mem_interleaved_alloc_free.ori"),
        "mem_interleaved_alloc_free",
    );
}
