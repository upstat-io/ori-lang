//! Tests for symbol mangling (`ori_llvm::aot::mangle`).
//!
//! These tests verify:
//! - Simple function mangling
//! - Module path encoding
//! - Trait impl mangling
//! - Extension mangling
//! - Generic type mangling
//! - Associated function mangling
//! - Demangling roundtrips
//! - Special character encoding

use ori_llvm::aot::mangle::{demangle, extract_function_name, is_ori_symbol, Mangler};

#[test]
fn test_mangle_simple_function() {
    let mangler = Mangler::new();
    assert_eq!(mangler.mangle_function("", "main"), "_ori_main");
    assert_eq!(mangler.mangle_function("", "add"), "_ori_add");
}

#[test]
fn test_mangle_module_function() {
    let mangler = Mangler::new();
    assert_eq!(mangler.mangle_function("math", "add"), "_ori_math$add");
    assert_eq!(
        mangler.mangle_function("data/utils", "process"),
        "_ori_data$utils$process"
    );
    assert_eq!(
        mangler.mangle_function("std.io", "read"),
        "_ori_std$io$read"
    );
}

#[test]
fn test_mangle_trait_impl() {
    let mangler = Mangler::new();
    assert_eq!(
        mangler.mangle_trait_impl("int", "Eq", "equals"),
        "_ori_int$$Eq$equals"
    );
    assert_eq!(
        mangler.mangle_trait_impl("Point", "Clone", "clone"),
        "_ori_Point$$Clone$clone"
    );
}

#[test]
fn test_mangle_extension() {
    let mangler = Mangler::new();
    assert_eq!(
        mangler.mangle_extension("[int]", "sum", ""),
        "_ori_$LBint$RB$$ext$sum"
    );
    assert_eq!(
        mangler.mangle_extension("str", "to_upper", "string_utils"),
        "_ori_str$$ext$string_utils$to_upper"
    );
}

#[test]
fn test_mangle_generic() {
    let mangler = Mangler::new();
    assert_eq!(
        mangler.mangle_generic("", "identity", &["int"]),
        "_ori_identity$Gint"
    );
    assert_eq!(
        mangler.mangle_generic("", "map", &["int", "str"]),
        "_ori_map$Gint_str"
    );
}

#[test]
fn test_mangle_associated_function() {
    let mangler = Mangler::new();
    assert_eq!(
        mangler.mangle_associated_function("Option", "some"),
        "_ori_Option$A$some"
    );
    assert_eq!(
        mangler.mangle_associated_function("Result", "ok"),
        "_ori_Result$A$ok"
    );
}

#[test]
fn test_demangle_simple() {
    assert_eq!(demangle("_ori_main"), Some("@main".to_string()));
    assert_eq!(demangle("_ori_add"), Some("@add".to_string()));
}

#[test]
fn test_demangle_module() {
    // Ori-style: module.@function
    assert_eq!(demangle("_ori_math$add"), Some("math.@add".to_string()));
    // Nested modules: module/submodule.@function
    assert_eq!(
        demangle("_ori_data$utils$process"),
        Some("data/utils.@process".to_string())
    );
}

#[test]
fn test_demangle_trait_impl() {
    // Trait impl: type::Trait.@method
    assert_eq!(
        demangle("_ori_int$$Eq$equals"),
        Some("int::Eq.@equals".to_string())
    );
}

#[test]
fn test_demangle_not_ori_symbol() {
    assert_eq!(demangle("_ZN3foo3barE"), None);
    assert_eq!(demangle("printf"), None);
    assert_eq!(demangle(""), None);
}

#[test]
fn test_is_ori_symbol() {
    assert!(is_ori_symbol("_ori_main"));
    assert!(is_ori_symbol("_ori_math$add"));
    assert!(!is_ori_symbol("_ZN3foo3barE"));
    assert!(!is_ori_symbol("printf"));
}

#[test]
fn test_extract_function_name() {
    assert_eq!(extract_function_name("_ori_main"), Some("main"));
    assert_eq!(extract_function_name("_ori_math$add"), Some("add"));
    assert_eq!(
        extract_function_name("_ori_data$utils$process"),
        Some("process")
    );
}

#[test]
fn test_mangle_special_characters() {
    let mangler = Mangler::new();
    // Generic types
    assert_eq!(
        mangler.mangle_function("", "Option<int>"),
        "_ori_Option$LTint$GT"
    );
    // Array types
    assert_eq!(mangler.mangle_function("", "[int]"), "_ori_$LBint$RB");
}

#[test]
fn test_roundtrip() {
    let mangler = Mangler::new();

    // Test that demangling produces Ori-style readable output
    let cases = [
        ("", "main", "@main"),
        ("math", "add", "math.@add"),
        ("std.io", "read", "std/io.@read"),
    ];

    for (module, func, expected) in cases {
        let mangled_name = mangler.mangle_function(module, func);
        let demangled = demangle(&mangled_name).expect("should demangle");
        assert_eq!(
            demangled, expected,
            "demangled '{demangled}' should equal '{expected}'"
        );
    }
}

#[test]
fn test_mangler_for_windows() {
    let mangler = Mangler::for_windows();
    // Windows mangler should still produce valid output
    assert_eq!(mangler.mangle_function("", "main"), "_ori_main");
}

#[test]
fn test_mangle_special_characters_extended() {
    let mangler = Mangler::new();

    // Comma in generic types (e.g., Map<int, str>)
    assert!(mangler.mangle_function("", "Map<int, str>").contains("$C"));

    // Parentheses in function types
    assert!(mangler.mangle_function("", "(int) -> str").contains("$LP"));
    assert!(mangler.mangle_function("", "(int) -> str").contains("$RP"));

    // Colon in qualified paths
    assert!(mangler.mangle_function("", "Foo::Bar").contains("$CC"));

    // Dash in identifiers
    assert!(mangler.mangle_function("", "my-func").contains("$D"));

    // Space gets converted to underscore
    assert!(mangler.mangle_function("", "my func").contains('_'));
}

#[test]
fn test_mangle_hex_escape() {
    let mangler = Mangler::new();

    // Characters not in the allowed set get hex-escaped
    // '@' = 0x40 = 64
    let result = mangler.mangle_function("", "foo@bar");
    assert!(result.contains("$40"));

    // '#' = 0x23 = 35
    let result = mangler.mangle_function("", "foo#bar");
    assert!(result.contains("$23"));
}

#[test]
fn test_mangle_module_with_special_paths() {
    let mangler = Mangler::new();

    // Module path with backslash (Windows-style)
    let result = mangler.mangle_function("foo\\bar", "baz");
    assert_eq!(result, "_ori_foo$bar$baz");

    // Module path with colon gets encoded as module separator
    let result = mangler.mangle_function("C:/foo", "bar");
    // Colon becomes $ (module separator), so C: becomes C$
    assert!(result.contains('$'));
}

#[test]
fn test_demangle_special_characters() {
    // Demangle symbols with special character encodings

    // Array type with brackets
    let demangled = demangle("_ori_$LBint$RB");
    assert!(demangled.is_some());
    assert!(demangled.as_ref().unwrap().contains('['));
    assert!(demangled.as_ref().unwrap().contains(']'));

    // Function type with parentheses
    let demangled = demangle("_ori_$LPint$RP");
    assert!(demangled.is_some());
    assert!(demangled.as_ref().unwrap().contains('('));
    assert!(demangled.as_ref().unwrap().contains(')'));

    // Comma in generics
    let demangled = demangle("_ori_Map$Cint");
    assert!(demangled.is_some());
    assert!(demangled.unwrap().contains(','));

    // Dash in identifier
    let demangled = demangle("_ori_my$Dfunc");
    assert!(demangled.is_some());
    assert!(demangled.unwrap().contains('-'));

    // Generic marker $G adds opening angle bracket
    let demangled = demangle("_ori_identity$Gint");
    assert!(demangled.is_some());
    assert!(demangled.unwrap().contains('<'));

    // Associated function marker
    let demangled = demangle("_ori_Option$A$some");
    assert!(demangled.is_some());
    assert!(demangled.unwrap().contains('.'));

    // Qualified path with double colon $CC
    let demangled = demangle("_ori_foo$CCbar");
    assert!(demangled.is_some());
    assert!(demangled.unwrap().contains("::"));

    // Open angle bracket via $LT
    let demangled = demangle("_ori_Option$LTint");
    assert!(demangled.is_some());
    assert!(demangled.unwrap().contains('<'));
}

#[test]
fn test_demangle_incomplete_escapes() {
    // Test incomplete escape sequences - these should fallback gracefully

    // Incomplete $L (no following char) - falls back to $L
    let demangled = demangle("_ori_test$L");
    assert!(demangled.is_some());

    // Incomplete $R (no following char) - falls back to $R
    let demangled = demangle("_ori_test$R");
    assert!(demangled.is_some());

    // Incomplete $C with CC check
    let demangled = demangle("_ori_test$C");
    assert!(demangled.is_some());
    // Should be treated as comma
    assert!(demangled.unwrap().contains(','));
}
