//! AOT tests for the tagged-pointer enum optimization.
//!
//! Verifies the tagged-pointer behavioral contract end-to-end through the AOT
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
//!   `RcPointer { inner: OpaquePtr }`, which the optimization intentionally
//!   excludes from eligibility (recursive boxing-aware codegen for
//!   Construct/Project is future work).
//! - Channels (`OpaquePtr`) and iterators (`UnmanagedPtr`) are not
//!   ergonomic variant payloads in user code.
//!
//! As a result, the **negative pin** below is the most important AOT
//! test — it locks in that recursive enums (the most natural failure
//! mode) fall back to explicit-tag encoding and continue to execute
//! correctly. Without this, the recursive value-type AOT codegen hang would
//! reappear.
//!
//! ## JIT note
//!
//! These tests use AOT compilation instead of Ori spec tests because the
//! JIT test runner currently hangs on tagged-pointer enum spec tests
//! under directory sweep. AOT compilation works correctly
//! for the same source.
//!
//! tagged-pointer encoding.

use crate::util::{assert_aot_success, compile_to_llvm_ir};

/// **Negative pin (most important)**: recursive enums are NOT eligible
/// for tagged-pointer encoding. The cycle marker
/// (`RcPointer { inner: OpaquePtr }`) produced by `canonical_inner` is
/// rejected by `is_taggable_pointer` — the enum falls back to the
/// explicit-tag encoding. This test ensures recursive enums still
/// execute correctly under the explicit-tag path.
///
/// Without this exclusion, AOT codegen for
/// `IntCell = Empty | Holds(IntCell)` hangs because the
/// boxing semantics for recursive tagged-pointer fields are not
/// implemented by the tagged-pointer encoding.
#[test]
fn test_recursive_enum_falls_back_to_explicit_tag() {
    assert_aot_success(
        include_str!("fixtures/enum_tagged_ptr/recursive_depth.ori"),
        "recursive_enum_explicit_tag",
    );
}

/// Regression: explicit-tag enum Construct with `UnmanagedPtr` (iterator)
/// payload requires `ptrtoint` before `insertvalue` into `[N x i64]` slot.
/// Without the cast, LLVM rejects `insertvalue [1 x i64], ptr %p, 0`.
/// 10+ variants forces explicit-tag, iterator is ptr type.
#[test]
fn test_explicit_tag_enum_with_iterator_payload_compiles_and_runs() {
    assert_aot_success(
        include_str!("fixtures/enum_tagged_ptr/explicit_tag_iterator_payload.ori"),
        "explicit_tag_iterator_payload",
    );
}

/// Clamps tagged-pointer codegen for an enum satisfying
/// `can_use_tagged_pointer` (every non-unit variant carries a single
/// single-word pointer payload — `Iterator<int>` lowers to
/// `MachineRepr::UnmanagedPtr` per `is_taggable_pointer`).
///
/// This complements unit-test pins for `EnumTag::Explicit` and
/// Option/Result typed-payload variants at `arc_emitter/tests.rs` by
/// covering the third encoding (tagged-pointer), which requires running
/// the full pipeline through `canonical_enum` + `optimize_tagged_ptr_repr`
/// (the unit-test `TypeLayoutResolver` is constructed with `None`
/// `repr_plan`, masking the per-encoding tagged-pointer arm).
///
/// Assertion block-name deviation: the plan-body Pin 3 spec named
/// `tagged.encoded` / `tagged.tag` from `drop_enum.rs::emit_drop_enum_tagged_ptr`.
/// That helper fires only when a heap-allocated tagged-pointer enum gets a
/// generated `_ori_drop$<idx>` function — rare in practice because
/// tagged-ptr enums are 8 bytes and typically live inline in a parent
/// rather than as a standalone heap allocation (`drop_enum.rs` tagged-pointer
/// comment). The user-source path that does fire on a list-element decl
/// goes through `rc_helpers.rs::emit_tagged_ptr_enum_rc`, which emits
/// the same tagged-pointer dispatch shape (load encoded i64, decode tag,
/// switch, decode pointer, per-variant RC dec) but uses the
/// `rc_dec.tag` / `rc_dec.tp.ptr` / `rc_dec.done` block-name family. Both
/// helpers are entry points for tagged-pointer codegen — pinning the
/// shape exercised by user source follows the plan's requirement that assertions
/// match the actual emitted block-name shapes.
#[test]
fn test_burden_dec_variant_tagged_ptr_enum_emits_switch_and_rc_dec() {
    let ir = compile_to_llvm_ir(include_str!(
        "fixtures/enum_tagged_ptr/tagged_ptr_drop_burden_walk.ori"
    ))
    .expect("compilation failed");

    // The element-dec function is generated for the `[MaybeIter]` list
    // backing buffer; tagged-pointer dispatch fires inside it.
    assert!(
        ir.contains("_ori_elem_dec$"),
        "expected an `_ori_elem_dec$<idx>` function for the `[MaybeIter]` \
         backing buffer (RC dec on each element at buffer teardown)."
    );
    // `rc_dec.tag` — decoded 3-bit tag from the tagged-pointer encoded i64
    // (`rc_helpers.rs::emit_tagged_ptr_enum_rc` via `tagged_ptr_decode_tag`).
    assert!(
        ir.contains("rc_dec.tag"),
        "tagged-ptr dispatch must decode the tag into a value named \
         `rc_dec.tag`. Available drop / elem-dec functions in IR:\n{}",
        ir.lines()
            .filter(|l| l.starts_with("define ")
                && (l.contains("_ori_drop") || l.contains("_ori_elem_dec")))
            .take(10)
            .collect::<Vec<_>>()
            .join("\n")
    );
    // Per-variant dispatch is implemented as an LLVM `switch` on the
    // decoded tag.
    assert!(
        ir.contains("switch"),
        "tagged-ptr dispatch must emit `switch` on the decoded tag."
    );
    // Per-variant pointer decode reaching the per-variant block
    // (`rc_dec.tp.ptr` — `tagged_ptr_decode_ptr` extracts the high 61 bits
    // of the encoded i64 for the iterator-payload variant).
    assert!(
        ir.contains("rc_dec.tp.ptr"),
        "tagged-ptr dispatch must decode the pointer into a value named \
         `rc_dec.tp.ptr` for per-variant cleanup."
    );
    // The iterator-payload variant decrements via `ori_iter_drop` (RC dec
    // on an iterator handle is its drop call); the runtime call matches
    // the plan-body's `ori_rc_dec` semantic intent (per-variant RC dec)
    // adjusted for the iterator payload type.
    assert!(
        ir.contains("ori_iter_drop"),
        "tagged-ptr dispatch must call `ori_iter_drop` on the decoded \
         iterator handle (the per-variant RC dec for `Iterator<int>` payload)."
    );
}
