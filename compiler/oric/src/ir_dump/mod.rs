//! Typed IR pretty-printer for phase dumps.
//!
//! Produces human-readable representations of the type-annotated IR, intended
//! for compiler debugging via `ORI_DUMP_AFTER_TYPECK=1`. Output goes to stderr.
//!
//! Unlike `ast_dump` (which shows raw parse tree structure), this module shows
//! resolved types on every expression node, function signatures with inferred
//! types, and method dispatch info.

mod collections;
mod expr;
mod type_annot;

use std::fmt::Write;

use ori_ir::StringInterner;
use ori_parse::ParseOutput;
use ori_types::{Idx, Pool, TypedModule};

use self::expr::dump_expr;
use crate::dump_common;

/// Immutable render context threaded through the typed-IR dump recursion.
/// Bundles the per-dump state (the type pool + interner + idx-filter view) so
/// the recursive walkers stay at a small fixed arity instead of threading each
/// field positionally.
#[derive(Clone, Copy)]
pub(super) struct DumpCtx<'a> {
    pub(super) arena: &'a ori_ir::ExprArena,
    pub(super) typed: &'a TypedModule,
    pub(super) pool: &'a Pool,
    pub(super) interner: &'a StringInterner,
    /// Typeck idx-filter view: annotate every type with its resolved pool `Idx`.
    pub(super) with_idx: bool,
}

/// Render a type for the dump. When `with_idx` is set (the dump orchestrator's
/// typeck idx-filter view, folded in from `ORI_DUMP_TYPE_IDX`) annotates every
/// type with its resolved pool `Idx` (+ composite field/payload idxs) per
/// `Pool::format_type_with_idx`; otherwise the name-only resolved form.
pub(crate) fn fmt_ty(pool: &Pool, idx: Idx, interner: &StringInterner, with_idx: bool) -> String {
    if with_idx {
        pool.format_type_with_idx(idx, interner)
    } else {
        pool.format_type_resolved(idx, interner)
    }
}

/// Dump the typed IR to stderr.
///
/// Called when `ORI_DUMP_AFTER_TYPECK=1` is set. Shows the AST with resolved
/// types on every node, function signatures, and method dispatch info.
#[expect(clippy::unwrap_used, reason = "write! to String is infallible")]
pub(crate) fn dump_typed_ir(
    parse_result: &ParseOutput,
    typed: &TypedModule,
    pool: &Pool,
    interner: &StringInterner,
    path: &str,
    with_idx: bool,
) {
    let arena = &*parse_result.arena;
    let module = &parse_result.module;
    let ctx = DumpCtx {
        arena,
        typed,
        pool,
        interner,
        with_idx,
    };
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
        let path_str = dump_common::format_import_path(&use_def.path, interner);
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
        let header = dump_common::format_impl_header(imp, interner);
        writeln!(out, "Impl {header}").unwrap();
    }

    // Functions — pair AST bodies with TypedModule signatures
    for ast_func in &module.functions {
        let sig = typed.function(ast_func.name);
        dump_function(&mut out, ast_func, sig, &ctx);
    }

    // Tests
    for test in &module.tests {
        let name = interner.lookup(test.name);
        writeln!(out, "Test \"{name}\"").unwrap();
        dump_expr(&mut out, test.body, &ctx, 1);
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
    ctx: &DumpCtx,
) {
    let DumpCtx {
        pool,
        interner,
        with_idx,
        ..
    } = *ctx;
    let name = interner.lookup(func.name);
    let vis = if func.visibility == ori_ir::ast::Visibility::Public {
        "pub "
    } else {
        ""
    };

    let Some(sig) = sig else {
        writeln!(out, "{vis}Function @{name} (not typed)").unwrap();
        dump_expr(out, func.body, ctx, 1);
        return;
    };

    // Parameter list with resolved types
    let param_strs: Vec<String> = sig
        .param_names
        .iter()
        .zip(sig.param_types.iter())
        .map(|(pname, pty)| {
            let pname_str = interner.lookup(*pname);
            let type_str = fmt_ty(pool, *pty, interner, with_idx);
            format!("{pname_str}: {type_str}")
        })
        .collect();

    let ret_str = fmt_ty(pool, sig.return_type, interner, with_idx);

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
    dump_expr(out, func.body, ctx, 1);
}
