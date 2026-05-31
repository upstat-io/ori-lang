//! AOT tests for user `@drop` panic-unwind via invoke + landing-pad lowering.
//!
//! These tests exercise the end-to-end Drop story for AOT codegen: the
//! generated `_ori_drop$<idx>` function runs the user `@drop` body first,
//! then walks owned fields in reverse declaration order, then frees the
//! allocation. A panic inside a user `@drop` (or a field-drop) unwinds —
//! the cleanup landing pad runs the remaining field-walk + frees before
//! the unwind resumes — instead of aborting on the first panic. A SECOND
//! panic during cleanup aborts the process.
//!
//! Spec: drop-trait-proposal §Drop and panic (single drop-panic recoverable;
//! nested panic during unwind aborts via `ori_drop_double_panic_abort`).
//!
//! Trait impls use the grammar-mandated colon form `impl Type: Trait`
//! (grammar.ebnf:312, per the approved impl-colon-syntax-proposal). AOT
//! entry points use `@main`; expression-bodied `@drop` takes a trailing `;`.
//! Each test asserts the EXACT stdout sentinel sequence via
//! `compile_and_run_capture` — referencing a harness fn without calling it
//! (the prior ghost-test shape) asserts nothing.
//!
//! Blocked by (resolve in order): (1) the parser does not yet implement the
//! grammar's colon `trait_impl` production — `impl Type: Trait` is rejected
//! with E1001 (parser bug, parser behind grammar.ebnf:312); (2) BUG-02-034 —
//! user `impl <prelude-trait>: Type` ICEs in type-check registration; and
//! (3) BUG-04-125 — AOT user-`@drop` emission + invoke/landing-pad lowering
//! (the `_ori_user_drop$<idx>` target is undefined; the drop call is a plain
//! `call`, not `invoke`). Un-ignored when (1)+(2)+(3) land.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::compile_and_run_capture;

/// User `@drop` runs first, then the compiler walks owned fields in reverse
/// declaration order, then the allocation is freed. The captured stdout order
/// pins both axes: user-@drop-first AND reverse-declaration field walk.
#[test]
#[ignore = "BUG-04-125: AOT user-@drop emission + invoke/landing-pad lowering unimplemented (blocked behind colon-trait-impl parse + BUG-02-034 registration ICE); un-ignored when the §05 fix lands"]
fn drop_struct_runs_user_method_first_then_fields_in_reverse_decl_order() {
    let source = r#"
type Logged = { tag: str }

impl Logged: Drop {
    @drop (self) -> void = print(msg: `drop-Logged-{self.tag}`);
}

type Resource = { a: Logged, b: Logged }

impl Resource: Drop {
    @drop (self) -> void = print(msg: "drop-Resource-user");
}

@main () -> void = {
    let r = Resource {
        a: Logged { tag: "a" },
        b: Logged { tag: "b" },
    }
}
"#;
    let (exit_code, stdout, stderr) = compile_and_run_capture(source);
    assert_eq!(exit_code, 0, "expected clean exit (0); stderr:\n{stderr}");
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(
        lines,
        vec!["drop-Resource-user", "drop-Logged-b", "drop-Logged-a"],
        "user @drop must run first, then fields in reverse decl order; stdout:\n{stdout}"
    );
}

/// On an enum-shaped Drop type, the user `@drop` runs first, then the
/// discriminant-switch + per-variant field walk fires in reverse order.
#[test]
#[ignore = "BUG-04-125: AOT enum user-@drop emission (emit_user_drop_call_enum) + invoke/landing-pad lowering unimplemented (blocked behind colon-trait-impl parse + BUG-02-034); un-ignored when the §05 fix lands"]
fn drop_enum_runs_user_method_first_then_variant_fields_in_reverse_order() {
    let source = r#"
type Logged = { tag: str }

impl Logged: Drop {
    @drop (self) -> void = print(msg: `drop-Logged-{self.tag}`);
}

type EventLog = Single(payload: Logged) | Pair(first: Logged, second: Logged)

impl EventLog: Drop {
    @drop (self) -> void = print(msg: "drop-EventLog-user");
}

@main () -> void = {
    let pair = EventLog.Pair(
        first: Logged { tag: "first" },
        second: Logged { tag: "second" },
    )
}
"#;
    let (exit_code, stdout, stderr) = compile_and_run_capture(source);
    assert_eq!(exit_code, 0, "expected clean exit (0); stderr:\n{stderr}");
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(
        lines,
        vec![
            "drop-EventLog-user",
            "drop-Logged-second",
            "drop-Logged-first"
        ],
        "user @drop first, then variant payload reverse-decl walk; stdout:\n{stdout}"
    );
}

/// A panic inside a user `@drop` is recoverable: the cleanup landing pad runs
/// the remaining field-walk (both `Logged` fields drop) before the unwind
/// resumes, and the program exits via panic (exit 1) — NOT a pre-cleanup
/// SIGABRT. `ORI_CHECK_LEAKS=1` (on by default in the harness) confirms the
/// heap `str` fields were freed on the unwind path.
#[test]
#[ignore = "BUG-04-125: invoke/landing-pad lowering unimplemented (currently aborts on first @drop panic; blocked behind colon-trait-impl parse + BUG-02-034); un-ignored when the §05 fix lands"]
fn drop_struct_user_panic_still_runs_field_walk_then_resumes_unwind() {
    let source = r#"
type Logged = { tag: str }

impl Logged: Drop {
    @drop (self) -> void = print(msg: `drop-Logged-{self.tag}`);
}

type Resource = { a: Logged, b: Logged }

impl Resource: Drop {
    @drop (self) -> void = {
        print(msg: "drop-Resource-pre-panic");
        panic(msg: "intentional")
    }
}

@main () -> void = {
    let r = Resource {
        a: Logged { tag: "a" },
        b: Logged { tag: "b" },
    }
}
"#;
    let (exit_code, stdout, stderr) = compile_and_run_capture(source);
    // Single drop-panic is recoverable: exit via panic (1), NOT abort, NOT leak (2).
    assert_eq!(
        exit_code, 1,
        "single @drop panic must unwind (exit 1=panic), not abort or leak; stderr:\n{stderr}"
    );
    // Field-walk continued on the unwind path: user @drop ran, then both fields dropped.
    assert!(
        stdout.contains("drop-Resource-pre-panic"),
        "user @drop body must run before the panic; stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("drop-Logged-a") && stdout.contains("drop-Logged-b"),
        "both owned fields must still drop via the landing pad; stdout:\n{stdout}"
    );
}

/// A SECOND panic during the cleanup field-walk (here: `Logged::@drop` panics
/// while unwinding from `Resource::@drop`'s panic) aborts the process via
/// `ori_drop_double_panic_abort`. Observable as a non-recoverable exit
/// (neither clean 0, panic 1, nor leak 2).
#[test]
#[ignore = "BUG-04-125: nested-panic abort via ori_drop_double_panic_abort needs the landing-pad emission (blocked behind colon-trait-impl parse + BUG-02-034); un-ignored when the §05 fix lands"]
fn drop_nested_panic_during_cleanup_aborts_process() {
    let source = r#"
type Logged = { tag: str }

impl Logged: Drop {
    @drop (self) -> void = panic(msg: `panic-in-Logged-{self.tag}`);
}

type Resource = { a: Logged, b: Logged }

impl Resource: Drop {
    @drop (self) -> void = panic(msg: "panic-in-Resource");
}

@main () -> void = {
    let r = Resource {
        a: Logged { tag: "a" },
        b: Logged { tag: "b" },
    }
}
"#;
    let (exit_code, _stdout, stderr) = compile_and_run_capture(source);
    // Nested panic during unwind aborts: not clean (0), not single-panic (1), not leak (2).
    assert!(
        exit_code != 0 && exit_code != 1 && exit_code != 2,
        "nested drop-panic must abort the process; got exit {exit_code}; stderr:\n{stderr}"
    );
}
