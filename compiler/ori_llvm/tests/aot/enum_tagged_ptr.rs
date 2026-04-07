//! AOT tests for §07.3 Tagged Pointer Optimization.
//!
//! Verifies the §07.3.A behavioral contract end-to-end through the AOT
//! pipeline. The Rust unit tests in
//! `ori_repr/src/layout/tests.rs` cover the optimizer's analysis logic
//! against synthetic `MachineRepr` values; this file covers what user
//! source produces when compiled all the way to native code.
//!
//! ## Eligibility realities
//!
//! Tagged-pointer encoding requires a variant payload that is a
//! single-word pointer (`OpaquePtr` / `UnmanagedPtr` / non-recursive
//! `RcPointer`). In current Ori syntax there is **no easy non-recursive
//! way** to produce such a payload from user source:
//!
//! - FFI types like `CPtr` resolve to `Idx::INT` (64-bit integer), not
//!   pointers (see `well_known/mod.rs::resolve_ffi_concrete`).
//! - Recursive enums produce the cycle marker
//!   `RcPointer { inner: OpaquePtr }`, which §07.3.A intentionally
//!   excludes from eligibility (recursive boxing-aware codegen for
//!   Construct/Project is future work).
//! - Channels (`OpaquePtr`) and iterators (`UnmanagedPtr`) are not
//!   ergonomic variant payloads in user code.
//!
//! As a result, the **negative pin** below is the most important AOT
//! test — it locks in that recursive enums (the most natural failure
//! mode) fall back to explicit-tag encoding and continue to execute
//! correctly. Without this, BUG-04-043 (AOT codegen hang) would
//! reappear.
//!
//! ## JIT note
//!
//! These tests use AOT compilation instead of Ori spec tests because the
//! JIT test runner currently hangs on tagged-pointer enum spec tests
//! under directory sweep (BUG-04-043). AOT compilation works correctly
//! for the same source.
//!
//! Spec: `plans/repr-opt/section-07-enum-repr.md` §07.3 / §07.3.A.

use crate::util::assert_aot_success;

/// **Negative pin (most important)**: recursive enums are NOT eligible
/// for tagged-pointer encoding. The cycle marker
/// (`RcPointer { inner: OpaquePtr }`) produced by `canonical_inner` is
/// rejected by `is_taggable_pointer` — the enum falls back to the
/// explicit-tag encoding. This test ensures recursive enums still
/// execute correctly under the explicit-tag path.
///
/// Without this exclusion, AOT codegen for
/// `IntCell = Empty | Holds(IntCell)` hangs (BUG-04-043) because the
/// boxing semantics for recursive tagged-pointer fields are not
/// implemented in §07.3.A.
#[test]
fn test_recursive_enum_falls_back_to_explicit_tag() {
    assert_aot_success(
        include_str!("fixtures/enum_tagged_ptr/recursive_depth.ori"),
        "recursive_enum_explicit_tag",
    );
}
