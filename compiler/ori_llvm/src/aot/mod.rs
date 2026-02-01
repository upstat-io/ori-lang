//! AOT (Ahead-of-Time) Compilation Module
//!
//! This module provides functionality for generating native executables
//! and WebAssembly from Ori source code.
//!
//! # Architecture
//!
//! The AOT pipeline extends the existing JIT infrastructure:
//!
//! ```text
//! ┌─────────┐    ┌─────────┐    ┌─────────┐    ┌─────────┐
//! │  Parse  │───▶│  Type   │───▶│  LLVM   │───▶│ Object  │
//! │  (AST)  │    │  Check  │    │   IR    │    │  File   │
//! └─────────┘    └─────────┘    └─────────┘    └────┬────┘
//!                                                    │
//!                              ┌─────────┐    ┌─────▼────┐
//!                              │  Exe /  │◀───│   Link   │
//!                              │   Lib   │    │          │
//!                              └─────────┘    └──────────┘
//! ```
//!
//! # Key Components
//!
//! - [`TargetConfig`]: Target triple, CPU, and feature configuration
//! - [`ObjectEmitter`]: Emit LLVM modules as object files
//! - [`OutputFormat`]: Output format selection (object, assembly, bitcode, LLVM IR)
//! - [`DebugInfoBuilder`]: DWARF/CodeView debug information generation
//! - [`OptimizationConfig`]: Optimization pass pipeline configuration
//! - [`run_optimization_passes`]: Run LLVM optimization passes on a module
//! - [`LinkerDriver`]: Platform-agnostic linker driver for producing executables
//!
//! # Example
//!
//! ```ignore
//! use ori_llvm::aot::{TargetConfig, ObjectEmitter, OutputFormat, DebugInfoConfig, DebugLevel};
//! use ori_llvm::aot::{LinkerDriver, LinkInput, LinkOutput};
//! use std::path::Path;
//!
//! // Native compilation with debug info
//! let target = TargetConfig::native()?;
//! let emitter = ObjectEmitter::new(&target)?;
//!
//! // Configure debug info
//! let debug_config = DebugInfoConfig::new(DebugLevel::Full);
//! let debug_builder = DebugInfoBuilder::new(&module, &context, debug_config, "main.ori", "src");
//!
//! // Configure and emit module
//! emitter.configure_module(&module)?;
//! if let Some(di) = debug_builder {
//!     di.finalize();
//! }
//! emitter.emit_object(&module, Path::new("output.o"))?;
//!
//! // Link into executable
//! let driver = LinkerDriver::new(&target);
//! driver.link(LinkInput {
//!     objects: vec!["output.o".into()],
//!     output: "myapp".into(),
//!     output_kind: LinkOutput::Executable,
//!     ..Default::default()
//! })?;
//!
//! // Cross-compilation
//! let target = TargetConfig::from_triple("aarch64-apple-darwin")?
//!     .with_cpu("apple-m1")
//!     .with_opt_level(OptimizationLevel::Aggressive);
//! let emitter = ObjectEmitter::new(&target)?;
//! emitter.emit(&module, Path::new("output.o"), OutputFormat::Object)?;
//! ```
//!
//! # Modules
//!
//! - `target`: Target configuration and machine creation
//! - `object`: Object file emission
//! - `mangle`: Symbol name mangling
//! - `debug`: Debug information generation (DWARF/CodeView)
//! - `passes`: Optimization pipeline (LLVM new pass manager)
//! - `linker`: Platform-agnostic linker driver

pub mod debug;
pub mod linker;
pub mod mangle;
pub mod object;
pub mod passes;
pub mod target;

// Re-export key types from target
pub use target::{
    get_host_cpu_features, get_host_cpu_name, parse_features, TargetConfig, TargetError,
    TargetTripleComponents, SUPPORTED_TARGETS,
};

// Re-export key types from object
pub use object::{EmitError, ObjectEmitter, OutputFormat};

// Re-export key types from mangle
pub use mangle::{demangle, is_ori_symbol, Mangler, MANGLE_PREFIX};

// Re-export key types from debug
pub use debug::{
    DebugContext, DebugFormat, DebugInfoBuilder, DebugInfoConfig, DebugInfoError, DebugLevel,
    FieldInfo, LineMap,
};

// Re-export key types from passes
pub use passes::{
    run_custom_pipeline, run_optimization_passes, LtoMode, OptimizationConfig, OptimizationError,
    OptimizationLevel,
};

// Re-export key types from linker
pub use linker::{
    LibraryKind, LinkInput, LinkLibrary, LinkOutput, Linker, LinkerDetection, LinkerDriver,
    LinkerError, LinkerFlavor,
};
