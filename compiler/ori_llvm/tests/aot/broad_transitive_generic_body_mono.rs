//! AOT coverage for nested calls inside concrete generic-body instantiations.
//!
//! Trait methods, closures, constructors, runtime symbols, and generic callees
//! must receive concrete realization records. Each fixture runs through the
//! production `ori build` entry point with leak checking enabled.

use crate::util::assert_aot_success;

#[test]
fn assert_eq_on_generic_struct_in_generic_body_monos() {
    assert_aot_success(
        include_str!("fixtures/broad_transitive_generic_body_mono/assert_eq.ori"),
        "broad_transitive_assert_eq",
    );
}

#[test]
fn empty_queue_generic_ctor_in_generic_body_monos() {
    assert_aot_success(
        include_str!("fixtures/broad_transitive_generic_body_mono/empty_queue.ori"),
        "broad_transitive_empty_queue",
    );
}

#[test]
fn dequeue_generic_fn_in_generic_body_monos() {
    assert_aot_success(
        include_str!("fixtures/broad_transitive_generic_body_mono/dequeue.ori"),
        "broad_transitive_dequeue",
    );
}

#[test]
fn buffer_pop_generic_fn_in_generic_body_monos() {
    assert_aot_success(
        include_str!("fixtures/broad_transitive_generic_body_mono/buffer_pop.ori"),
        "broad_transitive_buffer_pop",
    );
}

#[test]
fn thread_id_nested_in_generic_body_monos() {
    assert_aot_success(
        include_str!("fixtures/broad_transitive_generic_body_mono/thread_id.ori"),
        "broad_transitive_thread_id",
    );
}

// Prelude generic `repeat<T: Clone>` resolves through a rigid `U` caller.
#[test]
fn repeat_prelude_generic_in_generic_body_stays_resolved() {
    assert_aot_success(
        include_str!("fixtures/broad_transitive_generic_body_mono/repeat_guard.ori"),
        "broad_transitive_repeat_guard",
    );
}
