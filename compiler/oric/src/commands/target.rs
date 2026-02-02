//! The `target` command: manage cross-compilation targets.
//!
//! This module provides subcommands for managing cross-compilation sysroots:
//! - `ori target list` - List installed targets
//! - `ori target add <target>` - Install a target's sysroot
//! - `ori target remove <target>` - Remove a target's sysroot

use std::fs;
use std::path::PathBuf;

/// Subcommand for the `target` command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetSubcommand {
    /// List installed targets.
    List,
    /// Add a target's sysroot.
    Add,
    /// Remove a target's sysroot.
    Remove,
}

impl TargetSubcommand {
    /// Parse a subcommand from a string.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "list" => Some(Self::List),
            "add" => Some(Self::Add),
            "remove" => Some(Self::Remove),
            _ => None,
        }
    }
}

/// Get the sysroots directory path.
///
/// Sysroots are stored in `~/.ori/sysroots/<target>/`.
fn sysroots_dir() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".ori").join("sysroots")
}

/// Get the sysroot path for a specific target.
fn sysroot_path(target: &str) -> PathBuf {
    sysroots_dir().join(target)
}

/// Check if a target's sysroot is installed.
#[cfg(feature = "llvm")]
fn is_target_installed(target: &str) -> bool {
    let path = sysroot_path(target);
    path.exists() && path.is_dir()
}

/// Run the `ori target list` command.
///
/// Lists all installed cross-compilation targets.
pub fn list_installed_targets() {
    let sysroots = sysroots_dir();

    println!("Installed targets:");
    println!();

    // Always show native target
    #[cfg(feature = "llvm")]
    {
        if let Ok(native) = ori_llvm::aot::TargetConfig::native() {
            println!("  {} (native)", native.triple());
        }
    }

    #[cfg(not(feature = "llvm"))]
    {
        // Without LLVM, just show a generic native target
        #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
        println!("  x86_64-unknown-linux-gnu (native)");
        #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
        println!("  aarch64-unknown-linux-gnu (native)");
        #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
        println!("  x86_64-apple-darwin (native)");
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        println!("  aarch64-apple-darwin (native)");
        #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
        println!("  x86_64-pc-windows-msvc (native)");
    }

    // List installed sysroots
    if sysroots.exists() {
        if let Ok(entries) = fs::read_dir(&sysroots) {
            for entry in entries.flatten() {
                if entry.path().is_dir() {
                    if let Some(name) = entry.file_name().to_str() {
                        println!("  {name}");
                    }
                }
            }
        }
    }

    println!();
    println!("Use `ori target add <target>` to install additional targets.");
    println!("Use `ori targets` to see all supported targets.");
}

/// Run the `ori target add <target>` command.
///
/// Downloads and installs a cross-compilation sysroot for the given target.
#[cfg(feature = "llvm")]
pub fn add_target(target: &str) {
    use ori_llvm::aot::SUPPORTED_TARGETS;

    // Validate target
    if !SUPPORTED_TARGETS.contains(&target) {
        eprintln!("error: unsupported target '{target}'");
        eprintln!();
        eprintln!("Supported targets:");
        for t in SUPPORTED_TARGETS {
            eprintln!("  {t}");
        }
        std::process::exit(1);
    }

    // Check if already installed
    if is_target_installed(target) {
        println!("Target '{target}' is already installed.");
        return;
    }

    let sysroot = sysroot_path(target);

    // For WASM targets, we can proceed without a full sysroot
    if target.starts_with("wasm32") {
        println!("Installing target '{target}'...");

        // Create the sysroot directory
        if let Err(e) = fs::create_dir_all(&sysroot) {
            eprintln!("error: failed to create sysroot directory: {e}");
            std::process::exit(1);
        }

        // For WASM, check for wasi-sdk if it's a WASI target
        if target == "wasm32-wasi" {
            check_wasi_sdk(&sysroot);
        } else {
            // For standalone WASM, just create the marker
            let marker = sysroot.join(".ori-target");
            if let Err(e) = fs::write(&marker, format!("target={target}\n")) {
                eprintln!("warning: failed to create marker file: {e}");
            }
        }

        println!("Target '{target}' installed successfully.");
        println!();
        println!("You can now build for this target with:");
        println!("  ori build --target={target} <file.ori>");
        return;
    }

    // For native platform targets, we need to install a sysroot
    println!("Installing target '{target}'...");
    println!();

    // Try to detect or download sysroot
    if let Some(existing) = detect_existing_sysroot(target) {
        // Found an existing sysroot, create a symlink
        println!("Found existing sysroot at: {}", existing.display());

        // Create parent directory
        if let Some(parent) = sysroot.parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                eprintln!("error: failed to create directory: {e}");
                std::process::exit(1);
            }
        }

        // Create symlink to existing sysroot
        #[cfg(unix)]
        {
            if let Err(e) = std::os::unix::fs::symlink(&existing, &sysroot) {
                eprintln!("error: failed to create symlink: {e}");
                std::process::exit(1);
            }
        }

        #[cfg(windows)]
        {
            if let Err(e) = std::os::windows::fs::symlink_dir(&existing, &sysroot) {
                eprintln!("error: failed to create symlink: {e}");
                std::process::exit(1);
            }
        }

        println!("Target '{target}' installed successfully.");
    } else {
        // No existing sysroot found
        eprintln!("error: could not find sysroot for target '{target}'");
        eprintln!();
        eprintln!("To cross-compile, you need to install the target's system libraries.");
        eprintln!();
        suggest_sysroot_installation(target);
        std::process::exit(1);
    }
}

/// Run the `ori target add <target>` command when LLVM is not available.
#[cfg(not(feature = "llvm"))]
pub fn add_target(_target: &str) {
    eprintln!("error: the 'target add' command requires the LLVM backend");
    eprintln!();
    eprintln!("The Ori compiler was built without LLVM support.");
    eprintln!("To enable cross-compilation, rebuild with the 'llvm' feature:");
    eprintln!();
    eprintln!("  cargo build --features llvm");
    std::process::exit(1);
}

/// Run the `ori target remove <target>` command.
pub fn remove_target(target: &str) {
    let sysroot = sysroot_path(target);

    if !sysroot.exists() {
        eprintln!("error: target '{target}' is not installed");
        std::process::exit(1);
    }

    // Check if it's a symlink (to existing sysroot) or actual directory
    let is_symlink = sysroot.symlink_metadata().is_ok_and(|m| m.is_symlink());

    println!("Removing target '{target}'...");

    if is_symlink {
        // Just remove the symlink
        if let Err(e) = fs::remove_file(&sysroot) {
            eprintln!("error: failed to remove symlink: {e}");
            std::process::exit(1);
        }
    } else {
        // Remove the entire directory
        if let Err(e) = fs::remove_dir_all(&sysroot) {
            eprintln!("error: failed to remove sysroot: {e}");
            std::process::exit(1);
        }
    }

    println!("Target '{target}' removed successfully.");
}

/// Check for WASI SDK installation and set up sysroot.
#[cfg(feature = "llvm")]
fn check_wasi_sdk(sysroot: &PathBuf) {
    // Get home directory
    let home_wasi_sdk = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(|h| PathBuf::from(h).join(".wasi-sdk"))
        .unwrap_or_default();

    // Common WASI SDK locations
    let wasi_sdk_paths = [
        PathBuf::from("/opt/wasi-sdk"),
        PathBuf::from("/usr/local/wasi-sdk"),
        home_wasi_sdk,
    ];

    for sdk_path in &wasi_sdk_paths {
        let wasi_sysroot = sdk_path.join("share/wasi-sysroot");
        if wasi_sysroot.exists() {
            println!("Found WASI SDK at: {}", sdk_path.display());

            // Create symlink to WASI sysroot
            #[cfg(unix)]
            {
                if std::os::unix::fs::symlink(&wasi_sysroot, sysroot).is_err() {
                    // If symlink fails (e.g., directory already created), try to remove and retry
                    let _ = fs::remove_dir(sysroot);
                    if let Err(e) = std::os::unix::fs::symlink(&wasi_sysroot, sysroot) {
                        eprintln!("warning: failed to create symlink: {e}");
                    }
                }
            }

            return;
        }
    }

    // WASI SDK not found, create marker for minimal WASM support
    eprintln!("warning: WASI SDK not found. WASI imports may not link correctly.");
    eprintln!();
    eprintln!("To enable full WASI support, install the WASI SDK:");
    eprintln!("  https://github.com/WebAssembly/wasi-sdk");
    eprintln!();

    let marker = sysroot.join(".ori-target");
    let _ = fs::write(&marker, "target=wasm32-wasi\nwasi_sdk=not_found\n");
}

/// Detect an existing sysroot for a target.
#[cfg(feature = "llvm")]
fn detect_existing_sysroot(target: &str) -> Option<PathBuf> {
    use ori_llvm::aot::TargetTripleComponents;

    let components = TargetTripleComponents::parse(target).ok()?;
    let config = ori_llvm::aot::SysLibConfig::for_target(&components).ok()?;

    config.sysroot().cloned()
}

/// Print suggestions for installing a sysroot.
#[cfg(feature = "llvm")]
fn suggest_sysroot_installation(target: &str) {
    if target.contains("linux") {
        if target.contains("musl") {
            println!("For musl targets, install the musl toolchain:");
            println!("  # Debian/Ubuntu");
            println!("  apt install musl-dev musl-tools");
            println!();
            println!("  # Or download from: https://musl.libc.org/");
        } else {
            println!("For Linux glibc targets, install cross-compilation tools:");
            println!("  # Debian/Ubuntu (for aarch64)");
            println!("  apt install gcc-aarch64-linux-gnu");
            println!();
            println!("  # Or use a distribution's cross-compilation packages");
        }
    } else if target.contains("darwin") {
        println!("For macOS targets, you need:");
        println!("  - macOS SDK from Xcode");
        println!("  - Or use osxcross: https://github.com/tpoechtrager/osxcross");
    } else if target.contains("windows") {
        println!("For Windows targets from Linux/macOS:");
        println!("  - Install mingw-w64 for GNU targets");
        println!("  - Or use cross-compilation tools");
        println!();
        println!("  # Debian/Ubuntu");
        println!("  apt install mingw-w64");
    }

    println!();
    println!("After installing, set the sysroot path:");
    println!(
        "  export ORI_SYSROOT_{}=/path/to/sysroot",
        target.to_uppercase().replace('-', "_")
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_target_subcommand_from_str() {
        assert_eq!(
            TargetSubcommand::parse("list"),
            Some(TargetSubcommand::List)
        );
        assert_eq!(TargetSubcommand::parse("add"), Some(TargetSubcommand::Add));
        assert_eq!(
            TargetSubcommand::parse("remove"),
            Some(TargetSubcommand::Remove)
        );
        assert_eq!(TargetSubcommand::parse("invalid"), None);
        assert_eq!(TargetSubcommand::parse(""), None);
    }

    #[test]
    fn test_sysroots_dir() {
        let dir = sysroots_dir();
        assert!(dir.to_string_lossy().contains(".ori"));
        assert!(dir.to_string_lossy().contains("sysroots"));
    }

    #[test]
    fn test_sysroot_path() {
        let path = sysroot_path("x86_64-unknown-linux-gnu");
        assert!(path.to_string_lossy().contains("x86_64-unknown-linux-gnu"));
    }

    #[test]
    fn test_is_target_installed_nonexistent() {
        // A random target name that definitely doesn't exist
        // Test the underlying logic since is_target_installed is feature-gated
        let path = sysroot_path("nonexistent-fake-target-12345");
        assert!(!path.exists());
    }

    #[test]
    fn test_target_subcommand_variants() {
        // Verify all variants can be compared
        assert_ne!(TargetSubcommand::List, TargetSubcommand::Add);
        assert_ne!(TargetSubcommand::Add, TargetSubcommand::Remove);
        assert_ne!(TargetSubcommand::Remove, TargetSubcommand::List);
    }

    #[test]
    fn test_target_subcommand_debug() {
        // Verify Debug trait works
        let list = TargetSubcommand::List;
        let debug_str = format!("{list:?}");
        assert_eq!(debug_str, "List");
    }

    #[test]
    fn test_target_subcommand_clone() {
        let original = TargetSubcommand::Add;
        let cloned = original;
        assert_eq!(original, cloned);
    }
}
