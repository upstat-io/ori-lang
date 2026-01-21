// Sigil Compiler Library
//
// This crate provides the core compiler functionality for the Sigil language.
// It can be used as a library by other tools (LSP, formatter, etc.) or
// as a standalone CLI via the sigilc binary.

pub mod ast;
pub mod builtins;
pub mod codegen;
pub mod errors;
pub mod eval;
pub mod lexer;
pub mod parser;
pub mod types;

pub use ast::Module;
pub use eval::run as interpret;
pub use types::check as type_check;

/// Compile source code to a typed AST
pub fn compile(source: &str, filename: &str) -> Result<Module, String> {
    let tokens = lexer::tokenize(source, filename)?;
    let ast = parser::parse(tokens, filename)?;
    let typed_ast = types::check(ast)?;
    Ok(typed_ast)
}

/// Run source code through the interpreter
pub fn run(source: &str, filename: &str) -> Result<(), String> {
    let typed_ast = compile(source, filename)?;
    eval::run(typed_ast)?;
    Ok(())
}

/// Compile to C code
pub fn emit_c(source: &str, filename: &str) -> Result<String, String> {
    let typed_ast = compile(source, filename)?;
    codegen::generate(&typed_ast)
}
