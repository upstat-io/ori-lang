//! Parser tests for extern block declarations.
//!
//! Grammar:
//! ```ebnf
//! extern_block  = [ "pub" ] "extern" string_literal [ "from" string_literal ] "{" { extern_item } "}" .
//! extern_item   = "@" identifier extern_params "->" type [ "as" string_literal ] .
//! extern_params = "(" [ extern_param { "," extern_param } ] [ c_variadic ] ")" .
//! extern_param  = identifier ":" type .
//! c_variadic    = "," "..." .
//! ```

use crate::common::{parse_err, parse_ok};

// Basic extern blocks

#[test]
fn test_extern_c_basic() {
    let output = parse_ok("extern \"c\" {\n    @sin (x: float) -> float\n}");
    assert_eq!(output.module.extern_blocks.len(), 1);
    let block = &output.module.extern_blocks[0];
    assert_eq!(block.items.len(), 1);
    assert!(!block.items[0].is_c_variadic);
    assert!(block.items[0].alias.is_none());
    assert!(block.library.is_none());
    assert_eq!(block.visibility, ori_ir::Visibility::Private);
}

#[test]
fn test_extern_js_basic() {
    let output = parse_ok("extern \"js\" {\n    @_now () -> float as \"Date.now\"\n}");
    assert_eq!(output.module.extern_blocks.len(), 1);
    let block = &output.module.extern_blocks[0];
    assert_eq!(block.items.len(), 1);
    assert!(block.items[0].alias.is_some());
}

#[test]
fn test_extern_empty_block() {
    let output = parse_ok("extern \"c\" {\n}");
    assert_eq!(output.module.extern_blocks.len(), 1);
    assert!(output.module.extern_blocks[0].items.is_empty());
}

// `from` clause

#[test]
fn test_extern_from_library() {
    let output = parse_ok("extern \"c\" from \"m\" {\n    @sin (x: float) -> float\n}");
    let block = &output.module.extern_blocks[0];
    assert!(block.library.is_some());
}

#[test]
fn test_extern_from_relative_path() {
    let output =
        parse_ok("extern \"c\" from \"./native/libcustom.so\" {\n    @custom () -> void\n}");
    assert!(output.module.extern_blocks[0].library.is_some());
}

#[test]
fn test_extern_js_from_module() {
    let output = parse_ok(
        "extern \"js\" from \"./utils.js\" {\n    @_fmt (ts: int) -> str as \"formatDate\"\n}",
    );
    let block = &output.module.extern_blocks[0];
    assert!(block.library.is_some());
    assert!(block.items[0].alias.is_some());
}

// `as` alias syntax

#[test]
fn test_extern_as_alias() {
    let output = parse_ok(
        "extern \"c\" from \"m\" {\n    @_sin (x: float) -> float as \"sin\"\n    @_sqrt (x: float) -> float as \"sqrt\"\n}",
    );
    let block = &output.module.extern_blocks[0];
    assert_eq!(block.items.len(), 2);
    assert!(block.items[0].alias.is_some());
    assert!(block.items[1].alias.is_some());
}

#[test]
fn test_extern_mixed_alias() {
    let output = parse_ok(
        "extern \"c\" from \"m\" {\n    @sin (x: float) -> float\n    @_abs (x: float) -> float as \"fabs\"\n}",
    );
    let block = &output.module.extern_blocks[0];
    assert_eq!(block.items.len(), 2);
    assert!(block.items[0].alias.is_none()); // sin — no alias
    assert!(block.items[1].alias.is_some()); // _abs as "fabs"
}

// C variadics

#[test]
fn test_extern_c_variadic() {
    let output = parse_ok("extern \"c\" {\n    @printf (fmt: CPtr, ...) -> c_int\n}");
    let block = &output.module.extern_blocks[0];
    assert_eq!(block.items.len(), 1);
    assert!(block.items[0].is_c_variadic);
    assert_eq!(block.items[0].params.len(), 1); // fmt is the only named param
}

#[test]
fn test_extern_c_variadic_no_params() {
    // Unusual but grammatically valid: `(...)` with no named params
    let output = parse_ok("extern \"c\" {\n    @va_func (...) -> void\n}");
    let block = &output.module.extern_blocks[0];
    assert!(block.items[0].is_c_variadic);
    assert!(block.items[0].params.is_empty());
}

// Visibility

#[test]
fn test_extern_pub() {
    let output = parse_ok("pub extern \"c\" from \"m\" {\n    @sin (x: float) -> float\n}");
    assert_eq!(
        output.module.extern_blocks[0].visibility,
        ori_ir::Visibility::Public
    );
}

#[test]
fn test_extern_private() {
    let output = parse_ok("extern \"c\" from \"m\" {\n    @sin (x: float) -> float\n}");
    assert_eq!(
        output.module.extern_blocks[0].visibility,
        ori_ir::Visibility::Private
    );
}

// Multiple items

#[test]
fn test_extern_multiple_items() {
    let output = parse_ok(
        "extern \"c\" from \"m\" {\n    @sin (x: float) -> float\n    @cos (x: float) -> float\n    @tan (x: float) -> float\n}",
    );
    assert_eq!(output.module.extern_blocks[0].items.len(), 3);
}

#[test]
fn test_extern_multiple_params() {
    let output = parse_ok(
        "extern \"c\" from \"libc\" {\n    @qsort (base: CPtr, nmemb: int, size: int) -> void\n}",
    );
    assert_eq!(output.module.extern_blocks[0].items[0].params.len(), 3);
}

// Multiple extern blocks

#[test]
fn test_multiple_extern_blocks() {
    let output = parse_ok(
        "extern \"c\" from \"m\" {\n    @sin (x: float) -> float\n}\nextern \"js\" {\n    @_now () -> float as \"Date.now\"\n}",
    );
    assert_eq!(output.module.extern_blocks.len(), 2);
}

// Extern blocks mixed with functions

#[test]
fn test_extern_with_functions() {
    let output = parse_ok(
        "extern \"c\" from \"m\" {\n    @_sin (x: float) -> float as \"sin\"\n}\n@main () -> void = ()",
    );
    assert_eq!(output.module.extern_blocks.len(), 1);
    assert_eq!(output.module.functions.len(), 1);
}

// Error cases

#[test]
fn test_extern_missing_convention() {
    parse_err(
        "extern {\n    @sin (x: float) -> float\n}",
        "expected calling convention",
    );
}

#[test]
fn test_extern_missing_lbrace() {
    parse_err("extern \"c\"\n    @sin (x: float) -> float", "expected {");
}

#[test]
fn test_extern_missing_from_string() {
    parse_err(
        "extern \"c\" from {\n    @sin (x: float) -> float\n}",
        "expected library path string",
    );
}

#[test]
fn test_extern_as_not_string() {
    parse_err(
        "extern \"c\" {\n    @sin (x: float) -> float as sin\n}",
        "expected string literal after `as`",
    );
}
