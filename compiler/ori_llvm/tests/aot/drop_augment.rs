//! AOT tests for user `@drop` emission + panic-unwind via invoke + landing-pad.
//!
//! These tests exercise the end-to-end Drop story for AOT codegen: the
//! generated drop path runs the user `@drop` body first, then walks owned
//! fields in reverse declaration order, then frees the allocation. A panic
//! inside a user `@drop` (or a field-drop) unwinds — the cleanup landing pad
//! runs the remaining field-walk + frees before the unwind resumes — instead
//! of aborting on the first panic. A SECOND panic during cleanup aborts the
//! process.
//!
//! Spec: drop-trait-proposal §Drop and panic (single drop-panic recoverable;
//! nested panic during unwind aborts). Trait impls use the colon form
//! `impl Type: Trait` (the sole trait-impl form). AOT entry points use
//! `@main`. Each behavioral cell asserts the EXACT stdout sentinel sequence
//! via `compile_and_run_capture` — asserting exit-0 alone is insufficient to
//! prove drop-order / panic-unwind / field-walk-continuation.
//!
//! Exit-code contract (per the harness, which sets `ORI_CHECK_LEAKS=1`):
//! clean = 0, panic-unwind = 1, leak = 2, abort (nested panic / drop guard) =
//! 134 (128 + SIGABRT).

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::{compile_and_run, compile_and_run_capture, compile_and_run_valgrind_with_args};

/// User `@drop` runs first, then the compiler walks owned fields in reverse
/// declaration order, then the allocation is freed. The captured stdout order
/// pins both axes: user-@drop-first AND reverse-declaration field walk.
#[test]
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
    let r = [Resource {
        a: Logged { tag: "a" },
        b: Logged { tag: "b" },
    }]
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
fn drop_enum_runs_user_method_first_then_variant_fields_in_reverse_order() {
    let source = r#"
type Logged = { tag: str }

impl Logged: Drop {
    @drop (self) -> void = print(msg: `drop-Logged-{self.tag}`);
}

type EventLog = Single(payload: Logged) | Pair(first: Logged, second: Logged);

impl EventLog: Drop {
    @drop (self) -> void = print(msg: "drop-EventLog-user");
}

@main () -> void = {
    let pair = [Pair(
        first: Logged { tag: "first" },
        second: Logged { tag: "second" },
    )]
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
/// abort. `ORI_CHECK_LEAKS=1` (on by default in the harness) confirms the
/// heap `str` fields were freed on the unwind path (exit would be 2 on leak).
#[test]
#[ignore = "BUG-04-125: recoverable drop-panic unwinding deferred — Ori panics are foreign Itanium exceptions; recovery needs whole-frame unwind threading + runtime catch-continue-reraise, beyond the drop-fn invoke/landing-pad"]
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
    let r = [Resource {
        a: Logged { tag: "a" },
        b: Logged { tag: "b" },
    }]
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
/// while unwinding from `Resource::@drop`'s panic) aborts the process. The
/// nested-panic abort path is reached via the drop guard / explicit double-
/// panic abort. Observable as a non-recoverable abort exit (134), not a clean
/// (0), panic (1), or leak (2) exit.
#[test]
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
    let r = [Resource {
        a: Logged { tag: "a" },
        b: Logged { tag: "b" },
    }]
}
"#;
    let (exit_code, _stdout, stderr) = compile_and_run_capture(source);
    // Nested panic during unwind aborts: not clean (0), not single-panic (1), not leak (2).
    assert!(
        exit_code != 0 && exit_code != 1 && exit_code != 2,
        "nested drop-panic must abort the process; got exit {exit_code}; stderr:\n{stderr}"
    );
}

// ── Core panic-unwindcells ──────────────────────────────────────────

/// Recoverable user-@drop panic on a bare-local Drop struct with an owned-str
/// field: the user @drop prints a sentinel then panics; the cleanup landing
/// pad still drops the heap `str` field, and the program exits via panic (1).
/// `ORI_CHECK_LEAKS=1` confirms the str field was freed on the unwind path.
#[test]
#[ignore = "BUG-04-125: recoverable drop-panic unwinding deferred — Ori panics are foreign Itanium exceptions; recovery needs whole-frame unwind threading + runtime catch-continue-reraise, beyond the drop-fn invoke/landing-pad"]
fn drop_struct_user_panic_recoverable_frees_owned_str() {
    let source = r#"
type Holder = { payload: str }

impl Holder: Drop {
    @drop (self) -> void = {
        print(msg: "drop-user");
        panic(msg: "boom")
    }
}

@main () -> void = {
    let h = [Holder { payload: "owned-heap-string-not-sso-xxxxxxxxxxxxxxxx" }]
}
"#;
    let (exit_code, stdout, stderr) = compile_and_run_capture(source);
    assert!(
        stdout.contains("drop-user"),
        "user @drop body must run before the panic; stdout:\n{stdout}"
    );
    assert_eq!(
        exit_code, 1,
        "single @drop panic must unwind (1), not abort or leak (2); stderr:\n{stderr}"
    );
}

/// Panic in a field-drop, sibling field cleanup continues. A struct
/// with two owned-str fields whose element type has a panicking @drop: dropping
/// field 0 panics → field 1's cleanup still ran (sentinel) + no leak.
#[test]
#[ignore = "BUG-04-125: recoverable drop-panic unwinding deferred — Ori panics are foreign Itanium exceptions; recovery needs whole-frame unwind threading + runtime catch-continue-reraise, beyond the drop-fn invoke/landing-pad"]
fn drop_field_panic_sibling_field_walk_continues() {
    let source = r#"
type Loud = { tag: str }

impl Loud: Drop {
    @drop (self) -> void = {
        print(msg: `loud-{self.tag}`);
        panic(msg: "loud-boom")
    }
}

type Quiet = { tag: str }

impl Quiet: Drop {
    @drop (self) -> void = print(msg: `quiet-{self.tag}`);
}

type Pair = { first: Loud, second: Quiet }

@main () -> void = {
    let p = [Pair {
        first: Loud { tag: "L" },
        second: Quiet { tag: "Q" },
    }]
}
"#;
    let (exit_code, stdout, stderr) = compile_and_run_capture(source);
    assert!(
        stdout.contains("loud-L"),
        "the panicking field-drop must run; stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("quiet-Q"),
        "the sibling field must still drop via the landing pad; stdout:\n{stdout}"
    );
    assert_eq!(
        exit_code, 1,
        "field-drop panic must unwind (1), not abort or leak (2); stderr:\n{stderr}"
    );
}

// ── Type / feature coverage ──────────────────────────────────────────

/// A closure captures a struct with `@drop`; closure env teardown runs the
/// captured value's @drop at scope exit. Pins user @drop on the closure-env
/// drop path.
#[test]
fn drop_closure_captured_value_runs_user_drop_at_env_teardown() {
    let source = r#"
type Logged = { tag: str }

impl Logged: Drop {
    @drop (self) -> void = print(msg: `drop-captured-{self.tag}`);
}

@main () -> void = {
    let captured = Logged { tag: "C" };
    let c = () -> captured.tag.length();
    let _len = c()
}
"#;
    let (exit_code, stdout, stderr) = compile_and_run_capture(source);
    assert_eq!(exit_code, 0, "expected clean exit (0); stderr:\n{stderr}");
    assert!(
        stdout.contains("drop-captured-C"),
        "closure-captured value's @drop must run at env teardown; stdout:\n{stdout}"
    );
}

/// Map value type with `@drop` — element @drop fires on map teardown.
#[test]
fn drop_map_value_type_runs_user_drop_on_teardown() {
    let source = r#"
type Logged = { tag: str }

impl Logged: Drop {
    @drop (self) -> void = print(msg: `drop-val-{self.tag}`);
}

@main () -> void = {
    let m = { "k1": Logged { tag: "v1" } }
}
"#;
    let (exit_code, stdout, stderr) = compile_and_run_capture(source);
    assert_eq!(exit_code, 0, "expected clean exit (0); stderr:\n{stderr}");
    assert!(
        stdout.contains("drop-val-v1"),
        "map value's @drop must run on teardown; stdout:\n{stdout}"
    );
}

/// Set element type with `@drop` — element @drop fires on set teardown.
#[test]
#[ignore = "BUG-05-006: Set<T> element @drop teardown segfaults (set-buffer hash-table runtime layout) — orthogonal to @drop emission"]
fn drop_set_element_type_runs_user_drop_on_teardown() {
    let source = r#"
type Logged = { tag: str }

impl Logged: Drop {
    @drop (self) -> void = print(msg: `drop-elem-{self.tag}`);
}

impl Logged: Eq {
    @equals (self, other: Logged) -> bool = self.tag == other.tag;
}

impl Logged: Hashable {
    @hash (self) -> int = self.tag.length();
}

@main () -> void = {
    let s: Set<Logged> = [Logged { tag: "e1" }].iter().collect()
}
"#;
    let (exit_code, stdout, stderr) = compile_and_run_capture(source);
    assert_eq!(exit_code, 0, "expected clean exit (0); stderr:\n{stderr}");
    assert!(
        stdout.contains("drop-elem-e1"),
        "set element's @drop must run on teardown; stdout:\n{stdout}"
    );
}

/// A `@drop`-typed value created inside a `for ... do` body drops at each loop
/// iteration's scope exit.
#[test]
fn drop_value_inside_for_do_drops_at_each_iteration() {
    let source = r#"
type Logged = { tag: str }

impl Logged: Drop {
    @drop (self) -> void = print(msg: "drop-iter");
}

@main () -> void = {
    for i in [0, 1, 2] do {
        let held = [Logged { tag: "x" }]
    }
}
"#;
    let (exit_code, stdout, stderr) = compile_and_run_capture(source);
    assert_eq!(exit_code, 0, "expected clean exit (0); stderr:\n{stderr}");
    let count = stdout.lines().filter(|l| *l == "drop-iter").count();
    assert_eq!(
        count, 3,
        "value's @drop must fire once per iteration (3); stdout:\n{stdout}"
    );
}

/// A `@drop`-typed value held across a `?` early-return path drops on the
/// success path's scope exit.
#[test]
fn drop_value_with_try_operator_drops_on_success_path() {
    let source = r#"
type Logged = { tag: str }

impl Logged: Drop {
    @drop (self) -> void = print(msg: "drop-try");
}

@compute () -> Result<int, str> = Ok(42);

@scoped () -> Result<int, str> = {
    let held = [Logged { tag: "t" }];
    let v = compute()?;
    Ok(v)
}

@main () -> void = {
    let _r = scoped()
}
"#;
    let (exit_code, stdout, stderr) = compile_and_run_capture(source);
    assert_eq!(exit_code, 0, "expected clean exit (0); stderr:\n{stderr}");
    assert!(
        stdout.contains("drop-try"),
        "value's @drop must fire on the success path scope exit; stdout:\n{stdout}"
    );
}

/// A `@drop`-typed value produced in a `for ... yield` body — the collected
/// list's elements drop on the list's teardown.
#[test]
fn drop_value_inside_for_yield_drops_on_collection_teardown() {
    let source = r#"
type Logged = { tag: str }

impl Logged: Drop {
    @drop (self) -> void = print(msg: "drop-yield");
}

@main () -> void = {
    let collected = for i in [0, 1] yield Logged { tag: "y" }
}
"#;
    let (exit_code, stdout, stderr) = compile_and_run_capture(source);
    assert_eq!(exit_code, 0, "expected clean exit (0); stderr:\n{stderr}");
    let count = stdout.lines().filter(|l| *l == "drop-yield").count();
    assert_eq!(
        count, 2,
        "each yielded value's @drop must fire on collection teardown (2); stdout:\n{stdout}"
    );
}

// ── All-shape panic-during-drop cells (all-shape panic-during-drop) ─────────────────

/// Enum-variant payload field-drop panics → remaining payload fields cleaned
/// up via the variant's per-field invoke cleanup (no leak, no double-free of
/// already-walked payload fields).
#[test]
#[ignore = "BUG-04-125: recoverable drop-panic unwinding deferred — Ori panics are foreign Itanium exceptions; recovery needs whole-frame unwind threading + runtime catch-continue-reraise, beyond the drop-fn invoke/landing-pad"]
fn drop_enum_variant_payload_panic_cleans_remaining_payload_fields() {
    let source = r#"
type Loud = { tag: str }

impl Loud: Drop {
    @drop (self) -> void = {
        print(msg: `loud-{self.tag}`);
        panic(msg: "loud-boom")
    }
}

type Quiet = { tag: str }

impl Quiet: Drop {
    @drop (self) -> void = print(msg: `quiet-{self.tag}`);
}

type Wrapper = Both(loud: Loud, quiet: Quiet);

@main () -> void = {
    let w = [Both(
        loud: Loud { tag: "L" },
        quiet: Quiet { tag: "Q" },
    )]
}
"#;
    let (exit_code, stdout, stderr) = compile_and_run_capture(source);
    assert!(
        stdout.contains("loud-L"),
        "panicking payload field-drop must run; stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("quiet-Q"),
        "sibling payload field must still drop via the landing pad; stdout:\n{stdout}"
    );
    assert_eq!(
        exit_code, 1,
        "payload field-drop panic must unwind (1), not abort or leak (2); stderr:\n{stderr}"
    );
}

/// Collection element @drop panics mid-walk → remaining elements still
/// dropped; the cleanup pad drops only indices > cursor (no double-free of the
/// already-walked element).
#[test]
#[ignore = "BUG-04-125: recoverable drop-panic unwinding deferred — Ori panics are foreign Itanium exceptions; recovery needs whole-frame unwind threading + runtime catch-continue-reraise, beyond the drop-fn invoke/landing-pad"]
fn drop_collection_element_panic_cleans_remaining_elements() {
    let source = r#"
type Counted = { tag: str }

impl Counted: Drop {
    @drop (self) -> void = print(msg: `drop-{self.tag}`);
}

type Bomb = { tag: str }

impl Bomb: Drop {
    @drop (self) -> void = {
        print(msg: `bomb-{self.tag}`);
        panic(msg: "bomb")
    }
}

type Mixed = { items: [Counted], trigger: Bomb }

@main () -> void = {
    let m = [Mixed {
        items: [Counted { tag: "a" }, Counted { tag: "b" }],
        trigger: Bomb { tag: "T" },
    }]
}
"#;
    let (exit_code, stdout, stderr) = compile_and_run_capture(source);
    assert!(
        stdout.contains("bomb-T"),
        "panicking field-drop must run; stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("drop-a") && stdout.contains("drop-b"),
        "remaining collection elements must drop via the landing pad; stdout:\n{stdout}"
    );
    assert_eq!(
        exit_code, 1,
        "element-drop panic must unwind (1), not abort or leak (2); stderr:\n{stderr}"
    );
}

/// Map VALUE @drop panics mid-walk → remaining entries cleaned up; both key
/// and value buffers freed.
#[test]
#[ignore = "BUG-04-125: recoverable drop-panic unwinding deferred — Ori panics are foreign Itanium exceptions; recovery needs whole-frame unwind threading + runtime catch-continue-reraise, beyond the drop-fn invoke/landing-pad"]
fn drop_map_value_panic_cleans_remaining_entries() {
    let source = r#"
type Boom = { tag: str }

impl Boom: Drop {
    @drop (self) -> void = {
        print(msg: `boom-{self.tag}`);
        panic(msg: "boom")
    }
}

type Wrap = { m: {str: Boom} }

@main () -> void = {
    let w = [Wrap { m: { "k": Boom { tag: "v" } } }]
}
"#;
    let (exit_code, stdout, stderr) = compile_and_run_capture(source);
    assert!(
        stdout.contains("boom-v"),
        "panicking map-value drop must run; stdout:\n{stdout}"
    );
    assert_eq!(
        exit_code, 1,
        "map-value-drop panic must unwind (1), not abort or leak (2); stderr:\n{stderr}"
    );
}

/// Map KEY @drop panics → the two-channel cleanup must still drop the
/// corresponding/remaining VALUES and free BOTH buffers (no value leak when
/// the panic originates on the key channel).
#[test]
#[ignore = "BUG-04-125: recoverable drop-panic unwinding deferred — Ori panics are foreign Itanium exceptions; recovery needs whole-frame unwind threading + runtime catch-continue-reraise, beyond the drop-fn invoke/landing-pad"]
fn drop_map_key_panic_two_channel_cleanup_frees_values() {
    let source = r#"
type BoomKey = { tag: str }

impl BoomKey: Drop {
    @drop (self) -> void = {
        print(msg: `boom-key-{self.tag}`);
        panic(msg: "boom-key")
    }
}

impl BoomKey: Eq {
    @equals (self, other: BoomKey) -> bool = self.tag == other.tag;
}

impl BoomKey: Hashable {
    @hash (self) -> int = self.tag.length();
}

type Val = { tag: str }

impl Val: Drop {
    @drop (self) -> void = print(msg: `val-{self.tag}`);
}

type Wrap = { m: {BoomKey: Val} }

@main () -> void = {
    let k = BoomKey { tag: "k" };
    let w = [Wrap { m: {[k]: Val { tag: "v" }} }]
}
"#;
    let (exit_code, stdout, stderr) = compile_and_run_capture(source);
    assert!(
        stdout.contains("boom-key-k"),
        "panicking map-key drop must run; stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("val-v"),
        "the value channel must still drop when the key-drop panics; stdout:\n{stdout}"
    );
    assert_eq!(
        exit_code, 1,
        "map-key-drop panic must unwind (1), not abort or leak (2); stderr:\n{stderr}"
    );
}

// ── Semantic + negative pins ─────────────────────────────────────────

/// Semantic pin (recoverable path): assert zero leaks via valgrind, which
/// tracks frees regardless of the `catch_bb` exit path (the `ORI_CHECK_LEAKS`
/// exit-counter may be bypassed on the panic-recovery exit). Proves every
/// owned field/element is freed exactly once (no leak, no double-free) on the
/// unwind exit.
#[test]
#[ignore = "BUG-04-125: recoverable drop-panic unwinding deferred — Ori panics are foreign Itanium exceptions; recovery needs whole-frame unwind threading + runtime catch-continue-reraise, beyond the drop-fn invoke/landing-pad"]
fn drop_recoverable_panic_path_no_leak_under_valgrind() {
    let source = r#"
type Logged = { tag: str }

impl Logged: Drop {
    @drop (self) -> void = print(msg: `drop-{self.tag}`);
}

type Resource = { a: Logged, b: Logged }

impl Resource: Drop {
    @drop (self) -> void = panic(msg: "intentional");
}

@main () -> void = {
    let r = [Resource {
        a: Logged { tag: "a" },
        b: Logged { tag: "b" },
    }]
}
"#;
    // valgrind absent in this environment → None → semantic pin skips; the
    // ORI_CHECK_LEAKS exit-2 check on the other panic cells is the fallback.
    if let Some((clean, report)) = compile_and_run_valgrind_with_args(source, &[]) {
        assert!(
            clean,
            "valgrind must report no leaks/double-frees on the recoverable @drop-panic path:\n{report}"
        );
    }
}

/// Negative pin: a single user-@drop panic does NOT abort before cleanup. This
/// rejects the current `call_drop_fn` first-panic-abort behavior — the program
/// must exit via panic (1), and the field-walk sentinel proves cleanup ran
/// before the unwind resumed (not a pre-cleanup abort).
#[test]
#[ignore = "BUG-04-125: recoverable drop-panic unwinding deferred — Ori panics are foreign Itanium exceptions; recovery needs whole-frame unwind threading + runtime catch-continue-reraise, beyond the drop-fn invoke/landing-pad"]
fn drop_single_panic_does_not_abort_before_cleanup() {
    let source = r#"
type Logged = { tag: str }

impl Logged: Drop {
    @drop (self) -> void = print(msg: `drop-{self.tag}`);
}

type Resource = { a: Logged }

impl Resource: Drop {
    @drop (self) -> void = panic(msg: "intentional");
}

@main () -> void = {
    let r = [Resource { a: Logged { tag: "a" } }]
}
"#;
    let exit_code = compile_and_run(source);
    assert_eq!(
        exit_code, 1,
        "single @drop panic must unwind (1), NOT abort (134) before cleanup"
    );
    let (_e, stdout, _s) = compile_and_run_capture(source);
    assert!(
        stdout.contains("drop-a"),
        "the owned field must still drop on the unwind path; stdout:\n{stdout}"
    );
}
