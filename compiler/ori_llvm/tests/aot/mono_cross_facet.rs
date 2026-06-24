//! Cross-facet AOT matrix — each test crosses >=2 regrounded mono-emission
//! paths SIMULTANEOUSLY, so a regression in any one surfaces on the AOT path
//! where the single-path pins cannot reach it.
//!
//! Paths crossed (per cell): cross-module import (`import_sig_by_name` mono
//! dispatch), complex-generic-arg instantiation, builtin Apply/cast
//! (`__cast` interception), typeck callee-var-mapping spine, trait-impl method
//! mono on a generic receiver, derived-Debug on a composite, and
//! `DoubleEndedIterator` consumers (rev / rfind / last) on a heap element.
//!
//! Reads stay on scalars / str / lists — a struct or `Option<T>` value is
//! never passed to `assert_eq` (that exact shape fails LLVM codegen under open
//! BUG-04-191), recursive derives are avoided (open BUG-04-216), and a heap
//! collection is never extracted out of an `Option` / generic method to a live
//! binding (that shape leaks an allocation under the active RC-emission
//! migration, tracked by open BUG-04-219). Each `assert_cell_output` /
//! multi-file gate enables `ORI_CHECK_LEAKS=1`, so every cell is also a leak
//! gate. The printed value is the semantic pin: a wrong mono dispatch, dropped
//! cast, or mis-ordered DEI walk changes it.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "consistency with sibling AOT fixture modules"
)]

use crate::util::{assert_cell_output, assert_multifile_cell_output};

const XFACET_IDLIB: &str = include_str!("fixtures/mono_cross_facet/xfacet_idlib.ori");

// Diagonal 1: cross-module import x complex-generic arg ([int]) x builtin
// Apply/cast. The imported generic `id<[int]>` + `first<int>` mono instances
// lower in the host's merged pool; the `[int]` result's length routes through
// the `__cast` protocol-builtin Apply. Reverting the `import_sig_by_name`
// lookup re-surfaces E5001 for both imported generics. Semantic pin: "3 10".
#[test]
fn test_import_x_complex_generic_cast() {
    assert_multifile_cell_output(
        &[
            (
                "host.ori",
                include_str!("fixtures/mono_cross_facet/host_import_x_complex_cast.ori"),
            ),
            ("xfacet_idlib.ori", XFACET_IDLIB),
        ],
        "import_x_complex_generic_cast",
        "3 10",
    );
}

// Diagonal 2: complex-generic arg (`[Option<int>]`) x builtin Apply/cast
// (`len() as int`, `unwrap() as int`). The nested-generic container routes its
// length and element through the `__cast` protocol-builtin Apply. Semantic
// pin: "3 10".
#[test]
fn test_complex_generic_x_builtin_cast() {
    assert_cell_output(
        include_str!("fixtures/mono_cross_facet/complex_generic_x_builtin_cast.ori"),
        "complex_generic_x_builtin_cast",
        "3 10",
    );
}

// Diagonal 3: trait-impl method (`p.norm()`) x derived-Debug (`p.debug()`) +
// derived-Eq (`==`) on a composite. The composite is read via scalars and the
// debug string (`.contains`), never passed to `assert_eq` (BUG-04-191).
// Semantic pin: "25 true true" (norm, derived-Eq verdict, debug-field presence).
#[test]
fn test_trait_impl_x_derived_debug() {
    assert_cell_output(
        include_str!("fixtures/mono_cross_facet/trait_impl_x_derived_debug.ori"),
        "trait_impl_x_derived_debug",
        "25 true true",
    );
}

// Diagonal 4: DoubleEndedIterator (rev / rfind / last) x complex-generic
// element (str heap fat pointer). `rev().collect()` reverses a heap-element
// buffer (leak gate on the reversed-buffer element ownership); `rfind` walks
// from the back via `next_back`; `last` returns the final element. Semantic
// pin: "dcba by r".
#[test]
fn test_dei_x_complex_element() {
    assert_cell_output(
        include_str!("fixtures/mono_cross_facet/dei_x_complex_element.ori"),
        "dei_x_complex_element",
        "dcba by r",
    );
}

// Diagonal 5 (full diagonal): trait-impl method on a generic receiver +
// complex-generic arg (Box<Option<int>>, nested generic) + builtin cast in one
// mono lowering. Three regrounded paths cross in a single specialization.
// Semantic pin: "41".
#[test]
fn test_full_diagonal_trait_nested_generic_cast() {
    assert_cell_output(
        include_str!("fixtures/mono_cross_facet/full_diagonal.ori"),
        "full_diagonal_trait_nested_generic_cast",
        "41",
    );
}

// Diagonal 6: typeck callee-var-mapping spine x trait-impl method on a generic
// receiver. `relay<T>` threads its `Box<T>` into `inner<T>` (the callee-var
// spine), and the trait-impl `.get()` is dispatched on the concrete spine
// result. Both legs mono at `int` and `str`. Semantic pin: "11 z".
#[test]
fn test_callee_var_spine_x_trait_impl() {
    assert_cell_output(
        include_str!("fixtures/mono_cross_facet/callee_var_spine_x_trait_impl.ori"),
        "callee_var_spine_x_trait_impl",
        "11 z",
    );
}
