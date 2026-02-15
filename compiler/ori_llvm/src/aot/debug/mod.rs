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
//! ```ignore
//! use ori_llvm::aot::debug::{DebugInfoBuilder, DebugInfoConfig, DebugLevel};
//!
//! let config = DebugInfoConfig::new(DebugLevel::Full);
//! let di = DebugInfoBuilder::new(&module, &context, config, "src/main.ori", "src")?;
//!
//! // Create function debug info
//! let func_di = di.create_function("my_func", 10, &fn_type);
//! fn_val.set_subprogram(func_di);
//!
//! // Set debug location for instructions
//! di.set_location(&builder, 15, 4);
//!
//! // Finalize before emission
//! di.finalize();
//! ```

mod builder;
mod builder_scope;
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
