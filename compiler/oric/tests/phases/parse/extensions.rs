//! Parser tests for extension definitions and extension imports.
//!
//! Extension definitions:
//! - Grammar: `extension_def = "extend" [ generics ] type [ where_clause ] "{" { method } "}" .`
//!
//! Extension imports:
//! - Grammar: `extension_import = "extension" import_path "{" extension_item { "," extension_item } "}" .`
//! - Grammar: `extension_item = identifier "." identifier .`

use crate::common::{parse_err, parse_ok};

// Extension definitions

#[test]
fn test_extend_basic() {
    let output = parse_ok("extend Point {\n    @distance (self) -> float = 0.0;\n}");
    assert_eq!(output.module.extends.len(), 1);
}

#[test]
fn test_extend_with_where_clause() {
    let output =
        parse_ok("extend List<T> where T: Eq {\n    @contains (self, item: T) -> bool = false;\n}");
    assert_eq!(output.module.extends.len(), 1);
}

#[test]
fn test_extend_with_multiple_bounds() {
    let output = parse_ok(
        "extend Map<K, V> where K: Eq + Hashable, V: Clone {\n    @deep_clone (self) -> Map<K, V> = self;\n}",
    );
    assert_eq!(output.module.extends.len(), 1);
}

#[test]
fn test_extend_with_where_clause_multiple_methods() {
    let output = parse_ok(
        "extend List<T> where T: Eq {\n    @contains (self, x: T) -> bool = false;\n    @index_of (self, x: T) -> int = 0;\n}",
    );
    assert_eq!(output.module.extends.len(), 1);
}

// Extension imports

#[test]
fn test_extension_import_basic() {
    let output =
        parse_ok("extension std.iter.extensions { Iterator.count }\n@main () -> void = ();");
    assert_eq!(output.module.extension_imports.len(), 1);
    let ext = &output.module.extension_imports[0];
    assert_eq!(ext.items.len(), 1);
}

#[test]
fn test_extension_import_multiple_items() {
    let output = parse_ok(
        "extension std.iter.extensions { Iterator.count, Iterator.last }\n@main () -> void = ();",
    );
    let ext = &output.module.extension_imports[0];
    assert_eq!(ext.items.len(), 2);
}

#[test]
fn test_extension_import_relative_path() {
    let output = parse_ok("extension \"./my_ext\" { Iterator.sum }\n@main () -> void = ();");
    assert_eq!(output.module.extension_imports.len(), 1);
    let ext = &output.module.extension_imports[0];
    assert!(matches!(ext.path, ori_ir::ImportPath::Relative(_)));
    assert_eq!(ext.items.len(), 1);
}

#[test]
fn test_extension_import_public() {
    let output =
        parse_ok("pub extension std.iter.extensions { Iterator.count }\n@main () -> void = ();");
    let ext = &output.module.extension_imports[0];
    assert_eq!(ext.visibility, ori_ir::Visibility::Public);
}

#[test]
fn test_extension_import_private() {
    let output =
        parse_ok("extension std.iter.extensions { Iterator.count }\n@main () -> void = ();");
    let ext = &output.module.extension_imports[0];
    assert_eq!(ext.visibility, ori_ir::Visibility::Private);
}

#[test]
fn test_extension_import_with_regular_imports() {
    let output = parse_ok(
        "use std.testing { assert_eq }\nextension std.iter { Iterator.count }\n@main () -> void = ();",
    );
    assert_eq!(output.module.imports.len(), 1);
    assert_eq!(output.module.extension_imports.len(), 1);
}

#[test]
fn test_extension_import_multiple_types() {
    let output = parse_ok(
        "extension std.collections.extensions { Vec.sort, Vec.reverse, Map.keys }\n@main () -> void = ();",
    );
    let ext = &output.module.extension_imports[0];
    assert_eq!(ext.items.len(), 3);
}

// Error cases

#[test]
fn test_extension_import_missing_dot() {
    parse_err(
        "extension std.iter { Iterator }\n@main () -> void = ();",
        "expected .",
    );
}
