//! WebAssembly linker implementation.
//!
//! This module provides the `WasmLinker` for WebAssembly targets
//! using `wasm-ld` from the LLVM toolchain.
//!
//! # Architecture
//!
//! The WASM linker follows the same pattern as other linkers but with
//! WebAssembly-specific features like memory configuration, feature flags,
//! and WASI support.
//!
//! # Usage
//!
//! ```ignore
//! use ori_llvm::aot::linker::WasmLinker;
//! use ori_llvm::aot::wasm::WasmConfig;
//!
//! let mut linker = WasmLinker::new(&target);
//!
//! // Apply WASM-specific configuration
//! let config = WasmConfig::browser();
//! linker.apply_config(&config);
//!
//! // Standard linking operations
//! linker.set_output(Path::new("output.wasm"));
//! linker.add_object(Path::new("main.o"));
//! ```

use std::path::Path;
use std::process::Command;

use super::{LibraryKind, LinkOutput};
use crate::aot::wasm::WasmConfig;
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

    /// Create a new WebAssembly linker with a specific linker path.
    pub fn with_linker(target: &TargetConfig, linker_path: &Path) -> Self {
        Self {
            cmd: Command::new(linker_path),
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

    /// Apply comprehensive WASM configuration.
    ///
    /// This applies memory settings, stack size, and feature flags
    /// from a `WasmConfig` struct.
    pub fn apply_config(&mut self, config: &WasmConfig) {
        for arg in config.linker_args() {
            self.cmd.arg(arg);
        }
    }

    /// Set initial and maximum memory size.
    ///
    /// Memory is specified in bytes. WASM memory is organized in 64KB pages,
    /// so values are rounded up to page boundaries.
    pub fn set_memory(&mut self, initial_bytes: u64, max_bytes: Option<u64>) {
        self.cmd.arg(format!("--initial-memory={initial_bytes}"));
        if let Some(max) = max_bytes {
            self.cmd.arg(format!("--max-memory={max}"));
        }
    }

    /// Set stack size in bytes.
    pub fn set_stack_size(&mut self, bytes: u32) {
        self.cmd.arg(format!("--stack-size={bytes}"));
    }

    /// Import memory from the host environment.
    ///
    /// When enabled, the WASM module expects memory to be provided
    /// during instantiation rather than creating its own.
    pub fn import_memory(&mut self, enable: bool) {
        if enable {
            self.cmd.arg("--import-memory");
        }
    }

    /// Export memory to the host environment.
    ///
    /// When enabled (default), the host can access WASM linear memory.
    pub fn export_memory(&mut self, enable: bool) {
        if enable {
            self.cmd.arg("--export-memory");
        }
    }

    /// Enable shared memory for threading support.
    pub fn shared_memory(&mut self, enable: bool) {
        if enable {
            self.cmd.arg("--shared-memory");
        }
    }

    /// Enable bulk memory operations (faster memcpy/memset).
    pub fn enable_bulk_memory(&mut self, enable: bool) {
        if enable {
            self.cmd.arg("--enable-bulk-memory");
        }
    }

    /// Enable SIMD instructions.
    pub fn enable_simd(&mut self, enable: bool) {
        if enable {
            self.cmd.arg("--enable-simd");
        }
    }

    /// Enable multi-value returns.
    pub fn enable_multivalue(&mut self, enable: bool) {
        if enable {
            self.cmd.arg("--enable-multivalue");
        }
    }

    /// Enable reference types.
    pub fn enable_reference_types(&mut self, enable: bool) {
        if enable {
            self.cmd.arg("--enable-reference-types");
        }
    }

    /// Enable exception handling.
    pub fn enable_exception_handling(&mut self, enable: bool) {
        if enable {
            self.cmd.arg("--enable-exception-handling");
        }
    }

    /// Set a custom entry point function.
    ///
    /// By default, WASM executables use `_start` as the entry point.
    pub fn set_entry(&mut self, entry: &str) {
        self.cmd.arg(format!("--entry={entry}"));
    }

    /// Disable the entry point (for library modules).
    pub fn no_entry(&mut self) {
        self.cmd.arg("--no-entry");
    }

    /// Allow undefined symbols (for partial linking or WASI).
    pub fn allow_undefined(&mut self, enable: bool) {
        if enable {
            self.cmd.arg("--allow-undefined");
        }
    }

    /// Allow undefined symbols from a file.
    pub fn allow_undefined_file(&mut self, path: &Path) {
        self.cmd.arg("--allow-undefined-file").arg(path);
    }

    /// Enable verbose output for debugging.
    pub fn verbose(&mut self, enable: bool) {
        if enable {
            self.cmd.arg("--verbose");
        }
    }

    /// Finalize and get the command to execute.
    pub fn finalize(self) -> Command {
        self.cmd
    }
}

#[cfg(test)]
mod tests;
