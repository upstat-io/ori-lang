/// Tests for the demangle logic.
///
/// The `demangle_symbol` function prints to stdout, so we test the underlying
/// demangling functions from `ori_llvm::aot::mangle`.
#[cfg(feature = "llvm")]
mod llvm_tests {
    use ori_llvm::aot::{demangle, is_ori_symbol};

    #[test]
    fn test_is_ori_symbol_valid() {
        assert!(is_ori_symbol("_ori_main"));
        assert!(is_ori_symbol("_ori_math$add"));
        assert!(is_ori_symbol("_ori_data$utils$process"));
        assert!(is_ori_symbol("_ori_int$$Eq$equals"));
    }

    #[test]
    fn test_is_ori_symbol_invalid() {
        // C++ mangled names
        assert!(!is_ori_symbol("_ZN3foo3barE"));
        assert!(!is_ori_symbol("_Z4funcPKc"));

        // Rust mangled names
        assert!(!is_ori_symbol("_RNvC4test4main"));

        // Plain C symbols
        assert!(!is_ori_symbol("printf"));
        assert!(!is_ori_symbol("main"));
        assert!(!is_ori_symbol("__libc_start_main"));

        // Empty and edge cases
        assert!(!is_ori_symbol(""));
        assert!(!is_ori_symbol("_ori")); // Missing underscore after _ori
    }

    #[test]
    fn test_demangle_simple_function() {
        // Ori-style: @function for root-level functions
        assert_eq!(demangle("_ori_main"), Some("@main".to_string()));
        assert_eq!(demangle("_ori_foo"), Some("@foo".to_string()));
        assert_eq!(demangle("_ori_bar_baz"), Some("@bar_baz".to_string()));
    }

    #[test]
    fn test_demangle_module_function() {
        // Ori-style: module.@function, nested: module/sub.@function
        assert_eq!(demangle("_ori_math$add"), Some("math.@add".to_string()));
        assert_eq!(
            demangle("_ori_std$io$read"),
            Some("std/io.@read".to_string())
        );
        assert_eq!(demangle("_ori_a$b$c$d"), Some("a/b/c.@d".to_string()));
    }

    #[test]
    fn test_demangle_trait_impl() {
        // Trait implementations: type::Trait.@method
        assert_eq!(
            demangle("_ori_int$$Eq$equals"),
            Some("int::Eq.@equals".to_string())
        );
        assert_eq!(
            demangle("_ori_Point$$Clone$clone"),
            Some("Point::Clone.@clone".to_string())
        );
    }

    #[test]
    fn test_demangle_non_ori_returns_none() {
        assert!(demangle("printf").is_none());
        assert!(demangle("_ZN3foo3barE").is_none());
        assert!(demangle("").is_none());
        assert!(demangle("_ori").is_none()); // Incomplete prefix
    }

    #[test]
    fn test_demangle_special_characters() {
        // Generic types with angle brackets
        let mangled = "_ori_Option$LTint$GT";
        let demangled = demangle(mangled);
        assert!(
            demangled.as_ref().is_some_and(|d| d.contains('<')),
            "generic type should demangle and contain '<'"
        );

        // Array types with square brackets
        let mangled = "_ori_$LBint$RB";
        let demangled = demangle(mangled);
        assert!(
            demangled.as_ref().is_some_and(|d| d.contains('[')),
            "array type should demangle and contain '['"
        );
    }

    #[test]
    fn test_demangle_associated_function() {
        let mangled = "_ori_Option$A$some";
        let demangled = demangle(mangled);
        assert!(
            demangled.as_ref().is_some_and(|d| d.contains('.')),
            "associated function should demangle and contain '.'"
        );
    }

    #[test]
    fn test_demangle_generic_instantiation() {
        // Generic marker $G
        let mangled = "_ori_identity$Gint";
        let demangled = demangle(mangled);
        assert!(
            demangled.as_ref().is_some_and(|d| d.contains('<')),
            "generic instantiation should demangle and contain '<'"
        );
    }
}
