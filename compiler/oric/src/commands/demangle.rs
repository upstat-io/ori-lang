//! The `demangle` command: decode mangled Ori symbol names.

/// Demangle an Ori symbol name.
///
/// Takes a mangled symbol like `_ori_MyModule_foo` and outputs
/// the demangled form like `MyModule.@foo`.
#[cfg(feature = "llvm")]
pub fn demangle_symbol(symbol: &str) {
    use ori_llvm::aot::{demangle, is_ori_symbol};

    if !is_ori_symbol(symbol) {
        // Not an Ori symbol, print as-is
        println!("{symbol}");
        return;
    }

    match demangle(symbol) {
        Some(demangled) => println!("{demangled}"),
        None => {
            // Couldn't demangle, print original
            println!("{symbol}");
        }
    }
}

/// Demangle when LLVM feature is not enabled.
#[cfg(not(feature = "llvm"))]
pub fn demangle_symbol(_symbol: &str) {
    eprintln!("error: the 'demangle' command requires the LLVM backend");
    eprintln!();
    eprintln!("The Ori compiler was built without LLVM support.");
    eprintln!("To enable demangling, rebuild with the 'llvm' feature:");
    eprintln!();
    eprintln!("  cargo build --features llvm");
    std::process::exit(1);
}

#[cfg(test)]
mod tests {
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
            assert!(demangled.is_some());
            assert!(demangled.as_ref().unwrap().contains('<'));

            // Array types with square brackets
            let mangled = "_ori_$LBint$RB";
            let demangled = demangle(mangled);
            assert!(demangled.is_some());
            assert!(demangled.as_ref().unwrap().contains('['));
        }

        #[test]
        fn test_demangle_associated_function() {
            let mangled = "_ori_Option$A$some";
            let demangled = demangle(mangled);
            assert!(demangled.is_some());
            assert!(demangled.as_ref().unwrap().contains('.'));
        }

        #[test]
        fn test_demangle_generic_instantiation() {
            // Generic marker $G
            let mangled = "_ori_identity$Gint";
            let demangled = demangle(mangled);
            assert!(demangled.is_some());
            assert!(demangled.unwrap().contains('<'));
        }
    }
}
