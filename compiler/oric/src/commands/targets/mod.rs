//! The `targets` command: list supported compilation targets.

/// Filter for which targets to display.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetFilter {
    /// Show all supported targets.
    All,
    /// Show only installed targets (those with sysroots available).
    InstalledOnly,
}

/// List all supported compilation targets.
///
/// With `InstalledOnly`, only shows targets that have sysroots available.
#[cfg(feature = "llvm")]
pub fn list_targets(filter: TargetFilter) {
    use ori_llvm::aot::SUPPORTED_TARGETS;

    if filter == TargetFilter::InstalledOnly {
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
pub fn list_targets(_filter: TargetFilter) {
    eprintln!("error: the 'targets' command requires the LLVM backend");
    eprintln!();
    eprintln!("The Ori compiler was built without LLVM support.");
    eprintln!("To enable target listing, rebuild with the 'llvm' feature:");
    eprintln!();
    eprintln!("  cargo build --features llvm");
    std::process::exit(1);
}

#[cfg(test)]
mod tests;
