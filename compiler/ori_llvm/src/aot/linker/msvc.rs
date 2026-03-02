//! Microsoft Visual C++ linker implementation.
//!
//! This module provides the `MsvcLinker` for Windows systems using
//! MSVC's `link.exe` or LLD in MSVC compatibility mode.
//!
//! # MSVC Discovery
//!
//! On Windows, MinGW's `link` (a hard-link coreutil) can shadow MSVC's
//! `link.exe` on PATH. [`find_msvc_toolchain`] resolves this by locating
//! MSVC's linker through Visual Studio installation discovery:
//!
//! 1. `VCToolsInstallDir` env var (set by VS dev prompt / `ilammy/msvc-dev-cmd`)
//! 2. `vswhere.exe` (installed with VS Build Tools)
//! 3. Well-known Visual Studio installation paths
//! 4. Bare `link.exe` from PATH (last resort)

use std::path::{Path, PathBuf};
use std::process::Command;

use super::{LibraryKind, LinkOutput};
use crate::aot::TargetConfig;

/// Microsoft Visual C++ linker implementation.
///
/// Uses `link.exe` directly (not via a compiler wrapper).
pub struct MsvcLinker {
    cmd: Command,
    target: TargetConfig,
}

impl MsvcLinker {
    /// Create a new MSVC linker.
    ///
    /// Locates MSVC's `link.exe` via Visual Studio installation discovery,
    /// falling back to bare PATH lookup only as a last resort. When running
    /// outside a VS Developer Command Prompt (i.e. `LIB` is not set), also
    /// discovers MSVC and Windows SDK library paths and injects them into
    /// the linker command's environment.
    pub fn new(target: &TargetConfig) -> Self {
        let (link_exe, lib_paths) = match find_msvc_toolchain() {
            Some(tc) => {
                let paths = if std::env::var_os("LIB").is_none() {
                    tc.lib_paths()
                } else {
                    Vec::new()
                };
                (tc.link_exe, paths)
            }
            None => (PathBuf::from("link.exe"), Vec::new()),
        };

        let mut cmd = Command::new(link_exe);

        // When not in a dev prompt, provide library paths so the linker
        // can find CRT and Windows SDK libraries. We use /LIBPATH: args
        // rather than the LIB env var because LinkerDriver::run_linker()
        // reconstructs the Command (copying only program+args, not env).
        for path in &lib_paths {
            cmd.arg(format!("/LIBPATH:{}", path.display()));
        }

        Self {
            cmd,
            target: target.clone(),
        }
    }

    /// Create a new linker using LLD in MSVC compatibility mode.
    pub fn with_lld(target: &TargetConfig) -> Self {
        let mut cmd = Command::new("lld-link");
        // LLD-link is MSVC-compatible by default
        cmd.arg("/nologo");

        Self {
            cmd,
            target: target.clone(),
        }
    }
}

impl MsvcLinker {
    /// Get the command being built.
    pub fn cmd(&mut self) -> &mut Command {
        &mut self.cmd
    }

    /// Get the target configuration.
    pub fn target(&self) -> &TargetConfig {
        &self.target
    }

    /// Set the output file.
    pub fn set_output(&mut self, path: &Path) {
        self.cmd.arg(format!("/OUT:{}", path.display()));
    }

    /// Set the output kind (executable, shared library, etc.).
    pub fn set_output_kind(&mut self, kind: LinkOutput) {
        match kind {
            LinkOutput::Executable | LinkOutput::PositionIndependentExecutable => {
                self.cmd.arg("/SUBSYSTEM:CONSOLE");
                // LLVM-emitted objects don't embed /DEFAULTLIB directives (unlike
                // cl.exe), so we must explicitly request the MSVC C runtime.
                // The linker finds these via the LIB environment variable.
                self.cmd.arg("/DEFAULTLIB:libcmt");
                self.cmd.arg("/DEFAULTLIB:libvcruntime");
                self.cmd.arg("/DEFAULTLIB:libucrt");
            }
            LinkOutput::SharedLibrary => {
                self.cmd.arg("/DLL");
            }
            LinkOutput::StaticLibrary => {
                // Use lib.exe instead - handled by driver
            }
        }
    }

    /// Add an object file to link.
    pub fn add_object(&mut self, path: &Path) {
        self.cmd.arg(path);
    }

    /// Add a library search path.
    pub fn add_library_path(&mut self, path: &Path) {
        self.cmd.arg(format!("/LIBPATH:{}", path.display()));
    }

    /// Link a library by name.
    pub fn link_library(&mut self, name: &str, _kind: LibraryKind) {
        // MSVC doesn't distinguish static/dynamic at link time the same way
        // The library extension (.lib) determines this
        self.cmd.arg(format!("{name}.lib"));
    }

    /// Enable garbage collection of unused sections.
    pub fn gc_sections(&mut self, enable: bool) {
        if enable {
            self.cmd.arg("/OPT:REF"); // Remove unreferenced functions
            self.cmd.arg("/OPT:ICF"); // Identical COMDAT folding
        }
    }

    /// Strip debug symbols from output.
    pub fn strip_symbols(&mut self, strip: bool) {
        if strip {
            self.cmd.arg("/DEBUG:NONE");
        }
    }

    /// Add symbols to export (for shared libraries).
    pub fn export_symbols(&mut self, symbols: &[String]) {
        for sym in symbols {
            self.cmd.arg(format!("/EXPORT:{sym}"));
        }
    }

    /// Add a raw argument to the linker command.
    pub fn add_arg(&mut self, arg: &str) {
        self.cmd.arg(arg);
    }

    /// Add a linker-specific argument (MSVC takes args directly).
    pub fn link_arg(&mut self, arg: &str) {
        self.cmd.arg(arg);
    }

    /// Finalize and get the command to execute.
    pub fn finalize(self) -> Command {
        self.cmd
    }
}

// MSVC Toolchain Discovery

/// Discovered MSVC toolchain paths.
///
/// Contains the `link.exe` path and enough context to derive library
/// search paths when `LIB` is not already set by a VS Developer prompt.
pub(super) struct MsvcToolchain {
    /// Full path to MSVC's `link.exe`.
    pub link_exe: PathBuf,
    /// Versioned VC Tools directory (e.g. `VC/Tools/MSVC/14.44.35207/`).
    /// Used to derive the CRT library path (`lib/x64/`).
    vc_tools_dir: Option<PathBuf>,
}

impl MsvcToolchain {
    /// Derive MSVC + Windows SDK library paths from the discovered toolchain.
    ///
    /// Returns paths for:
    /// - MSVC CRT: `$vc_tools_dir/lib/<arch>/` (libcmt, libvcruntime)
    /// - Windows SDK um: `Windows Kits/10/Lib/<ver>/um/<arch>/` (kernel32, etc.)
    /// - Windows SDK ucrt: `Windows Kits/10/Lib/<ver>/ucrt/<arch>/` (ucrt)
    fn lib_paths(&self) -> Vec<PathBuf> {
        let arch = msvc_lib_arch();
        let mut paths = Vec::new();

        // MSVC CRT libraries
        if let Some(ref vc_dir) = self.vc_tools_dir {
            let crt_path = vc_dir.join("lib").join(arch);
            if crt_path.exists() {
                paths.push(crt_path);
            }
        }

        // Windows SDK libraries (um + ucrt)
        if let Some(sdk_lib) = find_windows_sdk_lib_dir() {
            let um = sdk_lib.join("um").join(arch);
            if um.exists() {
                paths.push(um);
            }
            let ucrt = sdk_lib.join("ucrt").join(arch);
            if ucrt.exists() {
                paths.push(ucrt);
            }
        }

        paths
    }
}

/// Host-architecture subdirectory components for MSVC tool binaries.
///
/// Returns `(host_dir, target_dir)` — e.g. `("Hostx64", "x64")`.
fn msvc_host_target_dirs() -> (&'static str, &'static str) {
    if cfg!(target_arch = "x86_64") {
        ("Hostx64", "x64")
    } else if cfg!(target_arch = "aarch64") {
        ("Hostarm64", "arm64")
    } else {
        ("Hostx86", "x86")
    }
}

/// Library subdirectory name for the current host architecture.
fn msvc_lib_arch() -> &'static str {
    if cfg!(target_arch = "x86_64") {
        "x64"
    } else if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        "x86"
    }
}

/// Discover the MSVC toolchain (link.exe + VC tools directory).
///
/// Checks, in order:
/// 1. `VCToolsInstallDir` env var (set by VS dev prompt / `ilammy/msvc-dev-cmd`)
/// 2. `vswhere.exe` — standard VS installation discovery tool
/// 3. Well-known Visual Studio installation paths
///
/// Returns `None` if MSVC cannot be found through any discovery method.
pub(super) fn find_msvc_toolchain() -> Option<MsvcToolchain> {
    let (host_dir, target_dir) = msvc_host_target_dirs();

    // 1. VCToolsInstallDir — set by VS Developer Command Prompt and ilammy/msvc-dev-cmd
    if let Ok(vc_tools) = std::env::var("VCToolsInstallDir") {
        let vc_dir = PathBuf::from(&vc_tools);
        let link = vc_dir
            .join("bin")
            .join(host_dir)
            .join(target_dir)
            .join("link.exe");
        if link.exists() {
            return Some(MsvcToolchain {
                link_exe: link,
                vc_tools_dir: Some(vc_dir),
            });
        }
    }

    // 2. vswhere.exe — standard VS installation discovery tool
    if let Some(tc) = find_toolchain_via_vswhere(host_dir, target_dir) {
        return Some(tc);
    }

    // 3. Well-known installation paths
    find_toolchain_in_known_paths(host_dir, target_dir)
}

/// Find the MSVC toolchain using `vswhere.exe`.
fn find_toolchain_via_vswhere(host_dir: &str, target_dir: &str) -> Option<MsvcToolchain> {
    let vswhere =
        PathBuf::from(r"C:\Program Files (x86)\Microsoft Visual Studio\Installer\vswhere.exe");
    if !vswhere.exists() {
        return None;
    }

    let output = Command::new(&vswhere)
        .args([
            "-latest",
            "-requires",
            "Microsoft.VisualStudio.Component.VC.Tools.x86.x64",
            "-property",
            "installationPath",
        ])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let vs_path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if vs_path.is_empty() {
        return None;
    }

    find_latest_toolchain_in_vc_tools(
        &PathBuf::from(&vs_path).join(r"VC\Tools\MSVC"),
        host_dir,
        target_dir,
    )
}

/// Scan well-known Visual Studio installation paths for the MSVC toolchain.
fn find_toolchain_in_known_paths(host_dir: &str, target_dir: &str) -> Option<MsvcToolchain> {
    let base_dirs = [
        r"C:\Program Files\Microsoft Visual Studio",
        r"C:\Program Files (x86)\Microsoft Visual Studio",
    ];
    let years = ["2022", "2019"];
    let editions = ["BuildTools", "Enterprise", "Professional", "Community"];

    for base in &base_dirs {
        for year in &years {
            for edition in &editions {
                let vc_tools = PathBuf::from(base)
                    .join(year)
                    .join(edition)
                    .join(r"VC\Tools\MSVC");
                if let Some(tc) =
                    find_latest_toolchain_in_vc_tools(&vc_tools, host_dir, target_dir)
                {
                    return Some(tc);
                }
            }
        }
    }

    None
}

/// Find `link.exe` in the latest MSVC toolset version under `vc_tools_base`.
///
/// MSVC toolsets are versioned directories (e.g. `14.44.35207/`) under
/// `VC/Tools/MSVC/`. We sort descending to prefer the latest version.
fn find_latest_toolchain_in_vc_tools(
    vc_tools_base: &Path,
    host_dir: &str,
    target_dir: &str,
) -> Option<MsvcToolchain> {
    let mut versions: Vec<_> = std::fs::read_dir(vc_tools_base)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map_or(false, |ft| ft.is_dir()))
        .map(|e| e.path())
        .collect();

    // Sort descending — latest toolset version first
    versions.sort_unstable_by(|a, b| b.cmp(a));

    for version_dir in versions {
        let link = version_dir
            .join("bin")
            .join(host_dir)
            .join(target_dir)
            .join("link.exe");
        if link.exists() {
            return Some(MsvcToolchain {
                link_exe: link,
                vc_tools_dir: Some(version_dir),
            });
        }
    }

    None
}

// Windows SDK Discovery

/// Find the Windows SDK library directory (versioned).
///
/// Checks:
/// 1. `WindowsSdkDir` + `WindowsSDKVersion` env vars (VS dev prompt)
/// 2. Well-known Windows Kits path with latest version
///
/// Returns a path like `C:\Program Files (x86)\Windows Kits\10\Lib\10.0.26100.0\`.
fn find_windows_sdk_lib_dir() -> Option<PathBuf> {
    // 1. Environment variables (set by VS dev prompt / ilammy/msvc-dev-cmd)
    if let (Ok(sdk_dir), Ok(sdk_ver)) =
        (std::env::var("WindowsSdkDir"), std::env::var("WindowsSDKVersion"))
    {
        // WindowsSDKVersion often has a trailing backslash
        let ver = sdk_ver.trim_end_matches('\\');
        let path = PathBuf::from(&sdk_dir).join("Lib").join(ver);
        if path.exists() {
            return Some(path);
        }
    }

    // 2. Well-known Windows Kits location, latest version
    let kits_lib = PathBuf::from(r"C:\Program Files (x86)\Windows Kits\10\Lib");
    latest_version_dir(&kits_lib)
}

/// Find the latest versioned subdirectory (e.g. `10.0.26100.0/`).
fn latest_version_dir(base: &Path) -> Option<PathBuf> {
    let mut versions: Vec<_> = std::fs::read_dir(base)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map_or(false, |ft| ft.is_dir()))
        .map(|e| e.path())
        .collect();

    // Sort descending — latest version first
    versions.sort_unstable_by(|a, b| b.cmp(a));
    versions.into_iter().next()
}
