// Sigil Compiler Library
//
// This crate provides the core compiler functionality for the Sigil language.
// It can be used as a library by other tools (LSP, formatter, etc.) or
// as a standalone CLI via the sigilc binary.
//
// Pipeline:
// - AST path (interpreter): Source → Lexer → Parser → AST → TypeChecker → Interpreter
// - TIR path (codegen): Source → Lexer → Parser → AST → TypeChecker+Lower → TIR → Passes → Codegen

pub mod arc;
pub mod ast;
pub mod builtins;
pub mod codegen;
pub mod context;
pub mod core;
pub mod errors;
pub mod eval;
pub mod format;
pub mod intern;
pub mod ir;
pub mod lexer;
pub mod modules;
pub mod parser;
pub mod passes;
pub mod patterns;
pub mod symbols;
pub mod traits;
pub mod traverse;
pub mod types;

pub use ast::Module;
pub use errors::{Diagnostic, DiagnosticResult};
pub use eval::run as interpret;
pub use ir::TModule;
pub use types::check as type_check;
pub use types::check_and_lower;

// Re-export ARC validation types for mandatory ARC enforcement
pub use arc::{ArcError, ArcResult, ArcSummary, ArcValidatedModule, ModuleArcInfo};

use errors::codes::ErrorCode;

/// Convert string error to diagnostic
fn string_to_diag(msg: String, filename: &str) -> Diagnostic {
    Diagnostic::error(ErrorCode::E0000, msg)
        .with_label(errors::Span::new(filename, 0..0), "error occurred here")
}

/// Compile source code to a typed AST (for interpreter)
pub fn compile(source: &str, filename: &str) -> DiagnosticResult<Module> {
    let tokens = lexer::tokenize(source, filename).map_err(|e| string_to_diag(e, filename))?;
    let ast = parser::parse(tokens, filename).map_err(|e| string_to_diag(e, filename))?;
    types::check(ast)
}

/// Compile source code to TIR (for codegen)
pub fn compile_tir(source: &str, filename: &str) -> DiagnosticResult<TModule> {
    let tokens = lexer::tokenize(source, filename).map_err(|e| string_to_diag(e, filename))?;
    let ast = parser::parse(tokens, filename).map_err(|e| string_to_diag(e, filename))?;
    types::check_and_lower(ast)
}

/// Run source code through the interpreter
pub fn run(source: &str, filename: &str) -> DiagnosticResult<()> {
    let typed_ast = compile(source, filename)?;
    eval::run(typed_ast)
        .map(|_| ())
        .map_err(|e| string_to_diag(e, filename))
}

/// Compile to C code (AST-based)
pub fn emit_c(source: &str, filename: &str) -> DiagnosticResult<String> {
    let typed_ast = compile(source, filename)?;
    codegen::generate(&typed_ast).map_err(|e| string_to_diag(e, filename))
}

/// Compile to C code using TIR pipeline with mandatory ARC validation
///
/// This is the recommended path for code generation. It enforces exhaustive
/// ARC analysis through the type-state pattern (ArcValidatedModule), ensuring
/// that all IR variants are properly handled for memory management.
pub fn emit_c_tir(source: &str, filename: &str) -> DiagnosticResult<String> {
    let mut tir = compile_tir(source, filename)?;

    // Run passes
    let pm = passes::PassManager::default_pipeline();
    let mut ctx = passes::PassContext::new();
    pm.run(&mut tir, &mut ctx)
        .map_err(|e| string_to_diag(format!("Pass error: {}", e), filename))?;

    // Validate for ARC (MANDATORY) - returns ArcValidatedModule
    let validated = arc::ArcValidatedModule::validate(tir)
        .map_err(|e| string_to_diag(format!("ARC validation error: {}", e), filename))?;

    // Generate C code from validated module
    codegen::generate_from_validated(&validated).map_err(|e| string_to_diag(e, filename))
}

/// Compile to C code using TIR pipeline with verbose ARC tracking
///
/// The generated C code will print all ARC operations to stderr at runtime:
/// - ALLOC/FREE: Memory allocations and deallocations
/// - RETAIN/RELEASE: Reference count changes
/// - SUMMARY: At program end, shows totals and detects leaks
///
/// This is useful for debugging memory management issues or understanding
/// how ARC works in practice.
pub fn emit_c_tir_verbose(source: &str, filename: &str) -> DiagnosticResult<String> {
    let mut tir = compile_tir(source, filename)?;

    // Run passes
    let pm = passes::PassManager::default_pipeline();
    let mut ctx = passes::PassContext::new();
    pm.run(&mut tir, &mut ctx)
        .map_err(|e| string_to_diag(format!("Pass error: {}", e), filename))?;

    // Validate for ARC (MANDATORY)
    let validated = arc::ArcValidatedModule::validate(tir)
        .map_err(|e| string_to_diag(format!("ARC validation error: {}", e), filename))?;

    // Generate C code with verbose ARC tracking
    codegen::tir::generate_from_validated_verbose(&validated).map_err(|e| string_to_diag(e, filename))
}

/// Compile to C code using TIR pipeline, returning both code and ARC info
///
/// This function is useful when you need access to the ARC analysis results,
/// for example for debugging or optimization decisions.
pub fn emit_c_tir_with_info(
    source: &str,
    filename: &str,
) -> DiagnosticResult<(String, arc::ArcSummary)> {
    let mut tir = compile_tir(source, filename)?;

    // Run passes
    let pm = passes::PassManager::default_pipeline();
    let mut ctx = passes::PassContext::new();
    pm.run(&mut tir, &mut ctx)
        .map_err(|e| string_to_diag(format!("Pass error: {}", e), filename))?;

    // Validate for ARC (MANDATORY)
    let validated = arc::ArcValidatedModule::validate(tir)
        .map_err(|e| string_to_diag(format!("ARC validation error: {}", e), filename))?;

    // Compute summary
    let summary = arc::ArcSummary::from_arc_info(validated.arc_info());

    // Generate C code from validated module
    let code =
        codegen::generate_from_validated(&validated).map_err(|e| string_to_diag(e, filename))?;

    Ok((code, summary))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIMPLE_PROGRAM: &str = "@main () -> int = 42";
    const FUNCTION_PROGRAM: &str = r#"
@add (a: int, b: int) -> int = a + b
@main () -> int = add(1, 2)
@test_add tests @add () -> void = assert_eq(add(2, 3), 5)
"#;

    #[test]
    fn test_compile_simple() {
        let result = compile(SIMPLE_PROGRAM, "test.si");
        assert!(result.is_ok());
        let module = result.unwrap();
        assert_eq!(module.name, "test.si");
    }

    #[test]
    fn test_compile_with_function() {
        let result = compile(FUNCTION_PROGRAM, "test.si");
        assert!(result.is_ok());
    }

    #[test]
    fn test_compile_syntax_error() {
        let result = compile("@main () -> int =", "test.si");
        assert!(result.is_err());
    }

    #[test]
    fn test_compile_type_error() {
        let result = compile(r#"@main () -> int = "not an int""#, "test.si");
        assert!(result.is_err());
    }

    #[test]
    fn test_compile_tir_simple() {
        let result = compile_tir(SIMPLE_PROGRAM, "test.si");
        assert!(result.is_ok());
        let tir = result.unwrap();
        assert_eq!(tir.name, "test.si");
    }

    #[test]
    fn test_compile_tir_with_function() {
        let result = compile_tir(FUNCTION_PROGRAM, "test.si");
        assert!(result.is_ok(), "compile_tir failed: {:?}", result.err());
    }

    #[test]
    fn test_run_simple() {
        let result = run(SIMPLE_PROGRAM, "test.si");
        assert!(result.is_ok());
    }

    #[test]
    fn test_run_with_function() {
        let result = run(FUNCTION_PROGRAM, "test.si");
        assert!(result.is_ok());
    }

    #[test]
    fn test_run_error() {
        // Division by zero should cause a runtime error
        let result = run("@main () -> int = 1 / 0", "test.si");
        assert!(result.is_err());
    }

    #[test]
    fn test_emit_c_simple() {
        let result = emit_c(SIMPLE_PROGRAM, "test.si");
        assert!(result.is_ok());
        let c_code = result.unwrap();
        assert!(c_code.contains("main"));
    }

    #[test]
    fn test_emit_c_with_function() {
        let result = emit_c(FUNCTION_PROGRAM, "test.si");
        assert!(result.is_ok());
        let c_code = result.unwrap();
        assert!(c_code.contains("add"));
    }

    #[test]
    fn test_emit_c_tir_simple() {
        let result = emit_c_tir(SIMPLE_PROGRAM, "test.si");
        assert!(result.is_ok());
        let c_code = result.unwrap();
        assert!(c_code.contains("main"));
    }

    #[test]
    fn test_emit_c_tir_with_patterns() {
        let source = r#"
@main () -> int = fold([1, 2, 3], 0, +)
"#;
        let result = emit_c_tir(source, "test.si");
        assert!(result.is_ok());
    }

    #[test]
    fn test_interpret_reexport() {
        // Test that interpret is properly re-exported
        let result = interpret(compile(SIMPLE_PROGRAM, "test.si").unwrap());
        assert!(result.is_ok());
    }
}
