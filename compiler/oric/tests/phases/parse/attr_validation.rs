//! Parser tests for attribute placement validation.
//!
//! Validates that the parser rejects unsupported attributes on item kinds
//! that only accept `#target`/`#cfg` (imports, constants, impls) or no
//! attributes at all (traits, def impls, extends, extern blocks).
//!
//! Spec §25.4: item-level conditional compilation on functions, types,
//! trait implementations, constants, and imports.

use crate::common::{parse_err, parse_ok};

// Unsupported attrs on imports

#[test]
fn reject_skip_on_import() {
    parse_err("#skip(\"nope\")\nuse \"./foo\" { bar };", "#skip");
}

#[test]
fn reject_repr_on_import() {
    parse_err("#repr(\"c\")\nuse \"./foo\" { bar };", "#repr");
}

#[test]
fn reject_derive_on_import() {
    parse_err("#derive(Eq)\nuse \"./foo\" { bar };", "#derive");
}

#[test]
fn accept_target_on_import() {
    let output = parse_ok("#target(os: \"linux\")\nuse \"./foo\" { bar };");
    assert_eq!(output.module.imports.len(), 1);
    assert!(output.module.imports[0].target_attr.is_some());
}

#[test]
fn accept_cfg_on_import() {
    let output = parse_ok("#cfg(debug)\nuse \"./foo\" { bar };");
    assert_eq!(output.module.imports.len(), 1);
    assert!(output.module.imports[0].cfg_attr.is_some());
}

// Unsupported attrs on constants

#[test]
fn reject_repr_on_const() {
    parse_err("#repr(\"c\")\nlet $x = 1;", "#repr");
}

#[test]
fn reject_skip_on_const() {
    parse_err("#skip(\"nope\")\nlet $x = 1;", "#skip");
}

#[test]
fn reject_compile_fail_on_const() {
    parse_err("#compile_fail(\"err\")\nlet $x = 1;", "#compile_fail");
}

#[test]
fn reject_repr_on_bare_dollar_const() {
    parse_err("#repr(\"c\")\n$x = 1;", "#repr");
}

#[test]
fn accept_target_on_const() {
    let output = parse_ok("#target(os: \"linux\")\nlet $x = 42;");
    assert_eq!(output.module.consts.len(), 1);
    assert!(output.module.consts[0].target_attr.is_some());
}

#[test]
fn accept_cfg_on_const() {
    let output = parse_ok("#cfg(debug)\nlet $DEBUG_MODE = true;");
    assert_eq!(output.module.consts.len(), 1);
    assert!(output.module.consts[0].cfg_attr.is_some());
}

// Unsupported attrs on impls

#[test]
fn reject_repr_on_impl() {
    parse_err(
        "type P = { x: int }\n#repr(\"c\")\nimpl P {\n    @f (self) -> int = 0;\n}",
        "#repr",
    );
}

#[test]
fn reject_derive_on_impl() {
    parse_err(
        "type P = { x: int }\n#derive(Eq)\nimpl P {\n    @f (self) -> int = 0;\n}",
        "#derive",
    );
}

#[test]
fn accept_target_on_impl() {
    let output = parse_ok(
        "type P = { x: int }\n#target(os: \"linux\")\nimpl P {\n    @f (self) -> int = 0;\n}",
    );
    assert_eq!(output.module.impls.len(), 1);
    assert!(output.module.impls[0].target_attr.is_some());
}

// Unsupported attrs on extension imports (no attrs allowed per spec §25.4)

#[test]
fn reject_target_on_extension_import() {
    parse_err(
        "#target(os: \"linux\")\nextension std.iter.extensions { Iterator.count }",
        "not supported on extension import",
    );
}

#[test]
fn reject_cfg_on_extension_import() {
    parse_err(
        "#cfg(debug)\nextension std.iter.extensions { Iterator.count }",
        "not supported on extension import",
    );
}

#[test]
fn reject_repr_on_extension_import() {
    parse_err(
        "#repr(\"c\")\nextension std.iter.extensions { Iterator.count }",
        "not supported on extension import",
    );
}

#[test]
fn reject_derive_on_extension_import() {
    parse_err(
        "#derive(Eq)\nextension std.iter.extensions { Iterator.count }",
        "not supported on extension import",
    );
}

#[test]
fn semantic_pin_plain_extension_import_still_works() {
    let output = parse_ok("extension std.iter.extensions { Iterator.count }");
    assert_eq!(output.module.extension_imports.len(), 1);
}

// Unsupported attrs on traits, extends, extern (no attrs allowed)

#[test]
fn reject_any_attr_on_trait() {
    parse_err(
        "#target(os: \"linux\")\ntrait Foo {\n    @bar (self) -> int\n}",
        "not supported on trait",
    );
}

#[test]
fn reject_any_attr_on_extend() {
    parse_err(
        "#target(os: \"linux\")\nextend int {\n    @double (self) -> int = self * 2;\n}",
        "not supported on extension",
    );
}

#[test]
fn reject_any_attr_on_extern() {
    parse_err(
        "#target(os: \"linux\")\nextern \"c\" from \"math\" {\n    @_sin (x: float) -> float as \"sin\"\n}",
        "not supported on extern",
    );
}

// Semantic pin: valid attrs on valid targets must pass

#[test]
fn semantic_pin_cfg_on_const_preserved() {
    let output = parse_ok("#cfg(debug)\nlet $DEBUG_MODE = true;");
    assert_eq!(output.module.consts.len(), 1);
    assert!(
        output.module.consts[0].cfg_attr.is_some(),
        "cfg_attr must be preserved on constants"
    );
}

#[test]
fn semantic_pin_target_on_impl_preserved() {
    let output = parse_ok(
        "type H = { fd: int }\n#target(os: \"linux\")\nimpl H {\n    @close (self) -> int = 0;\n}",
    );
    assert_eq!(output.module.impls.len(), 1);
    assert!(
        output.module.impls[0].target_attr.is_some(),
        "target_attr must be preserved on impls"
    );
}

// Orphaned attrs at EOF must produce errors.
// When attrs are parsed by parse_imports() but no declaration follows (EOF),
// the parser must diagnose them instead of silently dropping.

#[test]
fn reject_orphan_target_at_eof() {
    parse_err("#target(os: \"linux\")\n", "attributes must be followed by");
}

#[test]
fn reject_orphan_cfg_at_eof() {
    parse_err("#cfg(debug)\n", "attributes must be followed by");
}

#[test]
fn reject_orphan_attrs_after_import_at_eof() {
    parse_err(
        "use std.math { sqrt };\n\n#target(os: \"linux\")\n",
        "attributes must be followed by",
    );
}

#[test]
fn semantic_pin_orphan_attrs_eof_is_error_not_silent() {
    // Semantic pin: this test ONLY passes when orphaned attrs at EOF are
    // diagnosed. Without the fix, parsing succeeds silently.
    let output = crate::common::parse_source("#target(os: \"linux\")\n");
    assert!(
        output.has_errors(),
        "semantic pin: orphaned attrs at EOF must produce errors, not be silently dropped"
    );
}

// Diagnostic message must list all valid declaration kinds per §25.4,
// not just "function or test definition". These pins check the FULL message text
// to prevent future drift as new attribute targets are added.

#[test]
fn diagnostic_pin_orphan_attr_full_message_full_parse() {
    // Full-parse EOF path: orphaned #target at end of file.
    parse_err(
        "#target(os: \"linux\")\n",
        "attributes must be followed by a declaration (function, type, impl, constant, import, or test)",
    );
}

#[test]
fn diagnostic_pin_orphan_attr_full_message_after_import() {
    // Full-parse EOF path: orphaned #cfg after an import.
    parse_err(
        "use std.math { sqrt };\n\n#cfg(debug)\n",
        "attributes must be followed by a declaration (function, type, impl, constant, import, or test)",
    );
}

#[test]
fn diagnostic_pin_orphan_attr_full_message_before_invalid_token() {
    // Declaration-error path: attrs followed by an invalid token (number literal).
    parse_err(
        "#target(os: \"linux\")\n42\n",
        "attributes must be followed by a declaration (function, type, impl, constant, import, or test)",
    );
}

#[test]
fn diagnostic_pin_orphan_attr_full_message_before_identifier() {
    // Declaration-error path: attrs followed by an unknown identifier.
    parse_err(
        "#cfg(debug)\nfoo\n",
        "attributes must be followed by a declaration (function, type, impl, constant, import, or test)",
    );
}

// Test-only attrs on a plain (non-test) function — BUG-07-171.
//
// A test-only attribute (`#compile_fail`/`#fail`/`#skip`) on a plain function
// has nowhere to live (the `Function` struct carries no `expected_errors`/
// `fail_expected`), so the parser silently dropped it — a zero-protection ghost
// test. These mirror `reject_compile_fail_on_const`: a plain function cannot
// host a test attribute (Spec: 19-testing.md §19.8 / 10-declarations.md §10.7.3
// define them on `tests`-marked declarations only).

#[test]
fn reject_compile_fail_on_function() {
    parse_err("#compile_fail(\"err\")\n@f () -> int = 1;", "#compile_fail");
}

#[test]
fn reject_fail_on_function() {
    parse_err("#fail(\"boom\")\n@f () -> int = 1;", "#fail");
}

#[test]
fn reject_skip_on_function() {
    parse_err("#skip(\"later\")\n@f () -> int = 1;", "#skip");
}

#[test]
fn reject_skip_backend_on_function() {
    parse_err(
        "#skip(backend: \"llvm\", reason: \"x\")\n@f () -> int = 1;",
        "#skip",
    );
}

// Positive pins — the gate must NOT over-reject. `#fbip`/`#cfg`/`#target` are
// legitimate on a plain function (the `Function` struct carries `is_fbip`/
// `target_attr`/`cfg_attr`); reusing `has_non_conditional_attrs()` (which
// includes `is_fbip`) would regress `#fbip @f`, so the gate uses a
// test-only-scoped predicate.

#[test]
fn accept_fbip_on_function() {
    let output = parse_ok("#fbip\n@f () -> int = 1;");
    assert_eq!(output.module.functions.len(), 1);
    assert!(output.module.functions[0].is_fbip);
}

#[test]
fn accept_cfg_on_function() {
    let output = parse_ok("#cfg(debug)\n@f () -> int = 1;");
    assert_eq!(output.module.functions.len(), 1);
    assert!(output.module.functions[0].cfg_attr.is_some());
}

#[test]
fn accept_target_on_function() {
    let output = parse_ok("#target(os: \"linux\")\n@f () -> int = 1;");
    assert_eq!(output.module.functions.len(), 1);
    assert!(output.module.functions[0].target_attr.is_some());
}

// Positive pins — test-marked functions DO legitimately host test attributes;
// the gate fires only on the plain-function branch, never the `tests` branch.

#[test]
fn accept_compile_fail_on_floating_test() {
    let output = parse_ok("#compile_fail(\"err\")\n@check tests _ () -> void = { () }");
    assert_eq!(output.module.tests.len(), 1);
    assert!(output.module.tests[0].is_compile_fail());
}

#[test]
fn accept_compile_fail_on_attached_test() {
    let output = parse_ok("#compile_fail(\"err\")\n@check tests @target () -> void = { () }");
    assert_eq!(output.module.tests.len(), 1);
    assert!(output.module.tests[0].is_compile_fail());
}

#[test]
fn accept_skip_on_test() {
    let output = parse_ok("#skip(\"later\")\n@check tests _ () -> void = { () }");
    assert_eq!(output.module.tests.len(), 1);
    assert!(output.module.tests[0].skip_reason.is_some());
}

#[test]
fn accept_fail_on_test() {
    let output = parse_ok("#fail(\"boom\")\n@check tests _ () -> void = { () }");
    assert_eq!(output.module.tests.len(), 1);
    assert!(output.module.tests[0].fail_expected.is_some());
}

#[test]
fn accept_skip_backend_on_test() {
    let output =
        parse_ok("#skip(backend: \"llvm\", reason: \"x\")\n@check tests _ () -> void = { () }");
    assert_eq!(output.module.tests.len(), 1);
    assert!(!output.module.tests[0].skip_backends.is_empty());
}

// Test-only attrs on a type declaration — BUG-07-171 (second ghost host).
//
// Type declarations legitimately carry `#derive`/`#repr`/`#target`/`#cfg`, so
// the type-decl dispatch branch cannot reuse `has_non_conditional_attrs()`
// (which rejects derive/repr). A test-only attr on a `type` was therefore
// silently dropped (type_decl.rs consumes derives/repr_attrs/target/cfg, never
// expected_errors/fail_expected/skip_*) — the same ghost class, cured by the
// same `has_test_only_attrs()` predicate at the dispatch type-decl branch.

#[test]
fn reject_compile_fail_on_type() {
    parse_err(
        "#compile_fail(\"err\")\ntype Bad = { x: int }",
        "#compile_fail",
    );
}

#[test]
fn reject_fail_on_type() {
    parse_err("#fail(\"boom\")\ntype Bad = { x: int }", "#fail");
}

#[test]
fn reject_skip_on_type() {
    parse_err("#skip(\"later\")\ntype Bad = { x: int }", "#skip");
}

// Positive pins — `#derive`/`#repr` are legitimate on a type declaration; the
// gate must NOT over-reject (it uses the test-only-scoped predicate).

#[test]
fn accept_derive_on_type() {
    let output = parse_ok("#derive(Eq)\ntype P = { x: int }");
    assert_eq!(output.module.types.len(), 1);
    assert!(!output.module.types[0].derives.is_empty());
}

#[test]
fn accept_repr_on_type() {
    let output = parse_ok("#repr(\"c\")\ntype P = { x: int }");
    assert_eq!(output.module.types.len(), 1);
    assert!(!output.module.types[0].repr_attrs.is_empty());
}

#[test]
fn reject_skip_backend_on_type() {
    parse_err(
        "#skip(backend: \"llvm\", reason: \"x\")\ntype Bad = { x: int }",
        "#skip",
    );
}

// `#skip(backend:)` sets `skip_backends`, NOT `skip_reason`. The const/import/impl
// gate (`has_non_conditional_attrs()`) omitted `skip_backends`, so a backend-skip
// on a const was a THIRD silently-dropped ghost host. The fix adds `skip_backends`
// to that predicate; this pin clamps it.

#[test]
fn reject_skip_backend_on_const() {
    parse_err(
        "#skip(backend: \"llvm\", reason: \"x\")\nlet $x = 1;",
        "#skip",
    );
}

// `is_empty()` (the trait/extern/def-impl/extend reject-all gate) ALSO omitted
// `skip_backends`, so a backend-skip on a trait was a FOURTH silently-dropped
// ghost host (`#skip(backend:)` alone made `is_empty()` return true). The fix
// adds `skip_backends` to `is_empty()`; this pin clamps it.

#[test]
fn reject_skip_backend_on_trait() {
    parse_err(
        "#skip(backend: \"llvm\", reason: \"x\")\ntrait Foo {\n    @bar (self) -> int\n}",
        "not supported on trait",
    );
}
