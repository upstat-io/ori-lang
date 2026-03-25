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
