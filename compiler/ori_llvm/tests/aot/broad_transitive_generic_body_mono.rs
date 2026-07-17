//! AOT pins for the BROAD transitive-generic-body monomorphization gap
//! (ROOT-B): a generic function whose body calls a trait method, a passed
//! closure, or a generic constructor on its OWN rigid type param, invoked from
//! ANOTHER generic body with a concrete instantiation. The outer generic fn
//! records its own `MonoInstance`, but the nested call stays type-checked
//! against rigid `T` and is never recorded per concrete instantiation, so at
//! true AOT (`ori build` -> native binary) the dispatch path
//! (`arc_emitter/apply.rs::resolve_callee` -> `lookup_mono_dispatch`,
//! `mono_instance_id = None` fallback) starves and emission aborts with
//! `error[E5001]`: `unresolved function 'X' in apply -- missing mono
//! instance?`. The interpreter computes the value through dynamic dispatch,
//! so dual execution exposes the missing AOT instance.
//!
//! These cells drive the REAL `ori build` production entry point
//! (`compile_and_run_capture` shells out to the workspace `ori` binary and runs
//! the native binary under `ORI_CHECK_LEAKS=1`): L8 AOT, L12 production entry
//! point, L10 leak-freedom. Interpreter and AOT execution must agree for every
//! fixture.
//!
//! The `assert_eq`, `empty_queue`, `dequeue`, and `buffer_pop` cases require a
//! nested call record for each concrete instantiation. The `repeat` case covers
//! a directly resolvable form of the same shape.

use crate::util::assert_aot_success;

// Producer-closure guard — `assert_eq<Box<int>>` is discovered through the
// generic-calling-generic fixed point, then its `T: Debug` bound must seed the
// exact derived `Box<int>.debug` body before shared executable realization.
#[test]
fn assert_eq_on_generic_struct_in_generic_body_monos() {
    assert_aot_success(
        include_str!("fixtures/broad_transitive_generic_body_mono/assert_eq.ori"),
        "broad_transitive_assert_eq",
    );
}

// `make_empty<U>` requires the concrete nested `empty_queue<int>` instance.
#[test]
fn empty_queue_generic_ctor_in_generic_body_monos() {
    assert_aot_success(
        include_str!("fixtures/broad_transitive_generic_body_mono/empty_queue.ori"),
        "broad_transitive_empty_queue",
    );
}

// `drain<U>` requires the concrete nested `dequeue<int>` instance.
#[test]
fn dequeue_generic_fn_in_generic_body_monos() {
    assert_aot_success(
        include_str!("fixtures/broad_transitive_generic_body_mono/dequeue.ori"),
        "broad_transitive_dequeue",
    );
}

// `pop_one<U>` requires the concrete nested `buffer_pop<int>` instance.
#[test]
fn buffer_pop_generic_fn_in_generic_body_monos() {
    assert_aot_success(
        include_str!("fixtures/broad_transitive_generic_body_mono/buffer_pop.ori"),
        "broad_transitive_buffer_pop",
    );
}

// `thread_id` uses runtime-symbol codegen inside a generic body, independently
// of the enclosing `get_id<int>` monomorphization.
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
