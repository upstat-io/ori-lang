//! Microsoft Visual C++ linker implementation.
//!
//! This module provides the `MsvcLinker` for Windows systems using
//! MSVC's `link.exe` or LLD in MSVC compatibility mode.

use std::path::Path;
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
    pub fn new(target: &TargetConfig) -> Self {
        Self {
            cmd: Command::new("link.exe"),
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
                // Default for link.exe
                self.cmd.arg("/SUBSYSTEM:CONSOLE");
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
