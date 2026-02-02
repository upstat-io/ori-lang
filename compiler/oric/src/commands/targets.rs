//! The `targets` command: list supported compilation targets.

/// List all supported compilation targets.
///
/// With `--installed`, only shows targets that have sysroots available.
#[cfg(feature = "llvm")]
pub fn list_targets(installed_only: bool) {
    use ori_llvm::aot::SUPPORTED_TARGETS;

    if installed_only {
        // For now, we only support native target without explicit sysroot
        println!("Installed targets:");
        println!();

        // Check which targets have sysroots installed
        // For now, just show native target as installed
        if let Ok(native) = ori_llvm::aot::TargetConfig::native() {
            println!("  {} (native)", native.triple());
        }

        println!();
        println!("Use `ori target add <target>` to install additional target sysroots.");
    } else {
        println!("Supported targets:");
        println!();

        // Group targets by platform
        println!("  Linux:");
        for target in SUPPORTED_TARGETS {
            if target.contains("linux") {
                println!("    {target}");
            }
        }

        println!();
        println!("  macOS:");
        for target in SUPPORTED_TARGETS {
            if target.contains("darwin") {
                println!("    {target}");
            }
        }

        println!();
        println!("  Windows:");
        for target in SUPPORTED_TARGETS {
            if target.contains("windows") {
                println!("    {target}");
            }
        }

        println!();
        println!("  WebAssembly:");
        for target in SUPPORTED_TARGETS {
            if target.contains("wasm") {
                println!("    {target}");
            }
        }

        println!();
        println!("Use `ori build --target=<target>` to cross-compile.");
        println!("Use `ori targets --installed` to see targets with sysroots.");
    }
}

/// List targets when LLVM feature is not enabled.
#[cfg(not(feature = "llvm"))]
pub fn list_targets(_installed_only: bool) {
    eprintln!("error: the 'targets' command requires the LLVM backend");
    eprintln!();
    eprintln!("The Ori compiler was built without LLVM support.");
    eprintln!("To enable target listing, rebuild with the 'llvm' feature:");
    eprintln!();
    eprintln!("  cargo build --features llvm");
    std::process::exit(1);
}

#[cfg(test)]
mod tests {
    /// Test that SUPPORTED_TARGETS contains expected platform families.
    ///
    /// The actual `list_targets` function does I/O (prints to stdout), so we test
    /// the underlying data and logic that it depends on.
    #[cfg(feature = "llvm")]
    mod llvm_tests {
        use ori_llvm::aot::SUPPORTED_TARGETS;

        #[test]
        fn test_supported_targets_contains_linux() {
            let has_linux = SUPPORTED_TARGETS.iter().any(|t| t.contains("linux"));
            assert!(has_linux, "should have at least one Linux target");
        }

        #[test]
        fn test_supported_targets_contains_darwin() {
            let has_darwin = SUPPORTED_TARGETS.iter().any(|t| t.contains("darwin"));
            assert!(has_darwin, "should have at least one macOS target");
        }

        #[test]
        fn test_supported_targets_contains_windows() {
            let has_windows = SUPPORTED_TARGETS.iter().any(|t| t.contains("windows"));
            assert!(has_windows, "should have at least one Windows target");
        }

        #[test]
        fn test_supported_targets_contains_wasm() {
            let has_wasm = SUPPORTED_TARGETS.iter().any(|t| t.contains("wasm"));
            assert!(has_wasm, "should have at least one WebAssembly target");
        }

        #[test]
        fn test_supported_targets_triple_format() {
            // All targets should follow standard format: arch-vendor-os[-env] or arch-os (WASM)
            for target in SUPPORTED_TARGETS {
                let parts: Vec<&str> = target.split('-').collect();
                // WASM targets use 2-part format (wasm32-unknown-unknown, wasm32-wasi)
                // Standard targets use 3+ parts (arch-vendor-os[-env])
                let min_parts = if target.starts_with("wasm") { 2 } else { 3 };
                assert!(
                    parts.len() >= min_parts,
                    "target '{}' should have at least {} parts",
                    target,
                    min_parts
                );
            }
        }

        #[test]
        fn test_supported_targets_not_empty() {
            assert!(
                !SUPPORTED_TARGETS.is_empty(),
                "should have at least one supported target"
            );
        }

        #[test]
        fn test_supported_targets_unique() {
            let mut seen = std::collections::HashSet::new();
            for target in SUPPORTED_TARGETS {
                assert!(
                    seen.insert(target),
                    "duplicate target '{}' in SUPPORTED_TARGETS",
                    target
                );
            }
        }

        #[test]
        fn test_native_target_available() {
            // Native target should be detectable
            let result = ori_llvm::aot::TargetConfig::native();
            assert!(
                result.is_ok(),
                "native target should be available: {:?}",
                result.err()
            );
        }
    }
}
