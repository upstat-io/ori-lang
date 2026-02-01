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
//!
//! # Example
//!
//! ```ignore
//! use ori_llvm::aot::{TargetConfig, ObjectEmitter, OutputFormat};
//! use std::path::Path;
//!
//! // Native compilation
//! let target = TargetConfig::native()?;
//! let emitter = ObjectEmitter::new(&target)?;
//!
//! // Configure and emit module
//! emitter.configure_module(&module)?;
//! emitter.emit_object(&module, Path::new("output.o"))?;
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

pub mod mangle;
pub mod object;
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
