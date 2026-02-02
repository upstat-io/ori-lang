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
mod tests {
    use super::*;
    use crate::aot::target::TargetTripleComponents;

    fn test_target() -> TargetConfig {
        let components = TargetTripleComponents::parse("wasm32-unknown-unknown").unwrap();
        TargetConfig::from_components(components)
    }

    #[test]
    fn test_wasm_linker_new() {
        let target = test_target();
        let linker = WasmLinker::new(&target);
        assert!(linker.target().is_wasm());
    }

    #[test]
    fn test_wasm_linker_output() {
        let target = test_target();
        let mut linker = WasmLinker::new(&target);
        linker.set_output(Path::new("output.wasm"));
        let cmd = linker.finalize();
        let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();
        assert!(args.contains(&"-o".into()));
        assert!(args.contains(&"output.wasm".into()));
    }

    #[test]
    fn test_wasm_linker_executable() {
        let target = test_target();
        let mut linker = WasmLinker::new(&target);
        linker.set_output_kind(LinkOutput::Executable);
        let cmd = linker.finalize();
        let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();
        assert!(args.contains(&"--entry=_start".into()));
    }

    #[test]
    fn test_wasm_linker_shared_library() {
        let target = test_target();
        let mut linker = WasmLinker::new(&target);
        linker.set_output_kind(LinkOutput::SharedLibrary);
        let cmd = linker.finalize();
        let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();
        assert!(args.contains(&"--no-entry".into()));
        assert!(args.contains(&"--export-dynamic".into()));
    }

    #[test]
    fn test_wasm_linker_memory_config() {
        let target = test_target();
        let mut linker = WasmLinker::new(&target);
        linker.set_memory(1024 * 1024, Some(16 * 1024 * 1024));
        let cmd = linker.finalize();
        let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();
        assert!(args.contains(&"--initial-memory=1048576".into()));
        assert!(args.contains(&"--max-memory=16777216".into()));
    }

    #[test]
    fn test_wasm_linker_stack_size() {
        let target = test_target();
        let mut linker = WasmLinker::new(&target);
        linker.set_stack_size(512 * 1024);
        let cmd = linker.finalize();
        let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();
        assert!(args.contains(&"--stack-size=524288".into()));
    }

    #[test]
    fn test_wasm_linker_import_export_memory() {
        let target = test_target();
        let mut linker = WasmLinker::new(&target);
        linker.import_memory(true);
        linker.export_memory(true);
        let cmd = linker.finalize();
        let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();
        assert!(args.contains(&"--import-memory".into()));
        assert!(args.contains(&"--export-memory".into()));
    }

    #[test]
    fn test_wasm_linker_features() {
        let target = test_target();
        let mut linker = WasmLinker::new(&target);
        linker.enable_bulk_memory(true);
        linker.enable_simd(true);
        linker.enable_multivalue(true);
        let cmd = linker.finalize();
        let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();
        assert!(args.contains(&"--enable-bulk-memory".into()));
        assert!(args.contains(&"--enable-simd".into()));
        assert!(args.contains(&"--enable-multivalue".into()));
    }

    #[test]
    fn test_wasm_linker_gc_and_strip() {
        let target = test_target();
        let mut linker = WasmLinker::new(&target);
        linker.gc_sections(true);
        linker.strip_symbols(true);
        let cmd = linker.finalize();
        let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();
        assert!(args.contains(&"--gc-sections".into()));
        assert!(args.contains(&"--strip-all".into()));
    }

    #[test]
    fn test_wasm_linker_export_symbols() {
        let target = test_target();
        let mut linker = WasmLinker::new(&target);
        linker.export_symbols(&["foo".to_string(), "bar".to_string()]);
        let cmd = linker.finalize();
        let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();
        assert!(args.contains(&"--export=foo".into()));
        assert!(args.contains(&"--export=bar".into()));
    }

    #[test]
    fn test_wasm_linker_library_path() {
        let target = test_target();
        let mut linker = WasmLinker::new(&target);
        linker.add_library_path(Path::new("/usr/lib/wasm"));
        let cmd = linker.finalize();
        let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();
        assert!(args.contains(&"-L".into()));
        assert!(args.contains(&"/usr/lib/wasm".into()));
    }

    #[test]
    fn test_wasm_linker_link_library() {
        let target = test_target();
        let mut linker = WasmLinker::new(&target);
        linker.link_library("c", LibraryKind::Static);
        let cmd = linker.finalize();
        let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();
        assert!(args.contains(&"-lc".into()));
    }

    #[test]
    fn test_wasm_linker_custom_entry() {
        let target = test_target();
        let mut linker = WasmLinker::new(&target);
        linker.set_entry("main");
        let cmd = linker.finalize();
        let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();
        assert!(args.contains(&"--entry=main".into()));
    }

    #[test]
    fn test_wasm_linker_no_entry() {
        let target = test_target();
        let mut linker = WasmLinker::new(&target);
        linker.no_entry();
        let cmd = linker.finalize();
        let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();
        assert!(args.contains(&"--no-entry".into()));
    }

    #[test]
    fn test_wasm_linker_allow_undefined() {
        let target = test_target();
        let mut linker = WasmLinker::new(&target);
        linker.allow_undefined(true);
        let cmd = linker.finalize();
        let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();
        assert!(args.contains(&"--allow-undefined".into()));
    }

    #[test]
    fn test_wasm_linker_apply_config() {
        use crate::aot::wasm::{WasmFeatures, WasmMemoryConfig, WasmStackConfig};

        let target = test_target();
        let mut linker = WasmLinker::new(&target);

        let config = WasmConfig {
            memory: WasmMemoryConfig::default().with_initial_pages(32),
            stack: WasmStackConfig::default().with_size_kb(256),
            features: WasmFeatures {
                bulk_memory: true,
                simd: true,
                ..WasmFeatures::default()
            },
            ..WasmConfig::default()
        };

        linker.apply_config(&config);
        let cmd = linker.finalize();
        let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();

        // Check memory config was applied
        assert!(args.iter().any(|a| a.contains("--initial-memory=")));
        assert!(args.iter().any(|a| a.contains("--stack-size=")));
        assert!(args.contains(&"--enable-bulk-memory".into()));
        assert!(args.contains(&"--enable-simd".into()));
    }

    #[test]
    fn test_wasm_linker_verbose() {
        let target = test_target();
        let mut linker = WasmLinker::new(&target);
        linker.verbose(true);
        let cmd = linker.finalize();
        let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();
        assert!(args.contains(&"--verbose".into()));
    }

    #[test]
    fn test_wasm_linker_shared_memory() {
        let target = test_target();
        let mut linker = WasmLinker::new(&target);
        linker.shared_memory(true);
        let cmd = linker.finalize();
        let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();
        assert!(args.contains(&"--shared-memory".into()));
    }

    #[test]
    fn test_wasm_linker_exception_handling() {
        let target = test_target();
        let mut linker = WasmLinker::new(&target);
        linker.enable_exception_handling(true);
        let cmd = linker.finalize();
        let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();
        assert!(args.contains(&"--enable-exception-handling".into()));
    }

    #[test]
    fn test_wasm_linker_reference_types() {
        let target = test_target();
        let mut linker = WasmLinker::new(&target);
        linker.enable_reference_types(true);
        let cmd = linker.finalize();
        let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();
        assert!(args.contains(&"--enable-reference-types".into()));
    }
}
