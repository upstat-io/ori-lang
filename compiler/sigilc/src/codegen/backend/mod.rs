// Backend abstraction for Sigil code generation
//
// Provides a trait-based interface for code generation backends.
// Currently supports:
// - C backend (default): Generates C source code
//
// Future backends:
// - LLVM: Direct LLVM IR generation
// - WebAssembly: WASM output

pub mod c;
mod registry;
mod traits;

pub use c::CBackend;
pub use registry::{
    backend_names, get_backend, get_default_backend, has_backend, registry, BackendInfo,
    BackendRegistry,
};
pub use traits::{
    Backend, CodegenMetadata, CodegenOptions, CodegenStats, ExecutableBackend, GeneratedCode,
    GeneratedContent, OutputFormat,
};
