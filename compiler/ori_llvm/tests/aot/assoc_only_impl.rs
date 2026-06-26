//! AOT pins for associated functions (no `self`) on an impl whose methods are
//! ALL associated functions ("assoc-only impl").
//!
//! The owning-type registration in `type_idx_to_name`
//! (`codegen/function_compiler/impls.rs`) was sourced from `sig.param_types
//! .first()` (the self receiver). A NO-ARG associated function on an assoc-only
//! impl never registered its owning type, so the no-receiver call-site path
//! `lookup_method_by_return_type` (`arc_emitter/apply.rs`) mapped the owning-type
//! `Idx` to nothing and emitted `error[E5001]: unresolved function 'X' in apply`.
//! The fix registers the owning impl-receiver `Idx` (the leading impl-sig tuple
//! element) unconditionally, so a no-receiver call whose return type resolves to
//! the owning type (`-> Self`, or a same-owning-type return) resolves.
//!
//! Non-`Self`-return no-receiver dispatch (`@try_make () -> Result<Widget, str>`
//! called via `Widget.try_make()`) is a DISTINCT defect tracked as BUG-04-228:
//! `lookup_method_by_return_type` keys on the full return type (the `Result`
//! `Idx`, never the owning type), so no owning-type registration can resolve it
//! — proven by experiment (a self-method that registers the owning type does not
//! fix it). Its cure is owning-type-qualifier threading through the ARC `Apply`,
//! not this registration fix.
//!
//! The interpreter false-passes (dynamically typed), so these pins MUST exercise
//! the real AOT/LLVM build (`ori build`) — `assert_cell_output` /
//! `compile_and_run_capture` shell out to the workspace `ori` binary and run the
//! native binary under `ORI_CHECK_LEAKS=1`.
//!
//! Layer coverage: L8 AOT (real `ori build` + native run), L10 leak-freedom
//! (always-on `ORI_CHECK_LEAKS=1`), L6/L7 debug-vs-release (the same `#[test]`
//! bodies run under `cargo test` and `cargo test --release` — `ori_binary()`
//! selects the profile from the test process's own `cfg!(debug_assertions)`).
//! L9 dual-exec parity + L11 spec are pinned by the sibling `.ori` corpus
//! `tests/spec/types/associated_only_impl.ori` under `ori test --backend=llvm`.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::{assert_cell_output, compile_and_run_capture};

// ---------------------------------------------------------------------------
// Cell 1 — Functional north-star (positive pin): trait-impl assoc-ONLY.
// An `impl Widget: Make` whose only method is `@make () -> Self`, called via
// `Widget.make()`. FAIL-FIRST: E5001 `unresolved function `make`` pre-fix.
// ---------------------------------------------------------------------------
const TRAIT_ASSOC_ONLY_SELF_RETURN_SRC: &str = r#"
trait Make { @make () -> Self }
type Widget = { id: int }
impl Widget: Make { @make () -> Self = Widget { id: 7 } }
@main () -> void = {
    let w = Widget.make();
    print(msg: `{w.id}`)
}
"#;

#[test]
fn test_assoc_only_trait_impl_self_return_builds_and_runs() {
    assert_cell_output(
        TRAIT_ASSOC_ONLY_SELF_RETURN_SRC,
        "assoc_only_trait_impl_self_return",
        "7",
    );
}

// ---------------------------------------------------------------------------
// Cell 2 — Inherent assoc-ONLY impl (positive pin).
// `impl Widget { @new () -> Self }` (no trait), called via `Widget.new()`.
// The defect is NOT trait-specific — an inherent assoc-only impl fails too.
// FAIL-FIRST: E5001 `unresolved function `new`` pre-fix.
// ---------------------------------------------------------------------------
const INHERENT_ASSOC_ONLY_SELF_RETURN_SRC: &str = r#"
type Widget = { id: int }
impl Widget { @new () -> Self = Widget { id: 42 } }
@main () -> void = {
    let w = Widget.new();
    print(msg: `{w.id}`)
}
"#;

#[test]
fn test_assoc_only_inherent_impl_self_return_builds_and_runs() {
    assert_cell_output(
        INHERENT_ASSOC_ONLY_SELF_RETURN_SRC,
        "assoc_only_inherent_impl_self_return",
        "42",
    );
}

// ---------------------------------------------------------------------------
// Cell 3 — Non-`Self` return on an assoc-ONLY impl: BUG-04-228 (distinct defect).
// `@try_make () -> Result<Widget, str>` called via `Widget.try_make()`.
// `lookup_method_by_return_type` (`arc_emitter/apply.rs`) keys on the full return
// type (the `Result<Widget,str>` `Idx`, NEVER the owning `Widget` `Idx`), so no
// owning-type registration in `type_idx_to_name` can resolve this call — distinct
// root cause from BUG-04-215's registration gap (proven: a self-method that
// registers the owning type does not fix it). The cure is owning-type-qualifier
// threading through the ARC `Apply` lowering, owned by BUG-04-228. This pin
// asserts the post-BUG-04-228 behavior (stdout `9`); ignored until that lands.
// ---------------------------------------------------------------------------
const ASSOC_ONLY_NON_SELF_RETURN_SRC: &str = r#"
type Widget = { id: int }
impl Widget {
    @try_make () -> Result<Widget, str> = Ok(Widget { id: 9 });
}
@main () -> void = {
    let r = Widget.try_make();
    match r {
        Ok(w) -> print(msg: `{w.id}`),
        Err(e) -> print(msg: `err`),
    }
}
"#;

#[test]
#[ignore = "BUG-04-228: non-Self-return no-receiver associated dispatch — \
            return-type-keyed lookup cannot resolve a non-Self return; distinct \
            root cause + cure surface from BUG-04-215's owning-type registration"]
fn test_assoc_only_impl_non_self_return_resolves_and_runs() {
    assert_cell_output(
        ASSOC_ONLY_NON_SELF_RETURN_SRC,
        "assoc_only_non_self_return",
        "9",
    );
}

// ---------------------------------------------------------------------------
// Cell 6 — Negative/over-broadening pin: distinct types, SAME assoc-fn name,
// both assoc-ONLY. `Widget.make()` and `Gadget.make()` must resolve to their
// OWN owning type — the unconditional `type_idx_to_name` insert must not
// cross-wire two types that share a method name (Q2 collision dimension).
// FAIL-FIRST: E5001 `unresolved function `make`` pre-fix (neither registered).
// ---------------------------------------------------------------------------
const ASSOC_ONLY_DISTINCT_TYPES_SAME_METHOD_SRC: &str = r#"
trait Make { @make () -> Self }
type Widget = { id: int }
type Gadget = { tag: int }
impl Widget: Make { @make () -> Self = Widget { id: 7 } }
impl Gadget: Make { @make () -> Self = Gadget { tag: 99 } }
@main () -> void = {
    let w = Widget.make();
    let g = Gadget.make();
    print(msg: `{w.id} {g.tag}`)
}
"#;

#[test]
fn test_assoc_only_impl_distinct_types_same_method_no_crosswire() {
    assert_cell_output(
        ASSOC_ONLY_DISTINCT_TYPES_SAME_METHOD_SRC,
        "assoc_only_distinct_types_same_method",
        "7 99",
    );
}

// ---------------------------------------------------------------------------
// Cell 4 — Regression / collision pin (Q2): a type with a SELF-method impl
// block AND a SEPARATE assoc-only impl block. The self-method already registers
// the owning type, so `Widget.make()` (assoc) resolves today. The fix's
// unconditional insert writes the SAME `(Widget_idx -> Widget)` entry —
// idempotent; both the assoc call and the self-method call must STAY green.
// PASSES PRE-FIX (its job is to guard against the fix breaking the working
// self-method + sibling-assoc path).
// ---------------------------------------------------------------------------
const SELF_METHOD_BLOCK_PLUS_ASSOC_ONLY_BLOCK_SRC: &str = r#"
type Widget = { id: int }
impl Widget { @make () -> Self = Widget { id: 5 } }
impl Widget { @bump (self) -> int = self.id + 1; }
@main () -> void = {
    let w = Widget.make();
    print(msg: `{w.bump()}`)
}
"#;

#[test]
fn test_assoc_fn_with_sibling_self_method_block_both_resolve() {
    assert_cell_output(
        SELF_METHOD_BLOCK_PLUS_ASSOC_ONLY_BLOCK_SRC,
        "assoc_fn_with_sibling_self_method_block",
        "6",
    );
}

// ---------------------------------------------------------------------------
// Cell 7 — Param-arity regression pin: a MULTI-ARG assoc-only impl
// `@at (x: int, y: int) -> Self`. A no-arg assoc fn fails (cells 1-3,6); a
// multi-arg assoc fn resolves TODAY by coincidence — the first value param's
// `Idx` (sourced by `param_types.first()`) is mis-registered to the owning type
// AND the call site's receiver-lookup keys on that same first-arg `Idx`, so the
// wrong registration and wrong lookup cancel out. The fix replaces that source
// with the owning-receiver `Idx`, so this must STAY green via the CORRECT path.
// PASSES PRE-FIX (by coincidental mis-registration).
// ---------------------------------------------------------------------------
const ASSOC_ONLY_MULTIARG_SRC: &str = r#"
type Widget = { x: int, y: int }
impl Widget { @at (x: int, y: int) -> Self = Widget { x, y } }
@main () -> void = {
    let w = Widget.at(x: 3, y: 4);
    print(msg: `{w.x} {w.y}`)
}
"#;

#[test]
fn test_assoc_only_impl_multiarg_param_arity_builds_and_runs() {
    assert_cell_output(ASSOC_ONLY_MULTIARG_SRC, "assoc_only_multiarg", "3 4");
}

// ---------------------------------------------------------------------------
// Cell 5 — Genuine-unsupported preserved: a call to a NON-EXISTENT associated
// function never produces a runnable binary. The BUG-04-215 fix only ADDS
// owning-type registration for real impl methods — it must NOT over-broaden a
// genuinely unresolvable call into a successful build. PASSES PRE-FIX (that is
// its job: build does not succeed).
//
// NOTE: `Widget.phantom()` currently passes `ori check` (typeck returns an
// ERROR type rather than rejecting the unknown associated function) and then
// panics (PC-2 ICE) in `ori_arc` collections lowering rather than emitting a
// clean diagnostic — that mis-diagnosis is an INDEPENDENT defect tracked
// separately. The invariant this cell pins is backend-stable regardless: a
// non-existent associated function never yields a runnable binary
// (`compile_and_run_capture` returns -1 on the compile failure), pre- and
// post-fix.
// ---------------------------------------------------------------------------
const NONEXISTENT_ASSOC_FN_SRC: &str = r#"
type Widget = { id: int }
impl Widget { @make () -> Self = Widget { id: 1 } }
@main () -> void = {
    let w = Widget.phantom();
    print(msg: `{w.id}`)
}
"#;

#[test]
fn test_nonexistent_assoc_fn_call_never_produces_binary() {
    let (exit_code, stdout, _stderr) = compile_and_run_capture(NONEXISTENT_ASSOC_FN_SRC);
    assert_eq!(
        exit_code, -1,
        "nonexistent assoc fn `Widget.phantom()` must NOT compile to a runnable \
         binary (expected compile failure -1, got exit {exit_code}); stdout: {stdout:?}"
    );
}
