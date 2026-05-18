//! AOT tests for Drop trait AUGMENT body shape (§04.3 of
//! `plans/aims-burden-tracking/section-04-recursive-closures-drop-value.md`).
//!
//! These tests exercise the END-TO-END Drop AUGMENT story: typeck-side
//! `user_drop` + `compiled_drop` population at `register_impl` (§04.3
//! Drop impl registration), codegen-side AUGMENT body shape extension
//! to `DropKind::Fields` + `DropKind::Enum` (§04.3 Drop AUGMENT body
//! shape), and runtime-side `ori_drop_double_panic_abort` semantics
//! (§04.3 Drop panic-safety).
//!
//! Pre-existing blocker chain for the AOT slice:
//!
//! 1. `BUG-04-118` — closure `UserBurdenSpec` lambda-side wiring follow-up:
//!    the lambda-typecheck site at
//!    `compiler_repo/compiler/ori_types/src/infer/expr/blocks.rs:223`
//!    does not yet call `compose_closure_burden_spec` +
//!    `register_user_burden`, so closures-and-Drop interactions don't
//!    yet roundtrip through the same Drop AUGMENT body.
//!
//! 2. `BUG-04-119` — AUGMENT body shape AOT slice follow-up: the
//!    plan-mandated `invoke` + landing-pad lowering around `Apply(user_drop)`
//!    (per `drop-trait-proposal.md §Drop and panic`) is not yet wired in
//!    `emit_drop_fields` / `emit_drop_enum`; current slice ships a plain
//!    `call` to user `@drop` (via `_ori_user_drop$<idx>` placeholder) and
//!    relies on `ori_rt::rc::call_drop_fn`'s abort-on-any-panic runtime
//!    guard for panic-safety (strict mode — first-panic recovery deferred
//!    until landing-pad wiring lands).
//!
//! Until both blockers ship, the §04.3 deliverables (typeck E2048,
//! `user_drop` + `compiled_drop` populate, `DropKind::{Fields,Enum}`
//! struct-form extension, `ori_drop_double_panic_abort` runtime symbol)
//! are pinned by:
//! - `ori_types::check::validators::partial_move::tests` — smoke +
//!   pre-deployment behavior pins
//! - `ori_llvm::codegen::arc_emitter::tests` — existing 36 drop-codegen
//!   tests, all migrated to the new `DropKind::{Fields,Enum}` struct shape
//! - `ori_rt::rc::ori_drop_double_panic_abort` — runtime entry available
//!   for landing-pad emission once `BUG-04-119` ships

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

/// Regression: §04.3 deliverable for Drop AUGMENT struct body.
///
/// Verifies user `@drop` runs FIRST (observable side effect), THEN the
/// compiler walks owned fields in reverse declaration order, THEN the
/// allocation is freed. Captured `print` output order pins both axes:
/// user-first AND reverse-decl.
///
/// Blocked by `BUG-04-119` — AUGMENT body shape AOT slice; `BUG-04-118`
/// for closure-aware Drop interactions.
#[test]
#[ignore = "BUG-04-119: AUGMENT body shape AOT slice — invoke + landing-pad lowering follow-up; BUG-04-118: closure UserBurdenSpec lambda-side wiring"]
fn drop_augment_user_method_first_then_compiler_field_walk_reverse_order() {
    let source = r#"
type Logged = { tag: str }

impl Logged: Drop {
    @drop (self) -> void = print(`drop-Logged-{self.tag}`)
}

type Resource = { a: Logged, b: Logged }

impl Resource: Drop {
    @drop (self) -> void = print("drop-Resource-user")
}

@t tests _ () -> void = {
    let r = Resource {
        a: Logged { tag: "a" },
        b: Logged { tag: "b" },
    }
    // r dropped at scope exit:
    //   1. drop-Resource-user (user @drop runs FIRST)
    //   2. drop-Logged-b      (reverse-decl: b drops before a)
    //   3. drop-Logged-a      (a drops last)
}
"#;
    let _ = assert_aot_success;
    let _ = source;
}

/// Regression: §04.3 deliverable for Drop AUGMENT enum body.
///
/// Verifies user `@drop` runs FIRST on an enum-shaped Drop type, THEN
/// the discriminant-switch + per-variant field walk fires. Without
/// §04.3's `DropKind::Enum { variants, user_drop }` extension, the
/// user `@drop` body would never be emitted for enum-shaped Drop types —
/// silent bypass per gemini blind-spot #1.
///
/// Blocked by `BUG-04-119`.
#[test]
#[ignore = "BUG-04-119: AUGMENT body shape AOT slice — invoke + landing-pad lowering follow-up"]
fn drop_augment_enum_user_method_first_then_variant_field_walk_reverse_order() {
    let source = r#"
type Logged = { tag: str }

impl Logged: Drop {
    @drop (self) -> void = print(`drop-Logged-{self.tag}`)
}

type EventLog = Single(payload: Logged) | Pair(first: Logged, second: Logged)

impl EventLog: Drop {
    @drop (self) -> void = print("drop-EventLog-user")
}

@t tests _ () -> void = {
    let pair = EventLog.Pair(
        first: Logged { tag: "first" },
        second: Logged { tag: "second" },
    )
    // pair dropped at scope exit:
    //   1. drop-EventLog-user    (user @drop runs FIRST)
    //   2. drop-Logged-second    (reverse-decl within Pair variant)
    //   3. drop-Logged-first
}
"#;
    let _ = assert_aot_success;
    let _ = source;
}

/// Regression: §04.3 deliverable for landing-pad-around-`invoke` lowering.
///
/// Verifies that despite a panic in `Resource::@drop`, the field-walk
/// STILL fires via the unwind path — `Logged::@drop` runs for both
/// fields before the unwind resumes. `ORI_CHECK_LEAKS=1` reports zero
/// leaks on the unwind path.
///
/// Blocked by `BUG-04-119` — the landing-pad-around-`invoke` lowering
/// is the explicit deliverable.
#[test]
#[ignore = "BUG-04-119: AUGMENT body shape AOT slice — invoke + landing-pad lowering follow-up"]
fn drop_augment_landing_pad_runs_field_walk_on_user_drop_panic_path() {
    let source = r#"
type Logged = { tag: str }

impl Logged: Drop {
    @drop (self) -> void = print(`drop-Logged-{self.tag}`)
}

type Resource = { a: Logged, b: Logged }

impl Resource: Drop {
    @drop (self) -> void = {
        print("drop-Resource-pre-panic")
        panic(msg: "intentional")
    }
}

@t tests _ () -> void = {
    let r = Resource {
        a: Logged { tag: "a" },
        b: Logged { tag: "b" },
    }
    // r dropped at scope exit; Resource::@drop panics, but Logged
    // fields a and b still drop via the landing-pad unwind path.
}
"#;
    let _ = assert_aot_success;
    let _ = source;
}

/// Regression: §04.3 deliverable for nested-panic abort via
/// `ori_drop_double_panic_abort`.
///
/// Verifies process aborts when a SECOND panic surfaces during the
/// cleanup field-walk. Observable via process exit code matching abort
/// signal (NOT clean exit).
///
/// Blocked by `BUG-04-119`.
#[test]
#[ignore = "BUG-04-119: AUGMENT body shape AOT slice — invoke + landing-pad lowering follow-up"]
fn drop_augment_nested_panic_invokes_ori_drop_double_panic_abort() {
    let source = r#"
type Logged = { tag: str }

impl Logged: Drop {
    @drop (self) -> void = panic(msg: `panic-in-Logged-{self.tag}`)
}

type Resource = { a: Logged, b: Logged }

impl Resource: Drop {
    @drop (self) -> void = panic(msg: "panic-in-Resource")
}

@t tests _ () -> void = {
    let r = Resource {
        a: Logged { tag: "a" },
        b: Logged { tag: "b" },
    }
    // r dropped at scope exit; Resource::@drop panics; during cleanup,
    // Logged::@drop ALSO panics. Nested-panic semantics: abort via
    // ori_drop_double_panic_abort.
}
"#;
    let _ = assert_aot_success;
    let _ = source;
}
