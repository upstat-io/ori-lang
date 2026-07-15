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

use crate::util::{
    compile_and_run, compile_and_run_capture, compile_and_run_valgrind_with_args,
    compile_and_run_with_build_env,
};

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

// A `@drop`-typed value created inside a `for ... do` body drops at each loop
// iteration's scope exit.
// BUG-05-006 matrix — `.iter().collect()` result + element ownership.
// Gated burden probe (`ORI_DISABLE_PREDICATE_STACK_RC=1`) is the
// RC/AOT verdict surface; the leak reproduces on both default + gated paths.
// These pins FAIL on the unfixed code (collect result mis-classified non-fresh
// -> dead-value cleanup leak; collect copies elements -> source re-drops).

const BUG_05_006_GATED: &[(&str, &str)] = &[
    ("ORI_DISABLE_PREDICATE_STACK_RC", "1"),
    ("ORI_VERIFY_ARC", "1"),
];

/// P2 — USED set-collect with an `@drop` element: the element `@drop` must fire
/// EXACTLY ONCE (set teardown), not twice (source-list + set). Pre-fix: `drop`
/// prints twice (collect copies the element; source buffer re-drops its copy).
#[test]
#[ignore = "BUG-05-006: collect copies elements + source re-drops -> @drop fires 2x on AOT (correct: 1x)"]
fn drop_set_collect_element_drops_exactly_once_when_used() {
    let source = r#"
type Logged = { tag: str }
impl Logged: Drop     { @drop (self) -> void = print(msg: `drop-elem-{self.tag}`); }
impl Logged: Eq       { @equals (self, other: Logged) -> bool = self.tag == other.tag; }
impl Logged: Hashable { @hash (self) -> int = self.tag.length(); }

@main () -> void = {
    let s: Set<Logged> = [Logged { tag: "e1" }].iter().collect();
    print(msg: `size={s.len()}`)
}
"#;
    let (exit_code, stdout, stderr) = compile_and_run_with_build_env(source, BUG_05_006_GATED);
    assert_eq!(exit_code, 0, "expected clean exit (0); stderr:\n{stderr}");
    let drops = stdout.lines().filter(|l| *l == "drop-elem-e1").count();
    assert_eq!(
        drops, 1,
        "collected set element @drop must fire exactly once (one logical value), \
         not once-per-buffer; stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("size=1"),
        "set must contain the collected element; stdout:\n{stdout}"
    );
}

/// P3 — UNUSED list-collect leaks its buffer (NOT set-specific). Pre-fix: the
/// `ori_iter_collect` result is mis-classified non-fresh -> no scope-exit dec
/// -> exit 2 (leak). Post-fix: exit 0, `@drop` once.
#[test]
#[ignore = "BUG-05-006: unused .iter().collect() result (list family) not admitted -> buffer leak (exit 2)"]
fn drop_list_collect_unused_result_is_freed() {
    let source = r#"
type Logged = { tag: str }
impl Logged: Drop { @drop (self) -> void = print(msg: `drop-elem-{self.tag}`); }

@main () -> void = {
    let l: [Logged] = [Logged { tag: "e1" }].iter().collect()
}
"#;
    let (exit_code, stdout, stderr) = compile_and_run_with_build_env(source, BUG_05_006_GATED);
    assert_eq!(
        exit_code, 0,
        "unused list-collect result must be freed (no leak, exit 0); \
         stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    let drops = stdout.lines().filter(|l| *l == "drop-elem-e1").count();
    assert_eq!(
        drops, 1,
        "element @drop fires exactly once; stdout:\n{stdout}"
    );
}

/// P5 — heap-element (non-SSO str) USED set-collect: no double-FREE (RC of the
/// shared heap str must balance) AND `@drop` exactly once. Pre-fix: `@drop` 2x
/// (RC balanced, no double-free) — the count is the regression, not memory.
#[test]
#[ignore = "BUG-05-006: heap-element collect runs @drop 2x (correct: 1x); RC balanced so no double-free"]
fn drop_set_collect_heap_element_drops_once_no_double_free() {
    let source = r#"
type Logged = { tag: str }
impl Logged: Drop     { @drop (self) -> void = print(msg: "drop"); }
impl Logged: Eq       { @equals (self, other: Logged) -> bool = self.tag == other.tag; }
impl Logged: Hashable { @hash (self) -> int = self.tag.length(); }

@main () -> void = {
    let s: Set<Logged> = [Logged { tag: "this-string-is-longer-than-twenty-three-bytes-heap" }].iter().collect();
    print(msg: `size={s.len()}`)
}
"#;
    let (exit_code, stdout, stderr) = compile_and_run_with_build_env(source, BUG_05_006_GATED);
    assert_eq!(exit_code, 0, "no leak / no double-free; stderr:\n{stderr}");
    let drops = stdout.lines().filter(|l| *l == "drop").count();
    assert_eq!(
        drops, 1,
        "heap-element @drop fires exactly once; stdout:\n{stdout}"
    );
}

/// P6 — scalar-element collect (no `@drop`, no RC children): positive pin that
/// MUST stay green across the fix (no regression on the scalar path).
#[test]
fn drop_set_collect_scalar_element_no_leak() {
    let source = r#"
@main () -> void = {
    let s: Set<int> = [1, 2, 3].iter().collect();
    print(msg: `n={s.len()}`)
}
"#;
    let (exit_code, stdout, stderr) = compile_and_run_with_build_env(source, BUG_05_006_GATED);
    assert_eq!(
        exit_code, 0,
        "scalar set-collect must not leak; stderr:\n{stderr}"
    );
    assert!(
        stdout.contains("n=3"),
        "set has 3 elements; stdout:\n{stdout}"
    );
}

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

/// Multi-variant (tagged general enum) payload field-drop panics on the
/// FIRST-walked field → the remaining LATER-walked sibling payload field of
/// the SAME variant must still drop via the per-field cleanup pad. The
/// single-variant tagless cell above
/// (`drop_enum_variant_payload_panic_cleans_remaining_payload_fields`) does not
/// cover this: a tagless enum has no discriminant switch, and its panicking
/// field was the last-walked (no later sibling to leak). Here the panicking
/// `Loud` field is declared LAST, so reverse-decl LIFO walks it FIRST; the
/// earlier-declared `Quiet` sibling must still drop on the unwind path (its
/// `quiet-Q` sentinel proves the sibling's heap `str` was freed — without the
/// per-field cleanup pad the sibling silently leaks).
#[test]
fn drop_tagged_enum_variant_payload_panic_cleans_remaining_sibling() {
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

type Wrapper = Solo(only: Loud) | Pair(quiet: Quiet, loud: Loud);

@main () -> void = {
    let w = [Pair(
        quiet: Quiet { tag: "Q" },
        loud: Loud { tag: "L" },
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
        "later-walked sibling payload field must still drop via the per-field cleanup pad; stdout:\n{stdout}"
    );
    assert_eq!(
        exit_code, 1,
        "tagged-enum payload field-drop panic must unwind (1), not abort or leak (2); stderr:\n{stderr}"
    );
}

/// Boxed-recursive (heap drop-fn) enum payload: EVERY node's payload fields'
/// user `@drop` MUST run on teardown, in reverse declaration order, across the
/// recursive chain. The heap drop-fn path (`_ori_drop$<Enum>`) is distinct from
/// the inline value-traversal path covered by
/// `drop_tagged_enum_variant_payload_panic_cleans_remaining_sibling` — a
/// genuinely boxed-recursive `next` field forces every node through
/// `emit_drop_enum`/`emit_drop_enum_variant_fields`. Before the canonical
/// field-walk consolidation, that heap path only dec'd each payload field's RC
/// child (the inner `str`) and NEVER ran the inline struct payload's user
/// `@drop` — so the inner node's `quiet-inner-*` sentinels silently vanished.
/// Each node has TWO Drop-impl payload fields so the per-node multi-field walk
/// is exercised: both fields' `@drop` must run.
#[test]
fn drop_boxed_recursive_enum_runs_every_node_payload_drop_in_reverse_order() {
    let source = r#"
type Logged = { tag: str }

impl Logged: Drop {
    @drop (self) -> void = print(msg: `drop-{self.tag}`);
}

type Chain = Nil | Link(a: Logged, b: Logged, next: Chain);

@main () -> void = {
    let c = Link(
        a: Logged { tag: "outer-a" },
        b: Logged { tag: "outer-b" },
        next: Link(
            a: Logged { tag: "inner-a" },
            b: Logged { tag: "inner-b" },
            next: Nil,
        ),
    )
}
"#;
    let (exit_code, stdout, stderr) = compile_and_run_capture(source);
    assert_eq!(exit_code, 0, "expected clean exit (0); stderr:\n{stderr}");
    // Both payload fields of BOTH nodes (outer + boxed inner) must run their
    // user @drop — the heap drop-fn path previously skipped inline-payload @drop
    // entirely.
    for tag in [
        "drop-outer-a",
        "drop-outer-b",
        "drop-inner-a",
        "drop-inner-b",
    ] {
        assert!(
            stdout.contains(tag),
            "every node payload field @drop must run via the heap drop-fn path; missing {tag}; stdout:\n{stdout}"
        );
    }
    // ORDER pin (the name's actual claim): the recursive `next` field is
    // declared LAST in `Link(a, b, next)`, so reverse-decl-order walks `next`
    // FIRST — the whole boxed inner node (itself reverse-decl: b then a) drops
    // in full before the outer node's own b/a fields.
    let idx = |tag: &str| {
        stdout
            .find(tag)
            .unwrap_or_else(|| panic!("missing {tag} in stdout:\n{stdout}"))
    };
    let (inner_b, inner_a, outer_b, outer_a) = (
        idx("drop-inner-b"),
        idx("drop-inner-a"),
        idx("drop-outer-b"),
        idx("drop-outer-a"),
    );
    assert!(
        inner_b < inner_a && inner_a < outer_b && outer_b < outer_a,
        "reverse-decl order across the recursive chain must be \
         inner-b, inner-a, outer-b, outer-a; stdout:\n{stdout}"
    );
}

/// Recoverable boxed-recursive enum `@drop` panic, reached through
/// `emit_drop_enum`. A `next: EventLog` field forces the
/// genuinely boxed-recursive heap drop-fn path: the OUTER `Entry` node is dec'd
/// through `_ori_drop$<EventLog>` (`emit_drop_enum`), which runs the node's own
/// `@drop` then walks the variant payload — including the boxed `next` child,
/// dec'd via the heap drop fn AGAIN. The inner `Boom` node's `@drop` panics.
///
/// For recovery, TWO cleanup pads must fire: (1) `emit_drop_enum` invoke-wraps
/// the enum's OWN `@drop` so a panicking own-`@drop` still runs the variant walk
/// plus free; (2) the may-unwind FIELD WALK routes the boxed-`next` dec through
///   `invoke ori_rc_dec_unwind` so a panic inside the boxed child's drop tree
///   lands in a cleanup pad that drains remaining siblings + resumes, instead of
///   aborting at the plain `ori_rc_dec` boundary ("Rust cannot catch foreign
///   exceptions, aborting" → exit 134).
///
/// `@drop` matches `self` and panics ONLY on `Boom` — exactly ONE node panics
/// (the boxed inner node), so it is a SINGLE recoverable drop-panic (exit 1),
/// not a nested panic (134). The outer `Entry` payload `str` + the inner `Boom`
/// payload `str` must both be freed on the unwind path (`ORI_CHECK_LEAKS=1`
/// would force exit 2 on leak).
#[test]
fn drop_enum_boxed_recursive_drop_panic_recoverable_frees_payload() {
    let source = r#"
type EventLog = Empty | Entry(payload: str, next: EventLog) | Boom(tag: str);

impl EventLog: Drop {
    @drop (self) -> void = match self {
        Boom(tag:) -> {
            print(msg: "drop-Boom");
            panic(msg: "boom")
        },
        Entry(payload:, next:) -> print(msg: "drop-Entry"),
        Empty -> print(msg: "drop-Empty"),
    }
}

@main () -> void = {
    let log = [Entry(
        payload: "outer-heap-string-not-sso-xxxxxxxxxxxxxxxx",
        next: Boom(tag: "inner-heap-string-not-sso-yyyyyyyyyyyyyyyy"),
    )]
}
"#;
    let (exit_code, stdout, stderr) = compile_and_run_capture(source);
    assert!(
        stdout.contains("drop-Entry"),
        "outer node @drop must run; stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("drop-Boom"),
        "boxed inner node @drop (reached via emit_drop_enum) must run; stdout:\n{stdout}"
    );
    // Single boxed-child drop-panic is recoverable: exit via panic (1), NOT
    // abort (134) at the plain ori_rc_dec boundary, and NOT a leak (2) — both
    // payload strs are freed on the unwind path via the enum-side cleanup pad +
    // the boxed-field invoke-ori_rc_dec_unwind routing.
    assert_eq!(
        exit_code, 1,
        "boxed-recursive enum drop-panic must unwind (1), not abort (134) or leak (2); stderr:\n{stderr}"
    );
}

/// NICHE enum payload field-drop panic → later-walked sibling payload field
/// still dropped via the per-field cleanup pad. A 2-variant niche-encoded enum
/// (`Option<Pair>` with `Pair` carrying a niche-bearing field) routes its data
/// variant teardown through `emit_drop_enum_niche` (heap) / `emit_niche_enum_rc`
/// (inline). Both now route through the canonical
/// `dec_fields_may_unwind` SSOT, so a panicking payload field's `@drop` frees
/// the later-walked sibling instead of leaking it.
///
/// IGNORED: niche-encoded codegen is feature-gated OFF
/// (`NICHE_CODEGEN_READY = false` in `ori_repr/src/canonical/type_repr.rs`), so
/// no user program reaches the niche dec paths today — a behavioral AOT cell
/// cannot exercise them. The niche-dec SSOT routing is verified at the IR level
/// (per-field `invoke @drop → fld.cont/fld.cleanup` + `landingpad` + `resume`,
/// confirmed with the gate temporarily flipped). This cell activates when niche
/// codegen ships under BUG-04-222 (niche/tagless `TagEncoding` codegen-consumer
/// migration), at which point it must pass without modification.
#[test]
#[ignore = "BUG-04-222: niche-encoded codegen gated off (NICHE_CODEGEN_READY=false); F2 niche dec SSOT routing verified at IR level, this behavioral cell activates when niche codegen ships"]
fn drop_niche_enum_payload_panic_cleans_remaining_sibling() {
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

type Pair = { first: Quiet, second: Loud }

@main () -> void = {
    let p: Option<Pair> = Some(Pair {
        first: Quiet { tag: "Q" },
        second: Loud { tag: "L" },
    })
}
"#;
    let (exit_code, stdout, stderr) = compile_and_run_capture(source);
    assert!(
        stdout.contains("loud-L"),
        "panicking niche-payload field-drop must run; stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("quiet-Q"),
        "later-walked niche-payload sibling must still drop via the per-field cleanup pad; stdout:\n{stdout}"
    );
    assert_eq!(
        exit_code, 1,
        "niche-payload field-drop panic must unwind (1), not abort or leak (2); stderr:\n{stderr}"
    );
}

/// Collection element @drop panics mid-walk → remaining elements still
/// dropped; the cleanup pad drops only indices > cursor (no double-free of the
/// already-walked element).
#[test]
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
fn drop_map_key_panic_two_channel_cleanup_frees_values() {
    // Construction uses a type-annotated empty map + `.insert` rather than the
    // `{[k]: v}` computed-key literal form, which is a separate not-yet-shipped
    // surface (computed-map-keys-proposal) that parses the bracketed key as a
    // list literal. The insert path produces a genuine `{BoomKey: Val}` map so
    // the BoomKey key reaches the two-channel drop teardown.
    let source = r#"
type BoomKey = { tag: str }

impl BoomKey: Drop {
    @drop (self) -> void = {
        print(msg: `boom-key-{self.tag}`);
        panic(msg: "boom-key")
    }
}

impl BoomKey: Eq {
    @eq (self, other: BoomKey) -> bool = self.tag == other.tag;
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
    let m: {BoomKey: Val} = {};
    let m2 = m.insert(key: k, value: Val { tag: "v" });
    let w = [Wrap { m: m2 }]
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

// Scalar-repr struct with a user `@drop`.
//
// A struct whose monomorphized repr is provably `Scalar` (all-scalar fields, no
// heap field, no RC header) carrying a user `@drop`. Two pre-fix failure modes:
//   * READ local (`let r = Guard{7}; print(r.id)`): the surviving scope-exit
//     whole-var `RcDec` on a Scalar repr tripped VF-1 `RcOnScalar` -> compile
//     ICE under `ORI_VERIFY_ARC=1`.
//   * DEAD local (`let r = Guard{7}` never read): no last-use anchor + the
//     predicate stack skips scalars -> the `@drop` was silently elided.
// The fix routes a scalar+`@drop` scope-exit op through `RcStrategy::UserDrop`
// (the `@drop` CALL alone, balance-neutral, exempt from VF-1) and emits a
// completeness dec for the never-used case. Spec: Annex E §AIMS RL-DROP
// (`RLDROP_scalar_lifecycle_sound` / `RLDROP_exactly_once_on_glue`).
//
// Gated burden probe (`ORI_DISABLE_PREDICATE_STACK_RC=1` + `ORI_VERIFY_ARC=1`)
// is the RC/AOT verdict surface; these pins are non-ignored (the
// bug is fixed) and revert-detecting (revert -> DEAD loses its drop, READ ICEs).

const SCALAR_DROP_GATED: &[(&str, &str)] = &[
    ("ORI_DISABLE_PREDICATE_STACK_RC", "1"),
    ("ORI_VERIFY_ARC", "1"),
];

/// Semantic pin (filed bug): a DEAD scalar-repr struct local's `@drop` runs at
/// scope exit. Pre-fix: nothing prints (the `@drop` was elided).
#[test]
fn drop_dead_scalar_struct_local_runs_user_drop_at_scope_exit() {
    let source = r#"
type Guard = { id: int }
impl Guard: Drop { @drop (self) -> void = print(msg: `dropped-{self.id}`); }

@main () -> void = {
    let r = Guard { id: 7 };
    print(msg: "body")
}
"#;
    let (exit_code, stdout, stderr) = compile_and_run_with_build_env(source, SCALAR_DROP_GATED);
    assert_eq!(exit_code, 0, "expected clean exit (0); stderr:\n{stderr}");
    assert!(
        stdout.contains("dropped-7"),
        "the dead scalar-struct local's @drop must run at scope exit; stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("body"),
        "the body must execute before the scope-exit drop; stdout:\n{stdout}"
    );
}

/// Semantic pin (ICE case): a READ scalar-repr struct local compiles clean
/// under `ORI_VERIFY_ARC=1` (no `RcDec on scalar` VF-1 ICE) AND runs its
/// `@drop`. Pre-fix: compilation failed with the VF-1 `RcOnScalar` ICE.
#[test]
fn drop_read_scalar_struct_local_compiles_clean_and_runs_user_drop() {
    let source = r#"
type Guard = { id: int }
impl Guard: Drop { @drop (self) -> void = print(msg: `dropped-{self.id}`); }

@main () -> void = {
    let r = Guard { id: 7 };
    print(msg: `read-{r.id}`)
}
"#;
    let (exit_code, stdout, stderr) = compile_and_run_with_build_env(source, SCALAR_DROP_GATED);
    assert_eq!(
        exit_code, 0,
        "scalar-struct read must compile clean (no RcOnScalar ICE) + run; stderr:\n{stderr}"
    );
    assert!(
        stdout.contains("read-7"),
        "the field read must execute; stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("dropped-7"),
        "the read scalar-struct local's @drop must run at scope exit; stdout:\n{stdout}"
    );
}

#[test]
fn drop_target_ignores_preceding_inherent_same_named_method() {
    let source = r#"
type Guard = { id: int }

impl Guard {
    @drop (self) -> void = print(msg: `wrong-inherent-{self.id}`);
}

impl Guard: Drop {
    @drop (self) -> void = print(msg: `right-drop-{self.id}`);
}

@main () -> void = {
    let guard = Guard { id: 7 };
}
"#;
    let (exit_code, stdout, stderr) = compile_and_run_with_build_env(source, SCALAR_DROP_GATED);
    assert_eq!(exit_code, 0, "expected clean exit (0); stderr:\n{stderr}");
    assert_eq!(stdout.trim(), "right-drop-7", "stdout:\n{stdout}");
}

#[test]
fn drop_target_ignores_following_inherent_same_named_method() {
    let source = r#"
type Guard = { id: int }

impl Guard: Drop {
    @drop (self) -> void = print(msg: `right-drop-{self.id}`);
}

impl Guard {
    @drop (self) -> void = print(msg: `wrong-inherent-{self.id}`);
}

@main () -> void = {
    let guard = Guard { id: 7 };
}
"#;
    let (exit_code, stdout, stderr) = compile_and_run_with_build_env(source, SCALAR_DROP_GATED);
    assert_eq!(exit_code, 0, "expected clean exit (0); stderr:\n{stderr}");
    assert_eq!(stdout.trim(), "right-drop-7", "stdout:\n{stdout}");
}

#[test]
fn drop_target_ignores_same_named_other_trait_method() {
    let source = r#"
trait DecoyDrop {
    @drop (self) -> void
}

type Guard = { id: int }

impl Guard: DecoyDrop {
    @drop (self) -> void = print(msg: `wrong-decoy-{self.id}`);
}

impl Guard: Drop {
    @drop (self) -> void = print(msg: `right-drop-{self.id}`);
}

@main () -> void = {
    let guard = Guard { id: 7 };
}
"#;
    let (exit_code, stdout, stderr) = compile_and_run_with_build_env(source, SCALAR_DROP_GATED);
    assert_eq!(exit_code, 0, "expected clean exit (0); stderr:\n{stderr}");
    assert_eq!(stdout.trim(), "right-drop-7", "stdout:\n{stdout}");
}

#[test]
fn same_named_other_trait_method_does_not_create_drop_obligation() {
    let source = r#"
trait DecoyDrop {
    @drop (self) -> void
}

type Guard = { id: int }

impl Guard: DecoyDrop {
    @drop (self) -> void = print(msg: "must-not-run");
}

@main () -> void = {
    let guard = Guard { id: 7 };
}
"#;
    let (exit_code, stdout, stderr) = compile_and_run_with_build_env(source, SCALAR_DROP_GATED);
    assert_eq!(exit_code, 0, "expected clean exit (0); stderr:\n{stderr}");
    assert!(stdout.trim().is_empty(), "stdout:\n{stdout}");
}

#[test]
fn same_named_inherent_method_does_not_create_drop_obligation() {
    let source = r#"
type Guard = { id: int }

impl Guard {
    @drop (self) -> void = print(msg: "must-not-run");
}

@main () -> void = {
    let guard = Guard { id: 7 };
}
"#;
    let (exit_code, stdout, stderr) = compile_and_run_with_build_env(source, SCALAR_DROP_GATED);
    assert_eq!(exit_code, 0, "expected clean exit (0); stderr:\n{stderr}");
    assert!(stdout.trim().is_empty(), "stdout:\n{stdout}");
}

/// Type-dimension clamp: a multi-field scalar-repr struct (still all-scalar, no
/// heap field) drops exactly once. Guards against the repr-`Scalar` gate keying
/// on field count rather than repr.
#[test]
fn drop_dead_multi_field_scalar_struct_runs_user_drop_once() {
    let source = r#"
type Pair = { a: int, b: bool }
impl Pair: Drop { @drop (self) -> void = print(msg: "dropped-pair"); }

@main () -> void = {
    let p = Pair { a: 1, b: true };
    print(msg: "body")
}
"#;
    let (exit_code, stdout, stderr) = compile_and_run_with_build_env(source, SCALAR_DROP_GATED);
    assert_eq!(exit_code, 0, "expected clean exit (0); stderr:\n{stderr}");
    let drops = stdout.lines().filter(|l| *l == "dropped-pair").count();
    assert_eq!(
        drops, 1,
        "the multi-field scalar struct @drop fires exactly once; stdout:\n{stdout}"
    );
}

/// Multiplicity clamp (double-emit guard): two distinct DEAD scalar-struct
/// locals each drop EXACTLY once. A completeness pass that over-emitted would
/// drop one of them twice; one that under-emitted would miss one.
#[test]
fn drop_two_dead_scalar_structs_each_drop_exactly_once() {
    let source = r#"
type Guard = { id: int }
impl Guard: Drop { @drop (self) -> void = print(msg: `dropped-{self.id}`); }

@main () -> void = {
    let a = Guard { id: 1 };
    let b = Guard { id: 2 };
    print(msg: "body")
}
"#;
    let (exit_code, stdout, stderr) = compile_and_run_with_build_env(source, SCALAR_DROP_GATED);
    assert_eq!(exit_code, 0, "expected clean exit (0); stderr:\n{stderr}");
    let d1 = stdout.lines().filter(|l| *l == "dropped-1").count();
    let d2 = stdout.lines().filter(|l| *l == "dropped-2").count();
    assert_eq!(
        d1, 1,
        "first scalar struct drops exactly once; stdout:\n{stdout}"
    );
    assert_eq!(
        d2, 1,
        "second scalar struct drops exactly once; stdout:\n{stdout}"
    );
}

/// Negative pin: a scalar-repr struct with NO `Drop` impl emits NO drop op (no
/// spurious output, no ICE). Clamps that the completeness pass fires ONLY for a
/// type carrying a user `@drop`.
#[test]
fn dead_scalar_struct_without_drop_emits_no_drop_op() {
    let source = r#"
type Plain = { id: int }

@main () -> void = {
    let r = Plain { id: 7 };
    print(msg: "only-body")
}
"#;
    let (exit_code, stdout, stderr) = compile_and_run_with_build_env(source, SCALAR_DROP_GATED);
    assert_eq!(exit_code, 0, "expected clean exit (0); stderr:\n{stderr}");
    assert_eq!(
        stdout.trim(),
        "only-body",
        "a no-Drop scalar struct must emit no drop output; stdout:\n{stdout}"
    );
}

/// Regression clamp: the existing heap-field-struct dead-value drop path is NOT
/// disturbed by the scalar fix (the `@drop` still runs at scope exit for a
/// never-used heap-bearing struct).
#[test]
fn drop_dead_heap_field_struct_still_runs_user_drop() {
    let source = r#"
type Logged = { tag: str }
impl Logged: Drop { @drop (self) -> void = print(msg: `dropped-{self.tag}`); }

@main () -> void = {
    let r = Logged { tag: "x" };
    print(msg: "body")
}
"#;
    let (exit_code, stdout, stderr) = compile_and_run_with_build_env(source, SCALAR_DROP_GATED);
    assert_eq!(exit_code, 0, "expected clean exit (0); stderr:\n{stderr}");
    let drops = stdout.lines().filter(|l| *l == "dropped-x").count();
    assert_eq!(
        drops, 1,
        "the dead heap-field struct @drop must still fire exactly once; stdout:\n{stdout}"
    );
}

/// Matrix cell: a LET-BOUND user-drop struct VALUE borrowed by the map-insert
/// Invoke terminator. The value's release must land AFTER the call — the
/// "inserted" marker must print BEFORE the teardown-time drop panic (a
/// pre-call drop would print boom first and never reach the insert).
#[test]
fn drop_map_value_let_bound_released_after_insert() {
    let source = r#"
type Boom = { tag: str }

impl Boom: Drop {
    @drop (self) -> void = {
        print(msg: `boom-val-{self.tag}`);
        panic(msg: "boom-val")
    }
}

@main () -> void = {
    let v = Boom { tag: "v" };
    let m: {str: Boom} = {};
    let m2 = m.insert(key: "k", value: v);
    print(msg: "inserted");

    let w = [m2]
}
"#;
    let (_exit_code, stdout, _stderr) = compile_and_run_capture(source);
    let inserted = stdout.find("inserted");
    let boom = stdout.find("boom-val-v");
    assert!(
        inserted.is_some() && boom.is_some(),
        "both the post-insert marker and the teardown drop must run; stdout:\n{stdout}"
    );
    assert!(
        inserted < boom,
        "the value's drop must fire at teardown, AFTER the insert consumed it; stdout:\n{stdout}"
    );
}

/// Matrix cell: a LET-BOUND struct key with NO user drop (heap str field only)
/// borrowed by the map-insert Invoke terminator. A pre-call release of the key
/// frees its field payload before the insert elem-incs it — double-free at
/// teardown (SIGABRT). Must run clean and leak-free.
#[test]
fn map_insert_let_bound_struct_key_no_user_drop_clean() {
    let source = r#"
type Key = { tag: str }

impl Key: Eq {
    @eq (self, other: Key) -> bool = self.tag == other.tag;
}

impl Key: Hashable {
    @hash (self) -> int = self.tag.length();
}

@main () -> void = {
    let k = Key { tag: "some heap allocated key tag string long" };
    let m: {Key: int} = {};
    let m2 = m.insert(key: k, value: 7);
    print(msg: `{m2.len()}`)
}
"#;
    let (exit_code, stdout, stderr) = compile_and_run_capture(source);
    assert_eq!(
        exit_code, 0,
        "let-bound struct-key insert must not double-free; stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stdout.contains('1'),
        "the inserted map must report len 1; stdout:\n{stdout}"
    );
}

/// Over-fire guard cell (consensus requirement): a struct key with a LIVE field
/// extract crossing the borrowing call — the extracted str is read AFTER the
/// insert. The Struct-root death-point relocation must not double-free the
/// extracted view's backing. Clean exit + leak-free is the pin either way the
/// scan disposes the root (admit-with-guard or decline).
#[test]
fn map_insert_struct_key_live_field_extract_no_double_free() {
    let source = r#"
type Key = { tag: str }

impl Key: Eq {
    @eq (self, other: Key) -> bool = self.tag == other.tag;
}

impl Key: Hashable {
    @hash (self) -> int = self.tag.length();
}

@main () -> void = {
    let k = Key { tag: "live extract survives the borrowing insert" };
    let t = k.tag;
    let m: {Key: int} = {};
    let m2 = m.insert(key: k, value: 7);
    print(msg: `{m2.len()} {t.length()}`)
}
"#;
    let (exit_code, stdout, stderr) = compile_and_run_capture(source);
    assert_eq!(
        exit_code, 0,
        "live field extract across the borrowing insert must stay valid; stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stdout.contains("1 42"),
        "map len 1 + extracted tag length 42 expected; stdout:\n{stdout}"
    );
}

/// Matrix cell: a LET-BOUND struct VALUE with NO user drop (heap str field
/// only) borrowed by the map-insert Invoke terminator — value-position analog
/// of the no-user-drop key cell. Must run clean.
#[test]
fn map_insert_let_bound_struct_value_no_user_drop_clean() {
    let source = r#"
type Val = { tag: str }

@main () -> void = {
    let v = Val { tag: "some heap allocated value tag string long" };
    let m: {str: Val} = {};
    let m2 = m.insert(key: "k", value: v);
    print(msg: `{m2.len()}`)
}
"#;
    let (exit_code, stdout, stderr) = compile_and_run_capture(source);
    assert_eq!(
        exit_code, 0,
        "let-bound struct-value insert must not double-free or leak; stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stdout.contains('1'),
        "the inserted map must report len 1; stdout:\n{stdout}"
    );
}

/// Matrix cell: BOTH channels let-bound user-drop structs — two admitted
/// lineages borrowed by one insert call. Both drops must fire at teardown
/// (after the insert), key channel first panicking, value channel still
/// dropping via the two-channel cleanup.
#[test]
fn drop_map_both_channels_let_bound_structs() {
    let source = r#"
type BoomKey = { tag: str }

impl BoomKey: Drop {
    @drop (self) -> void = {
        print(msg: `boom-key-{self.tag}`);
        panic(msg: "boom-key")
    }
}

impl BoomKey: Eq {
    @eq (self, other: BoomKey) -> bool = self.tag == other.tag;
}

impl BoomKey: Hashable {
    @hash (self) -> int = self.tag.length();
}

type Val = { tag: str }

impl Val: Drop {
    @drop (self) -> void = print(msg: `val-{self.tag}`);
}

@main () -> void = {
    let k = BoomKey { tag: "k" };
    let v = Val { tag: "v" };
    let m: {BoomKey: Val} = {};
    let m2 = m.insert(key: k, value: v);
    print(msg: "inserted");

    let w = [m2]
}
"#;
    let (_exit_code, stdout, _stderr) = compile_and_run_capture(source);
    let inserted = stdout.find("inserted");
    let boom = stdout.find("boom-key-k");
    let val = stdout.find("val-v");
    assert!(
        inserted.is_some() && boom.is_some() && val.is_some(),
        "insert marker + both channel drops must all run; stdout:\n{stdout}"
    );
    assert!(
        inserted < boom,
        "the key drop must fire at teardown, AFTER the insert; stdout:\n{stdout}"
    );
    assert!(
        inserted < val,
        "the value drop must fire at teardown, AFTER the insert; stdout:\n{stdout}"
    );
}
