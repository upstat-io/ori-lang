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
//! - [`TargetError`]: Error types for target operations
//!
//! # Example
//!
//! ```ignore
//! use ori_llvm::aot::{TargetConfig, TargetError};
//!
//! // Native compilation
//! let target = TargetConfig::native()?;
//! let machine = target.create_target_machine()?;
//!
//! // Cross-compilation
//! let target = TargetConfig::from_triple("aarch64-apple-darwin")?
//!     .with_cpu("apple-m1")
//!     .with_opt_level(OptimizationLevel::Aggressive);
//! ```
//!
//! # Modules
//!
//! - `target`: Target configuration and machine creation

pub mod target;

// Re-export key types
pub use target::{
    get_host_cpu_features, get_host_cpu_name, parse_features, TargetConfig, TargetError,
    TargetTripleComponents, SUPPORTED_TARGETS,
};
