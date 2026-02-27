//! Typed IR pretty-printer for phase dumps.
//!
//! Produces human-readable representations of the type-annotated IR, intended
//! for compiler debugging via `ORI_DUMP_AFTER_TYPECK=1`. Output goes to stderr.
//!
//! Unlike `ast_dump` (which shows raw parse tree structure), this module shows
//! resolved types on every expression node, function signatures with inferred
//! types, and method dispatch info.

mod expr;

use std::fmt::Write;

use ori_ir::StringInterner;
use ori_parse::ParseOutput;
use ori_types::{Pool, TypedModule};

use self::expr::dump_expr;

/// Dump the typed IR to stderr.
///
/// Called when `ORI_DUMP_AFTER_TYPECK=1` is set. Shows the AST with resolved
/// types on every node, function signatures, and method dispatch info.
#[expect(clippy::unwrap_used, reason = "write! to String is infallible")]
pub fn dump_typed_ir(
    parse_result: &ParseOutput,
    typed: &TypedModule,
    pool: &Pool,
    interner: &StringInterner,
    path: &str,
) {
    let arena = &*parse_result.arena;
    let module = &parse_result.module;
    let mut out = String::with_capacity(8192);

    writeln!(out, "=== Typed IR after typeck: {path} ===").unwrap();
    writeln!(
        out,
        "  {} expressions, {} functions, {} types, {} errors",
        typed.expr_count(),
        typed.function_count(),
        typed.type_count(),
        typed.errors.len()
    )
    .unwrap();
    writeln!(out).unwrap();

    // Imports (no type info, context only)
    for use_def in &module.imports {
        let path_str = match &use_def.path {
            ori_ir::ast::ImportPath::Relative(name) => interner.lookup(*name).to_string(),
            ori_ir::ast::ImportPath::Module(parts) => parts
                .iter()
                .map(|n| interner.lookup(*n))
                .collect::<Vec<_>>()
                .join("."),
        };
        writeln!(out, "Use {path_str}").unwrap();
    }

    // Type declarations (names only — detailed type info is complex)
    for ty in &module.types {
        let name = interner.lookup(ty.name);
        writeln!(out, "Type {name}").unwrap();
    }

    // Trait definitions
    for tr in &module.traits {
        let name = interner.lookup(tr.name);
        writeln!(out, "Trait {name}").unwrap();
    }

    // Impl blocks
    for imp in &module.impls {
        let trait_str = imp.trait_path.as_ref().map_or_else(
            || "(inherent)".to_string(),
            |path| {
                path.iter()
                    .map(|n| interner.lookup(*n))
                    .collect::<Vec<_>>()
                    .join(".")
            },
        );
        let self_str: Vec<_> = imp.self_path.iter().map(|n| interner.lookup(*n)).collect();
        writeln!(out, "Impl {trait_str} for {}", self_str.join(".")).unwrap();
    }

    // Functions — pair AST bodies with TypedModule signatures
    for ast_func in &module.functions {
        let sig = typed.function(ast_func.name);
        dump_function(&mut out, ast_func, sig, arena, typed, pool, interner);
    }

    // Tests
    for test in &module.tests {
        let name = interner.lookup(test.name);
        writeln!(out, "Test \"{name}\"").unwrap();
        dump_expr(&mut out, test.body, arena, typed, pool, interner, 1);
    }

    // Extern blocks
    for block in &module.extern_blocks {
        let conv = interner.lookup(block.convention);
        writeln!(out, "Extern \"{conv}\"").unwrap();
    }

    writeln!(out, "=== END Typed IR ===").unwrap();
    eprint!("{out}");
}

/// Dump a function definition with resolved types from the type checker.
///
/// If the function has a `FunctionSig` from type checking, its parameter and
/// return types come from the Pool (resolved). Otherwise falls back to a
/// "not typed" placeholder.
#[expect(clippy::unwrap_used, reason = "write! to String is infallible")]
fn dump_function(
    out: &mut String,
    func: &ori_ir::Function,
    sig: Option<&ori_types::FunctionSig>,
    arena: &ori_ir::ExprArena,
    typed: &TypedModule,
    pool: &Pool,
    interner: &StringInterner,
) {
    let name = interner.lookup(func.name);
    let vis = if func.visibility == ori_ir::ast::Visibility::Public {
        "pub "
    } else {
        ""
    };

    let Some(sig) = sig else {
        writeln!(out, "{vis}Function @{name} (not typed)").unwrap();
        dump_expr(out, func.body, arena, typed, pool, interner, 1);
        return;
    };

    // Parameter list with resolved types
    let param_strs: Vec<String> = sig
        .param_names
        .iter()
        .zip(sig.param_types.iter())
        .map(|(pname, pty)| {
            let pname_str = interner.lookup(*pname);
            let type_str = pool.format_type_resolved(*pty, interner);
            format!("{pname_str}: {type_str}")
        })
        .collect();

    let ret_str = pool.format_type_resolved(sig.return_type, interner);

    // Generic parameters
    let generics = if sig.is_generic() {
        let params: Vec<&str> = sig
            .type_params
            .iter()
            .map(|n| interner.lookup(*n))
            .collect();
        format!("<{}>", params.join(", "))
    } else {
        String::new()
    };

    // Capability clauses
    let caps = if sig.has_capabilities() {
        let cap_names: Vec<&str> = sig
            .capabilities
            .iter()
            .map(|n| interner.lookup(*n))
            .collect();
        format!(" uses {}", cap_names.join(", "))
    } else {
        String::new()
    };

    writeln!(
        out,
        "{vis}Function @{name}{generics} ({}) -> {ret_str}{caps}",
        param_strs.join(", ")
    )
    .unwrap();

    // Body
    dump_expr(out, func.body, arena, typed, pool, interner, 1);
}
