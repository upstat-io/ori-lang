//! AOT codegen tests for tagless single-variant enums (`EnumTag::None`).
//!
//! Regression matrix for a tagless single-variant enum carrying a heap/RC
//! payload. Pre-fix, codegen mis-routed the tagless layout through the niche
//! drop/RC path (`emit_drop_enum_niche` / `emit_inline_enum_*`), panicking on
//! `niche_field_index().unwrap()` because a tagless enum has no niche field.
//! Fix routes `EnumTag::None` struct-like (direct field GEP + recursive-field
//! boxing) via `tagless_enum.rs` + `is_tagless_enum`.

use crate::util::assert_aot_success;

/// Scalar payload — no RC, must NOT take the niche/RC path. Boundary clamp:
/// a tagless enum with a non-heap payload builds and drops trivially.
#[test]
fn tagless_enum_scalar_payload_builds_runs() {
    let source = r"
use std.testing { assert_eq }
type Wrap = W(n: int);
@main () -> void = {
    let w = W(n: 42);
    let v = match w { W(n) -> n };
    assert_eq(actual: v, expected: 42);
    ()
}
";
    assert_aot_success(source, "tagless_enum_scalar_payload_builds_runs");
}

/// `str` payload — the canonical non-recursive repro that panicked pre-fix at
/// `emit_drop_enum_niche`. Build + run + leak-clean.
#[test]
fn tagless_enum_str_payload_drops_no_leak() {
    let source = r#"
type Wrap = W(s: str);
@main () -> void = {
    let w = W(s: "hello");
    ()
}
"#;
    assert_aot_success(source, "tagless_enum_str_payload_drops_no_leak");
}

/// `[int]` payload — collection element must dec-through on drop.
#[test]
fn tagless_enum_list_payload_reads_and_drops() {
    let source = r"
use std.testing { assert_eq }
type Wrap = W(xs: [int]);
@main () -> void = {
    let w = W(xs: [1, 2, 3]);
    let xs = match w { W(xs) -> xs };
    assert_eq(actual: xs.length(), expected: 3);
    ()
}
";
    assert_aot_success(source, "tagless_enum_list_payload_reads_and_drops");
}

/// Struct payload carrying a heap field — nested RC dec-through on the tagless
/// struct-like drop path.
#[test]
fn tagless_enum_struct_payload_reads_field() {
    let source = r#"
use std.testing { assert_eq }
type Inner = { label: str, count: int }
type Wrap = W(inner: Inner);
@main () -> void = {
    let w = W(inner: Inner { label: "a", count: 7 });
    let c = match w { W(inner) -> inner.count };
    assert_eq(actual: c, expected: 7);
    ()
}
"#;
    assert_aot_success(source, "tagless_enum_struct_payload_reads_field");
}

/// Aliased tagless value with a heap payload — shared RC, both alive to scope
/// exit, no double-free. Pins the inline-enum RC inc/dec tagless path.
#[test]
fn tagless_enum_aliased_no_double_free() {
    let source = r#"
use std.testing { assert_eq }
type Wrap = W(s: str);
@main () -> void = {
    let a = W(s: "shared");
    let b = a;
    let va = match a { W(s) -> s };
    let vb = match b { W(s) -> s };
    assert_eq(actual: va, expected: "shared");
    assert_eq(actual: vb, expected: "shared");
    ()
}
"#;
    assert_aot_success(source, "tagless_enum_aliased_no_double_free");
}
