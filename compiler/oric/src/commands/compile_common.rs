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
use ori_types::TypeCheckResult;
#[cfg(feature = "llvm")]
use oric::parser::ParseOutput;
#[cfg(feature = "llvm")]
use oric::query::{parsed, typed};
#[cfg(feature = "llvm")]
use oric::{CompilerDb, Db, SourceFile};

/// Information about an imported function for codegen.
#[cfg(feature = "llvm")]
#[derive(Debug, Clone)]
pub struct ImportedFunctionInfo {
    /// The mangled name of the function (e.g., `_ori_helper$add`).
    pub mangled_name: String,
    /// Parameter types as `TypeId`s.
    pub param_types: Vec<ori_ir::TypeId>,
    /// Return type.
    pub return_type: ori_ir::TypeId,
}

/// Check a source file for parse and type errors.
///
/// Prints all errors to stderr and returns `None` if any errors occurred.
/// This accumulates all errors before reporting, giving users a complete picture.
#[cfg(feature = "llvm")]
pub fn check_source(
    db: &CompilerDb,
    file: SourceFile,
    path: &str,
) -> Option<(ParseOutput, TypeCheckResult)> {
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
        for error in type_result.errors() {
            eprintln!("  {}: {}", error.span, error.message());
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
    type_result: &TypeCheckResult,
    source_path: &str,
) -> ori_llvm::inkwell::module::Module<'ctx> {
    // Use the interner from the database - Names in the AST reference this interner
    let interner = db.interner();
    let module_name = Path::new(source_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("module");

    let compiler = ModuleCompiler::new(context, interner, module_name);
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
    // Convert Idx to TypeId at the LLVM boundary (both are u32 newtypes)
    let arena = &parse_result.arena;
    let expr_types: Vec<ori_ir::TypeId> = type_result
        .typed
        .expr_types
        .iter()
        .map(|idx| ori_ir::TypeId::from_raw(idx.raw()))
        .collect();
    for func in &module.functions {
        compiler.compile_function(func, arena, &expr_types);
    }

    compiler.module().clone()
}

/// Compile source to LLVM IR with explicit module name and import declarations.
///
/// This is used for multi-file compilation where:
/// - The module name is explicitly provided for proper symbol mangling
/// - Imported functions are declared as external symbols
///
/// # Arguments
///
/// * `context` - The LLVM context
/// * `db` - The compiler database
/// * `parse_result` - Parsed AST
/// * `type_result` - Type checking results
/// * `source_path` - Path to the source file
/// * `module_name` - Explicit module name for symbol mangling
/// * `imported_functions` - Functions imported from other modules (declared as external)
#[cfg(feature = "llvm")]
pub fn compile_to_llvm_with_imports<'ctx>(
    context: &'ctx Context,
    db: &CompilerDb,
    parse_result: &ParseOutput,
    type_result: &TypeCheckResult,
    source_path: &str,
    module_name: &str,
    imported_functions: &[ImportedFunctionInfo],
) -> ori_llvm::inkwell::module::Module<'ctx> {
    use ori_llvm::inkwell::types::BasicMetadataTypeEnum;

    // Use the interner from the database
    let interner = db.interner();

    let compiler = ModuleCompiler::new(context, interner, module_name);
    compiler.declare_runtime();

    let cx = compiler.cx();

    // Declare imported functions as external symbols
    for import_info in imported_functions {
        // Convert TypeIds to LLVM types
        let param_llvm_types: Vec<BasicMetadataTypeEnum<'ctx>> = import_info
            .param_types
            .iter()
            .map(|&t| cx.llvm_type(t).into())
            .collect();

        let return_llvm_type = if import_info.return_type == ori_ir::TypeId::VOID {
            None
        } else {
            Some(cx.llvm_type(import_info.return_type))
        };

        cx.declare_external_fn_mangled(
            &import_info.mangled_name,
            &param_llvm_types,
            return_llvm_type,
        );
    }

    // Register user-defined struct types
    let module = &parse_result.module;
    for type_decl in &module.types {
        if let TypeDeclKind::Struct(fields) = &type_decl.kind {
            let field_names: Vec<_> = fields.iter().map(|f| f.name).collect();
            compiler.register_struct(type_decl.name, field_names);
        }
    }

    // Compile all functions
    // Convert Idx to TypeId at the LLVM boundary (both are u32 newtypes)
    let arena = &parse_result.arena;
    let expr_types: Vec<ori_ir::TypeId> = type_result
        .typed
        .expr_types
        .iter()
        .map(|idx| ori_ir::TypeId::from_raw(idx.raw()))
        .collect();
    for func in &module.functions {
        compiler.compile_function(func, arena, &expr_types);
    }

    // Log the source path for debugging (avoids unused variable warning)
    if std::env::var("ORI_DEBUG_LLVM").is_ok() {
        eprintln!(
            "Compiled module '{}' from '{}' with {} imported functions",
            module_name,
            source_path,
            imported_functions.len()
        );
    }

    compiler.module().clone()
}
