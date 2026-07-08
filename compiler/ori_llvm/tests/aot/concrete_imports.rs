//! Multi-module imported CONCRETE (non-generic) function call cells.
//!
//! An imported concrete function must resolve to a direct cross-module call
//! at its local/aliased call-site name; falling through to the runtime
//! method-dispatch trap ("no method 'f' on type T") is the pinned defect
//! class. Generic imports resolve via mono-dispatch and are covered by
//! `poly_lambda_mono` / `mono_cross_facet`.

use crate::util::assert_multifile_cell_output;

const MATHLIB: &str = include_str!("fixtures/concrete_imports/mathlib.ori");

#[test]
fn plain_int_import_calls_direct() {
    assert_multifile_cell_output(
        &[
            (
                "host.ori",
                include_str!("fixtures/concrete_imports/host_plain.ori"),
            ),
            ("mathlib.ori", MATHLIB),
        ],
        "plain_int_import",
        "42",
    );
}

#[test]
fn aliased_import_resolves_at_alias_name() {
    assert_multifile_cell_output(
        &[
            (
                "host.ori",
                include_str!("fixtures/concrete_imports/host_alias.ori"),
            ),
            ("mathlib.ori", MATHLIB),
        ],
        "aliased_import",
        "42",
    );
}

#[test]
fn void_returning_import_calls_direct() {
    assert_multifile_cell_output(
        &[
            (
                "host.ori",
                include_str!("fixtures/concrete_imports/host_void.ori"),
            ),
            ("mathlib.ori", MATHLIB),
        ],
        "void_returning_import",
        "hello-from-import",
    );
}

#[test]
#[ignore = "BUG-04-264: transitive module imports broken in the interpreter (imported module's own imports absent from its eval env); the AOT half of this shape is the concrete-import resolution defect this file pins"]
fn chained_imports_resolve_transitively() {
    assert_multifile_cell_output(
        &[
            (
                "host.ori",
                include_str!("fixtures/concrete_imports/host_chain.ori"),
            ),
            (
                "midlib.ori",
                include_str!("fixtures/concrete_imports/midlib.ori"),
            ),
            ("mathlib.ori", MATHLIB),
        ],
        "chained_imports",
        "20",
    );
}

#[test]
fn module_alias_qualified_import_calls_direct() {
    assert_multifile_cell_output(
        &[
            (
                "host.ori",
                include_str!("fixtures/concrete_imports/host_module_alias.ori"),
            ),
            ("mathlib.ori", MATHLIB),
        ],
        "module_alias_qualified_import",
        "42",
    );
}

// An imported concrete fn called from INSIDE a closure body resolves through
// the lambda's own emission context.
#[test]
fn import_called_inside_closure_body() {
    assert_multifile_cell_output(
        &[
            (
                "host.ori",
                include_str!("fixtures/concrete_imports/host_closure_body.ori"),
            ),
            ("mathlib.ori", MATHLIB),
        ],
        "import_called_inside_closure_body",
        "21",
    );
}

// sret-return isolation: int param (Direct) + Error return (Sret) — separates
// the return-ABI dimension from cell 6's combined indirect-param + sret shape.
#[test]
fn sret_return_import_isolated() {
    assert_multifile_cell_output(
        &[
            (
                "host.ori",
                include_str!("fixtures/concrete_imports/host_sret_only.ori"),
            ),
            (
                "errfact.ori",
                include_str!("fixtures/concrete_imports/errfact.ori"),
            ),
        ],
        "sret_return_import_isolated",
        "code-7",
    );
}

// TWO aliases of ONE imported fn each need their own registration - a
// single-slot local name collapses to the last alias.
#[test]
fn multi_alias_import_resolves_both_names() {
    assert_multifile_cell_output(
        &[
            (
                "host.ori",
                include_str!("fixtures/concrete_imports/host_multi_alias.ori"),
            ),
            ("mathlib.ori", MATHLIB),
        ],
        "multi_alias_import_resolves_both_names",
        "42\n84",
    );
}
