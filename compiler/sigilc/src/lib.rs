// Sigil Compiler Library
//
// This crate provides the core compiler functionality for the Sigil language.
// It can be used as a library by other tools (LSP, formatter, etc.) or
// as a standalone CLI via the sigilc binary.
//
// Pipeline:
// - AST path (interpreter): Source → Lexer → Parser → AST → TypeChecker → Interpreter
// - TIR path (codegen): Source → Lexer → Parser → AST → TypeChecker+Lower → TIR → Passes → Codegen

pub mod ast;
pub mod builtins;
pub mod codegen;
pub mod context;
pub mod core;
pub mod errors;
pub mod eval;
pub mod ir;
pub mod lexer;
pub mod parser;
pub mod passes;
pub mod patterns;
pub mod symbols;
pub mod traits;
pub mod types;

pub use ast::Module;
pub use eval::run as interpret;
pub use ir::TModule;
pub use types::check as type_check;
pub use types::check_and_lower;

/// Compile source code to a typed AST (for interpreter)
pub fn compile(source: &str, filename: &str) -> Result<Module, String> {
    let tokens = lexer::tokenize(source, filename)?;
    let ast = parser::parse(tokens, filename)?;
    let typed_ast = types::check(ast)?;
    Ok(typed_ast)
}

/// Compile source code to TIR (for codegen)
pub fn compile_tir(source: &str, filename: &str) -> Result<TModule, String> {
    let tokens = lexer::tokenize(source, filename)?;
    let ast = parser::parse(tokens, filename)?;
    let tir = types::check_and_lower(ast)?;
    Ok(tir)
}

/// Run source code through the interpreter
pub fn run(source: &str, filename: &str) -> Result<(), String> {
    let typed_ast = compile(source, filename)?;
    eval::run(typed_ast)?;
    Ok(())
}

/// Compile to C code (legacy AST-based)
pub fn emit_c(source: &str, filename: &str) -> Result<String, String> {
    let typed_ast = compile(source, filename)?;
    codegen::generate(&typed_ast)
}

/// Compile to C code using TIR pipeline
/// This is the new recommended path for code generation
pub fn emit_c_tir(source: &str, filename: &str) -> Result<String, String> {
    let mut tir = compile_tir(source, filename)?;

    // Run passes
    let pm = passes::PassManager::default_pipeline();
    let mut ctx = passes::PassContext::new();
    pm.run(&mut tir, &mut ctx)
        .map_err(|e| format!("Pass error: {}", e))?;

    // Generate C code from TIR
    codegen::generate_from_tir(&tir)
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
