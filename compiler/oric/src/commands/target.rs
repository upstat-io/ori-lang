//! The `target` command: manage cross-compilation targets.
//!
//! This module provides subcommands for managing cross-compilation sysroots:
//! - `ori target list` - List installed targets
//! - `ori target add <target>` - Install a target's sysroot
//! - `ori target remove <target>` - Remove a target's sysroot
//!
//! ## Canonicalization invariant
//!
//! Every target spelling accepted by the compiler (`ori build --target=...`)
//! must also be accepted by these commands (`ori target add/remove`), and
//! vice versa. Both sides route through the same canonical-key path
//! ([`TargetTripleComponents::support_key`]) so that aliased and versioned
//! spellings — `arm64-apple-darwin25.2.0`, `amd64-unknown-linux-gnu`, etc. —
//! resolve to the same logical target identity. The sysroot directory is
//! named with the canonical key, not the user's input, so there is one
//! sysroot per logical target with no per-OS-subversion duplicates.
//!
//! ## Install / discovery contract
//!
//! Sysroots are stored under `~/.ori/sysroots/<canonical-key>/` and the
//! WASI SDK is read from `~/.wasi-sdk/share/wasi-sysroot`. Both locations
//! are defined as the SSOT in [`ori_llvm::aot::syslib`] (via
//! [`ori_sysroot_path`] and [`home_wasi_sdk_sysroot`]) so the install side
//! and the build-time discovery side
//! ([`ori_llvm::aot::SysLibConfig::detect_sysroot`]) cannot drift.
//!
//! [`TargetTripleComponents::support_key`]: ori_llvm::aot::TargetTripleComponents::support_key
//! [`ori_sysroot_path`]: ori_llvm::aot::ori_sysroot_path
//! [`home_wasi_sdk_sysroot`]: ori_llvm::aot::home_wasi_sdk_sysroot

use std::fs;
use std::path::PathBuf;

#[cfg(feature = "llvm")]
use ori_llvm::aot::{
    is_supported_target, ori_sysroot_path, ori_sysroots_dir, target_sysroot_env_key, TargetError,
    TargetTripleComponents, SUPPORTED_TARGETS,
};

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
/// Thin wrapper around the SSOT in [`ori_llvm::aot::ori_sysroots_dir`] so
/// the install side and the build-time discovery side share one source of
/// truth for "where Ori puts managed sysroots".
#[cfg(feature = "llvm")]
fn sysroots_dir() -> PathBuf {
    ori_sysroots_dir()
}

/// Get the sysroot path for a specific target's canonical key.
///
/// Callers MUST pass the canonical [`TargetTripleComponents::support_key`]
/// — not the user's raw input — so that `arm64-apple-darwin25.2.0` and
/// `aarch64-apple-darwin25.2.0` resolve to the same on-disk directory.
///
/// [`TargetTripleComponents::support_key`]: ori_llvm::aot::TargetTripleComponents::support_key
#[cfg(feature = "llvm")]
fn sysroot_path(canonical_key: &str) -> PathBuf {
    ori_sysroot_path(canonical_key)
}

/// Canonicalize a user-supplied target spelling for use as the on-disk
/// sysroot directory name and the supported-targets lookup key.
///
/// Routes through the same [`TargetTripleComponents::support_key`] path
/// that `TargetConfig::from_triple` uses, so any spelling accepted by
/// `ori build --target=...` is also accepted here. Returns the canonical
/// key (the storage / lookup form, with Darwin OS version suffixes
/// stripped and arch aliases normalized) or a [`TargetError`] if the
/// input cannot be parsed or canonicalizes to an unsupported target.
///
/// This function is the SSOT for "what is the canonical name for this
/// user input". The CLI wrapper handles I/O and printing; this function
/// is pure and unit-testable.
///
/// [`TargetTripleComponents::support_key`]: ori_llvm::aot::TargetTripleComponents::support_key
#[cfg(feature = "llvm")]
fn canonicalize_target_for_install(user_input: &str) -> Result<String, TargetError> {
    let parsed = TargetTripleComponents::parse(user_input)?;
    let key = parsed.support_key();
    if !is_supported_target(&key) {
        return Err(TargetError::UnsupportedTarget {
            triple: user_input.to_string(),
            supported: SUPPORTED_TARGETS.to_vec(),
        });
    }
    Ok(key)
}

/// Check if a target's sysroot is installed.
///
/// Resolves the canonical key first so an alias query like
/// `is_target_installed("arm64-apple-darwin25.2.0")` finds the same
/// directory the canonical-name install created.
#[cfg(feature = "llvm")]
fn is_target_installed(target: &str) -> bool {
    let Ok(canonical) = canonicalize_target_for_install(target) else {
        return false;
    };
    let path = sysroot_path(&canonical);
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
///
/// The user-supplied `target` is routed through
/// [`canonicalize_target_for_install`] so any spelling accepted by
/// `ori build --target=...` (including arch aliases like
/// `arm64-apple-darwin` and Darwin OS version suffixes like
/// `arm64-apple-darwin25.2.0`) is also accepted here. The on-disk
/// sysroot directory is named with the canonical key — there is one
/// sysroot per logical target, no per-OS-subversion duplicates.
#[cfg(feature = "llvm")]
pub fn add_target(target: &str) {
    // Validate + canonicalize via the same path `from_triple` uses.
    let canonical = match canonicalize_target_for_install(target) {
        Ok(key) => key,
        Err(TargetError::UnsupportedTarget { supported, .. }) => {
            eprintln!("error: unsupported target '{target}'");
            eprintln!();
            eprintln!("Supported targets:");
            for t in &supported {
                eprintln!("  {t}");
            }
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("error: invalid target '{target}': {e}");
            std::process::exit(1);
        }
    };

    // Echo the canonical form when it differs from the user's input so
    // the user knows what's actually being installed.
    if canonical != target {
        println!("Canonicalizing '{target}' to '{canonical}'");
    }

    // Check if already installed (uses canonical key under the hood).
    if is_target_installed(&canonical) {
        println!("Target '{canonical}' is already installed.");
        return;
    }

    let sysroot = sysroot_path(&canonical);

    // For WASM targets, we can proceed without a full sysroot
    if canonical.starts_with("wasm32") {
        println!("Installing target '{canonical}'...");

        // Create the sysroot directory
        if let Err(e) = fs::create_dir_all(&sysroot) {
            eprintln!("error: failed to create sysroot directory: {e}");
            std::process::exit(1);
        }

        // For WASM, check for wasi-sdk if it's a WASI target. Use the
        // typed `is_wasi_target()` predicate so future Preview2 / canonical
        // alias variations don't bypass the check.
        if is_wasi_target(&canonical) {
            check_wasi_sdk(&sysroot);
        } else {
            // For standalone WASM, just create the marker
            let marker = sysroot.join(".ori-target");
            if let Err(e) = fs::write(&marker, format!("target={canonical}\n")) {
                eprintln!("warning: failed to create marker file: {e}");
            }
        }

        println!("Target '{canonical}' installed successfully.");
        println!();
        println!("You can now build for this target with:");
        println!("  ori build --target={canonical} <file.ori>");
        return;
    }

    // For native platform targets, we need to install a sysroot
    println!("Installing target '{canonical}'...");
    println!();

    // Try to detect or download sysroot. Pass the canonical key so the
    // existing-sysroot lookup is consistent with what we'll write.
    if let Some(existing) = detect_existing_sysroot(&canonical) {
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

        println!("Target '{canonical}' installed successfully.");
    } else {
        // No existing sysroot found
        eprintln!("error: could not find sysroot for target '{canonical}'");
        eprintln!();
        eprintln!("To cross-compile, you need to install the target's system libraries.");
        eprintln!();
        suggest_sysroot_installation(&canonical);
        std::process::exit(1);
    }
}

/// Run the `ori target add <target>` command when LLVM is not available.
#[cfg(not(feature = "llvm"))]
pub fn add_target(_target: &str) {
    eprintln!("error: the 'target add' command requires the LLVM backend");
    eprintln!();
    eprintln!("The Ori compiler was built without LLVM support.");
    eprintln!("To enable cross-compilation, reinstall Ori with LLVM support.");
    std::process::exit(1);
}

/// Run the `ori target remove <target>` command.
///
/// Routes through [`canonicalize_target_for_install`] so any spelling
/// accepted by `ori target add` (including arch aliases and Darwin OS
/// version suffixes) is also accepted here.
#[cfg(feature = "llvm")]
pub fn remove_target(target: &str) {
    // For invalid input, fall back to the raw target in the error message
    // so it accurately quotes what the user typed.
    let Ok(canonical) = canonicalize_target_for_install(target) else {
        eprintln!("error: target '{target}' is not installed");
        std::process::exit(1);
    };

    let sysroot = sysroot_path(&canonical);

    if !sysroot.exists() {
        eprintln!("error: target '{canonical}' is not installed");
        std::process::exit(1);
    }

    // Check if it's a symlink (to existing sysroot) or actual directory
    let is_symlink = sysroot.symlink_metadata().is_ok_and(|m| m.is_symlink());

    if canonical == target {
        println!("Removing target '{canonical}'...");
    } else {
        println!("Removing target '{target}' (canonical: {canonical})...");
    }

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

    println!("Target '{canonical}' removed successfully.");
}

/// `remove_target` shim for builds without LLVM. The CLI surface stays
/// the same so the dispatch code in `main.rs` doesn't need to be gated.
#[cfg(not(feature = "llvm"))]
pub fn remove_target(_target: &str) {
    eprintln!("error: the 'target remove' command requires the LLVM backend");
    std::process::exit(1);
}

/// Check for WASI SDK installation and set up sysroot.
#[cfg(feature = "llvm")]
fn check_wasi_sdk(sysroot: &std::path::Path) {
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
    let _ = fs::write(
        &marker,
        "target=wasm32-unknown-wasip1\nwasi_sdk=not_found\n",
    );
}

/// Predicate: is the given target spelling a WASI target?
///
/// Recognizes the modern Rust 1.78+ canonical spelling
/// (`wasm32-unknown-wasip1`). The historical 2-component `wasm32-wasi`
/// alias is no longer accepted — see BUG-04-045 / TPR-BUG-04-045-04 for
/// the deprecation rationale and Rust upstream's May 2024 rename.
#[cfg(feature = "llvm")]
fn is_wasi_target(target: &str) -> bool {
    target == "wasm32-unknown-wasip1"
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
    // Use the canonical SSOT helper so the documented env var matches
    // exactly what `SysLibConfig::detect_sysroot` looks up. Built from
    // `support_key()` so versioned Darwin spellings produce a shell-safe
    // key (no dots, no version suffix). See BUG-04-045 / TPR-BUG-04-045-07.
    let env_key = TargetTripleComponents::parse(target).map_or_else(
        |_| {
            // Fall back to the raw spelling if parsing fails — the
            // suggestion is best-effort guidance, not validation.
            format!("ORI_SYSROOT_{}", target.to_uppercase().replace('-', "_"))
        },
        |parsed| target_sysroot_env_key(&parsed),
    );
    println!("  export {env_key}=/path/to/sysroot");
}

#[cfg(test)]
mod tests;
