//! Tag-blind `Option<RC-inner>` RC value-walk — behavioral matrix.
//!
//! Corroborates the deterministic IR-shape pins (`tests/codegen/rc/
//! option_*_field_*_tag_guarded.ori`) with end-to-end runs of the
//! `Option<RC-inner>` dec-walk matrix under the verification env
//! (`ORI_VERIFY_ARC=1` `ORI_VERIFY_EACH=1`), run under `ORI_CHECK_LEAKS=1`
//! (leak -> exit 2). The verdict is carried by ORI_VERIFY_ARC/ORI_VERIFY_EACH
//! plus the leak check.
//!
//! Matrix: inner in {`str`, `[int]`, struct-with-RC field, closure} x variant
//! in {None, Some} x dec walk. Each `None` cell pins that the converged
//! tag-aware walk emits zero payload RC for a `None`; each `Some` cell is the
//! negative pin that the inner RC payload is still freed (a fix that skips the
//! inner walk to "fix" None would leak Some).
//!
//! The inc-walk arm is pinned deterministically by the IR-shape `FileCheck` pins
//! (`option_str_field_none_rc_inc_tag_guarded.ori`); the behavioral inc surface
//! requires struct sharing, which trips an independent pre-existing leak
//! distinct from the tag-blind Option walk, so inc behavioral cells are
//! omitted to avoid false attribution.

use crate::util::compile_and_run_with_build_env;

/// Verification env — the current-adapter RC/AOT verdict surface. The
/// verification vars carry the verdict. It does not establish shared-calculus
/// or cross-executor parity.
const VERIFY_ENV: &[(&str, &str)] = &[("ORI_VERIFY_ARC", "1"), ("ORI_VERIFY_EACH", "1")];

/// One matrix cell: a name for failure attribution + its Ori source.
struct Cell {
    name: &'static str,
    source: &'static str,
}

/// `inner` x `variant` dec-walk matrix. Every dec cell prints `c`.
const MATRIX: &[Cell] = &[
    Cell {
        name: "str_none_dec",
        source: include_str!("fixtures/option_inner_rc_walk/str_none_dec.ori"),
    },
    Cell {
        name: "str_some_dec",
        source: include_str!("fixtures/option_inner_rc_walk/str_some_dec.ori"),
    },
    Cell {
        name: "list_none_dec",
        source: include_str!("fixtures/option_inner_rc_walk/list_none_dec.ori"),
    },
    Cell {
        name: "list_some_dec",
        source: include_str!("fixtures/option_inner_rc_walk/list_some_dec.ori"),
    },
    Cell {
        name: "struct_none_dec",
        source: include_str!("fixtures/option_inner_rc_walk/struct_none_dec.ori"),
    },
    Cell {
        name: "struct_some_dec",
        source: include_str!("fixtures/option_inner_rc_walk/struct_some_dec.ori"),
    },
    Cell {
        name: "closure_none_dec",
        source: include_str!("fixtures/option_inner_rc_walk/closure_none_dec.ori"),
    },
    Cell {
        name: "closure_some_dec",
        source: include_str!("fixtures/option_inner_rc_walk/closure_some_dec.ori"),
    },
];

/// Build + run one cell under the verification env; map the outcome to a
/// human-readable failure or `Ok`.
fn run_cell(cell: &Cell) -> Result<(), String> {
    let (exit, stdout, stderr) = compile_and_run_with_build_env(cell.source, VERIFY_ENV);
    match exit {
        0 if stdout.trim() == "c" => Ok(()),
        0 => Err(format!("{}: wrong stdout {stdout:?}", cell.name)),
        2 => Err(format!(
            "{}: leak under the verification env\n{stderr}",
            cell.name
        )),
        -1 => Err(format!("{}: compile failed\n{stderr}", cell.name)),
        c => Err(format!("{}: exit {c}\n{stderr}", cell.name)),
    }
}

/// Every `Option<RC-inner>` dec-walk cell runs leak-free with the right value
/// under the verification env. Self-verifying: the count assertion proves no
/// cell was silently skipped (4 inner x 2 variants x dec walk = 8).
#[test]
fn option_inner_field_dec_walk_tag_guarded_and_leak_free() {
    let mut seen = 0usize;
    let mut failures: Vec<String> = Vec::new();
    for cell in MATRIX {
        if let Err(e) = run_cell(cell) {
            failures.push(e);
        }
        seen += 1;
    }
    assert_eq!(seen, 8, "matrix must visit every inner x variant cell");
    assert_eq!(MATRIX.len(), 8, "matrix array must hold every cell");
    assert!(
        failures.is_empty(),
        "Option<RC-inner> dec-walk failures:\n{}",
        failures.join("\n")
    );
}

/// Negative pin: a `Some(str)`-bearing Option field dec MUST still free its
/// inner str. A fix that skips the inner walk to "fix" the None path would
/// under-emit and leak Some — caught here as a leak (exit 2).
#[test]
fn option_str_field_some_dec_frees_inner_str() {
    let (exit, stdout, stderr) = compile_and_run_with_build_env(
        include_str!("fixtures/option_inner_rc_walk/str_some_dec.ori"),
        VERIFY_ENV,
    );
    assert_eq!(
        exit, 0,
        "Some(str) Option field dec must run leak-free under the verification env\nstdout:{stdout}\nstderr:{stderr}"
    );
    assert_eq!(stdout.trim(), "c", "Some(str) dec cell wrong stdout");
}

/// Collection-reach cell: a `[Option<str>]` list with mixed `None`/`Some(str)`
/// elements, dropped — exercises the tag-aware buffer elem-dec path the converge
/// fix produces (`ori_buffer_store_elem_dec` -> `_ori_elem_dec$N`, emitted via
/// `emit_inline_enum_dec`). None elements emit zero payload RC; Some elements
/// free their inner str exactly once. Confirms the converge fix's collection
/// reach is leak-free.
#[test]
fn list_option_str_element_dec_tag_guarded_and_leak_free() {
    let (exit, stdout, stderr) = compile_and_run_with_build_env(
        include_str!("fixtures/option_inner_rc_walk/list_option_str_elem_dec.ori"),
        VERIFY_ENV,
    );
    assert_eq!(
        exit, 0,
        "[Option<str>] element dec must run leak-free under the verification env\nstdout:{stdout}\nstderr:{stderr}"
    );
    assert_eq!(stdout.trim(), "c", "list element dec cell wrong stdout");
}

/// Inc-direction cell: cloning a `Holder { f: Some(str) }` incs the inner str
/// through the tag-aware inc walk (`emit_inline_enum_inc`), closing the
/// inc-direction behavioral gap WITHOUT the `[h, h]` list-sharing that trips the
/// independent struct-in-list inc leak. The clone + both Holders drop balanced —
/// leak-free under the verification env.
#[test]
fn option_str_field_inc_via_clone_leak_free() {
    let (exit, stdout, stderr) = compile_and_run_with_build_env(
        include_str!("fixtures/option_inner_rc_walk/option_str_field_inc_via_clone.ori"),
        VERIFY_ENV,
    );
    assert_eq!(
        exit, 0,
        "Option<str> inc-via-clone must run leak-free under the verification env\nstdout:{stdout}\nstderr:{stderr}"
    );
    assert_eq!(stdout.trim(), "c", "inc-via-clone cell wrong stdout");
}

/// Derived-`Clone` None negative pin: cloning a `Holder { f: None }` MUST inc
/// nothing for the empty Option field. The derived-Clone emission surface
/// (`derive_codegen/clone_rc.rs`) is separate from the value-walk; a tag-blind
/// clone reads the `None`'s uninitialized field-1 payload as a live RC pointer
/// and inc-on-garbage. The tag-aware `emit_clone_option_rc_inc` guards on the
/// discriminant; this cell pins it leak-free + crash-free under the verification env.
#[test]
fn option_str_field_none_inc_via_clone_leak_free() {
    let (exit, stdout, stderr) = compile_and_run_with_build_env(
        include_str!("fixtures/option_inner_rc_walk/option_str_field_none_inc_via_clone.ori"),
        VERIFY_ENV,
    );
    assert_eq!(
        exit, 0,
        "Option<str> None-clone must inc nothing + run leak-free under the verification env\nstdout:{stdout}\nstderr:{stderr}"
    );
    assert_eq!(stdout.trim(), "c", "None-clone cell wrong stdout");
}
