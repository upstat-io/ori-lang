//! Code generation phase tests.
//!
//! Tests for the `ori_llvm` crate, validating:
//! - LLVM IR generation
//! - Debug info (DWARF/CodeView)
//! - Linking behavior
//! - Optimization passes
//! - Target-specific code
//! - ABI compliance
//!
//! Note: These tests require the `llvm` feature to be enabled.
//!
//! # Test Organization
//!
//! - `debug_config` - Debug level, format, and configuration tests
//! - `debug_builder` - Debug info builder creation and basic types
//! - `debug_types` - Composite debug types (struct, enum, array, etc.)
//! - `debug_context` - Debug context and line map tests
//! - `linker_core` - Core linker infrastructure (flavor, output, library, driver)
//! - `linker_gcc` - GCC/Clang linker tests (Unix)
//! - `linker_msvc` - MSVC linker tests (Windows)
//! - `linker_wasm` - WebAssembly linker tests

// Debug info tests
#[cfg(feature = "llvm")]
mod debug_builder;
#[cfg(feature = "llvm")]
mod debug_config;
#[cfg(feature = "llvm")]
mod debug_context;
#[cfg(feature = "llvm")]
mod debug_types;

// Linker tests
#[cfg(feature = "llvm")]
mod linker_core;
#[cfg(feature = "llvm")]
mod linker_gcc;
#[cfg(feature = "llvm")]
mod linker_msvc;
#[cfg(feature = "llvm")]
mod linker_wasm;

// Optimization tests
#[cfg(feature = "llvm")]
mod optimization;

// Object emission tests
#[cfg(feature = "llvm")]
mod object_emit;

// Runtime library tests (ori_rt)
mod runtime_lib;

// WASM-specific tests
#[cfg(feature = "llvm")]
mod wasm;

// Build command tests
mod build_command;

// Target configuration tests
#[cfg(feature = "llvm")]
mod targets;

// Symbol mangling tests
#[cfg(feature = "llvm")]
mod mangling;

// Runtime library configuration tests
#[cfg(feature = "llvm")]
mod runtime;

// Diagnostic integration tests
#[cfg(feature = "llvm")]
mod diagnostics;
