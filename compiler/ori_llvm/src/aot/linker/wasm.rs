//! WebAssembly linker implementation.
//!
//! This module provides the `WasmLinker` for WebAssembly targets
//! using `wasm-ld` from the LLVM toolchain.

use std::path::Path;
use std::process::Command;

use super::{LibraryKind, LinkOutput};
use crate::aot::TargetConfig;

/// WebAssembly linker implementation using wasm-ld.
pub struct WasmLinker {
    cmd: Command,
    target: TargetConfig,
}

impl WasmLinker {
    /// Create a new WebAssembly linker.
    pub fn new(target: &TargetConfig) -> Self {
        Self {
            cmd: Command::new("wasm-ld"),
            target: target.clone(),
        }
    }
}

impl WasmLinker {
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
            LinkOutput::Executable => {
                // WASM "executable" - entry point is _start or main
                self.cmd.arg("--entry=_start");
            }
            LinkOutput::SharedLibrary => {
                // WASM module without entry point
                self.cmd.arg("--no-entry");
                self.cmd.arg("--export-dynamic");
            }
            LinkOutput::StaticLibrary | LinkOutput::PositionIndependentExecutable => {
                // Not applicable to WASM
            }
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

    /// Link a library by name (WASM linking is always static).
    pub fn link_library(&mut self, name: &str, _kind: LibraryKind) {
        self.cmd.arg(format!("-l{name}"));
    }

    /// Enable garbage collection of unused sections.
    pub fn gc_sections(&mut self, enable: bool) {
        if enable {
            self.cmd.arg("--gc-sections");
        }
    }

    /// Strip debug symbols from output.
    pub fn strip_symbols(&mut self, strip: bool) {
        if strip {
            self.cmd.arg("--strip-all");
        }
    }

    /// Add symbols to export (for shared libraries).
    pub fn export_symbols(&mut self, symbols: &[String]) {
        for sym in symbols {
            self.cmd.arg(format!("--export={sym}"));
        }
    }

    /// Add a raw argument to the linker command.
    pub fn add_arg(&mut self, arg: &str) {
        self.cmd.arg(arg);
    }

    /// Add a linker-specific argument.
    pub fn link_arg(&mut self, arg: &str) {
        self.cmd.arg(arg);
    }

    /// Finalize and get the command to execute.
    pub fn finalize(self) -> Command {
        self.cmd
    }
}
