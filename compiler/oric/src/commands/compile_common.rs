//! Shared compilation utilities for AOT build and run commands.
//!
//! This module extracts common compilation logic to avoid duplication between
//! `build_file` and `run_file_compiled`. Both commands need to:
//! 1. Parse and type-check source files
//! 2. Print accumulated errors
//! 3. Generate LLVM IR
//!
//! By centralizing this logic, bug fixes and enhancements apply to both commands.

#[cfg(feature = "llvm")]
use std::path::Path;

#[cfg(feature = "llvm")]
use ori_ir::ast::TypeDeclKind;
#[cfg(feature = "llvm")]
use ori_llvm::inkwell::context::Context;
#[cfg(feature = "llvm")]
use ori_llvm::module::ModuleCompiler;
#[cfg(feature = "llvm")]
use oric::parser::ParseOutput;
#[cfg(feature = "llvm")]
use oric::query::{parsed, typed};
#[cfg(feature = "llvm")]
use oric::typeck::TypedModule;
#[cfg(feature = "llvm")]
use oric::{CompilerDb, Db, SourceFile};

/// Check a source file for parse and type errors.
///
/// Prints all errors to stderr and returns `None` if any errors occurred.
/// This accumulates all errors before reporting, giving users a complete picture.
#[cfg(feature = "llvm")]
pub fn check_source(
    db: &CompilerDb,
    file: SourceFile,
    path: &str,
) -> Option<(ParseOutput, TypedModule)> {
    let mut has_errors = false;

    // Check for parse errors
    let parse_result = parsed(db, file);
    if parse_result.has_errors() {
        eprintln!("error: parse errors in '{path}':");
        for error in &parse_result.errors {
            eprintln!("  {}: {}", error.span, error.message);
        }
        has_errors = true;
    }

    // Check for type errors even if there were parse errors
    // This helps users see all issues at once
    let type_result = typed(db, file);
    if type_result.has_errors() {
        eprintln!("error: type errors in '{path}':");
        for error in &type_result.errors {
            let diag = error.to_diagnostic();
            eprintln!("  {diag}");
        }
        has_errors = true;
    }

    if has_errors {
        None
    } else {
        Some((parse_result, type_result))
    }
}

/// Compile source to LLVM IR.
///
/// Takes checked parse and type results and generates LLVM IR.
/// Returns the LLVM module ready for optimization and emission.
#[cfg(feature = "llvm")]
pub fn compile_to_llvm<'ctx>(
    context: &'ctx Context,
    db: &CompilerDb,
    parse_result: &ParseOutput,
    type_result: &TypedModule,
    source_path: &str,
) -> ori_llvm::inkwell::module::Module<'ctx> {
    // Use the interner from the database - Names in the AST reference this interner
    let interner = db.interner();
    let module_name = Path::new(source_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("module");

    let compiler = ModuleCompiler::new(context, &interner, module_name);
    compiler.declare_runtime();

    // Register user-defined struct types
    let module = &parse_result.module;
    for type_decl in &module.types {
        if let TypeDeclKind::Struct(fields) = &type_decl.kind {
            let field_names: Vec<_> = fields.iter().map(|f| f.name).collect();
            compiler.register_struct(type_decl.name, field_names);
        }
    }

    // Compile all functions
    let arena = &parse_result.arena;
    let expr_types = &type_result.expr_types;
    for func in &module.functions {
        compiler.compile_function(func, arena, expr_types);
    }

    compiler.module().clone()
}
