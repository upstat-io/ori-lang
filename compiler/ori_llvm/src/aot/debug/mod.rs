//! Debug Information Generation for AOT Compilation
//!
//! This module provides DWARF/CodeView debug information generation using LLVM's
//! `DIBuilder` infrastructure. Debug info enables source-level debugging with tools
//! like GDB, LLDB, and Visual Studio.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────┐     ┌──────────────────┐     ┌─────────────────┐
//! │  Source File    │────▶│  DebugInfoBuilder │────▶│  DWARF/CodeView │
//! │  (spans, names) │     │  (DIBuilder)      │     │  (in object)    │
//! └─────────────────┘     └──────────────────┘     └─────────────────┘
//! ```
//!
//! # Debug Levels
//!
//! - `None`: No debug info (smallest output, fastest compile)
//! - `LineTablesOnly`: Line numbers only (small overhead, basic debugging)
//! - `Full`: Complete debug info (types, variables, full debugging)
//!
//! # Usage
//!
//! ```no_run
//! use ori_llvm::aot::debug::{DebugInfoBuilder, DebugInfoConfig, DebugLevel};
//! use ori_llvm::inkwell::context::Context;
//! use ori_llvm::inkwell::debug_info::AsDIScope;
//!
//! let context = Context::create();
//! let module = context.create_module("example");
//! let builder = context.create_builder();
//! let fn_type = context.void_type().fn_type(&[], false);
//! let fn_val = module.add_function("my_func", fn_type, None);
//!
//! let config = DebugInfoConfig::new(DebugLevel::Full);
//! let di = DebugInfoBuilder::new(&module, &context, config, "src/main.ori", "src")
//!     .expect("full debug info is enabled");
//!
//! // Create function debug info
//! let debug_fn_type = di.create_subroutine_type(None, &[]);
//! let func_di = di.create_function("my_func", None, 10, debug_fn_type, false, true);
//! fn_val.set_subprogram(func_di);
//!
//! // Set debug location for instructions
//! di.set_location(&builder, 15, 4, func_di.as_debug_info_scope());
//!
//! // Finalize before emission
//! di.finalize();
//! ```

mod builder;
mod builder_scope;
mod builder_types;
mod config;
mod context;
mod line_map;

pub use builder::{DebugInfoBuilder, FieldInfo};
pub use config::{DebugFormat, DebugInfoConfig, DebugInfoError, DebugLevel};
pub use context::DebugContext;
pub use line_map::LineMap;

#[cfg(test)]
#[allow(clippy::doc_markdown, reason = "test code — doc style relaxed")]
mod tests;
