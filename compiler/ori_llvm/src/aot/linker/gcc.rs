//! GNU/Clang-style linker implementation.
//!
//! This module provides the `GccLinker` for Unix-like systems (Linux, macOS)
//! that use GCC or Clang as the linker driver.

use std::path::Path;
use std::process::Command;

use super::{LibraryKind, LinkOutput};
use crate::aot::TargetConfig;

/// GNU/Clang-style linker implementation.
///
/// Uses `cc` or `clang` as a wrapper, which handles:
/// - CRT object linking (crt1.o, crti.o, etc.)
/// - Standard library linking
/// - Proper argument ordering
pub struct GccLinker {
    cmd: Command,
    target: TargetConfig,
    /// Track current static/dynamic mode for hint optimization.
    hint_static: bool,
}

impl GccLinker {
    /// Create a new GCC-style linker.
    pub fn new(target: &TargetConfig) -> Self {
        // Use clang on macOS, cc otherwise
        let linker = if target.is_macos() { "clang" } else { "cc" };
        let cmd = Command::new(linker);

        Self {
            cmd,
            target: target.clone(),
            hint_static: false,
        }
    }

    /// Create a new linker using a specific compiler/linker path.
    pub fn with_path(target: &TargetConfig, path: &str) -> Self {
        Self {
            cmd: Command::new(path),
            target: target.clone(),
            hint_static: false,
        }
    }

    /// Switch to static linking mode.
    fn hint_static(&mut self) {
        if !self.hint_static {
            self.hint_static = true;
            if !self.target.is_macos() {
                // macOS doesn't support -Bstatic
                self.cmd.arg("-Wl,-Bstatic");
            }
        }
    }

    /// Switch to dynamic linking mode.
    fn hint_dynamic(&mut self) {
        if self.hint_static {
            self.hint_static = false;
            if !self.target.is_macos() {
                self.cmd.arg("-Wl,-Bdynamic");
            }
        }
    }
}

impl GccLinker {
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
        self.cmd.arg("-o").arg(path);
    }

    /// Set the output kind (executable, shared library, etc.).
    pub fn set_output_kind(&mut self, kind: LinkOutput) {
        match kind {
            LinkOutput::SharedLibrary => {
                self.cmd.arg("-shared");
                if self.target.is_macos() {
                    // macOS uses -dynamiclib
                    self.cmd.arg("-dynamiclib");
                } else {
                    self.cmd.arg("-fPIC");
                }
            }
            LinkOutput::PositionIndependentExecutable => {
                self.cmd.arg("-pie");
                self.cmd.arg("-fPIE");
            }
            // Executable: Default, no special flags needed
            // StaticLibrary: Created with ar, not the linker (handled specially by driver)
            LinkOutput::Executable | LinkOutput::StaticLibrary => {}
        }
    }

    /// Add an object file to link.
    pub fn add_object(&mut self, path: &Path) {
        self.cmd.arg(path);
    }

    /// Add a library search path.
    pub fn add_library_path(&mut self, path: &Path) {
        self.cmd.arg("-L").arg(path);
    }

    /// Link a library by name.
    pub fn link_library(&mut self, name: &str, kind: LibraryKind) {
        match kind {
            LibraryKind::Unspecified => {
                // Let the linker decide
                self.cmd.arg(format!("-l{name}"));
            }
            LibraryKind::Static => {
                if self.target.is_macos() {
                    // macOS: use -l with full path or -force_load
                    // For now, just use -l and hope for the best
                    // A more robust solution would search for the .a file
                    self.cmd.arg(format!("-l{name}"));
                } else {
                    self.hint_static();
                    self.cmd.arg(format!("-l{name}"));
                    self.hint_dynamic(); // Reset to dynamic for subsequent libs
                }
            }
            LibraryKind::Dynamic => {
                self.hint_dynamic();
                self.cmd.arg(format!("-l{name}"));
            }
        }
    }

    /// Enable garbage collection of unused sections.
    pub fn gc_sections(&mut self, enable: bool) {
        if enable {
            if self.target.is_macos() {
                self.cmd.arg("-Wl,-dead_strip");
            } else {
                self.cmd.arg("-Wl,--gc-sections");
            }
        }
    }

    /// Strip debug symbols from output.
    pub fn strip_symbols(&mut self, strip: bool) {
        if strip {
            if self.target.is_macos() {
                self.cmd.arg("-Wl,-S"); // Strip debug symbols only
            } else {
                self.cmd.arg("-Wl,--strip-all");
            }
        }
    }

    /// Add symbols to export (for shared libraries).
    pub fn export_symbols(&mut self, symbols: &[String]) {
        if symbols.is_empty() {
            return;
        }

        if self.target.is_macos() {
            // macOS: use -exported_symbols_list
            // For simplicity, we add individual -exported_symbol flags
            for sym in symbols {
                self.cmd.arg(format!("-Wl,-exported_symbol,_{sym}"));
            }
        } else {
            // Linux: use --export-dynamic for all, or version script for specific
            // For simplicity, export all dynamic symbols
            self.cmd.arg("-Wl,--export-dynamic");
        }
    }

    /// Add a raw argument to the linker command.
    pub fn add_arg(&mut self, arg: &str) {
        self.cmd.arg(arg);
    }

    /// Add a linker-specific argument (wrapped in -Wl for cc wrapper).
    pub fn link_arg(&mut self, arg: &str) {
        self.cmd.arg(format!("-Wl,{arg}"));
    }

    /// Finalize and get the command to execute.
    pub fn finalize(self) -> Command {
        self.cmd
    }
}
