//! AST pretty-printer for phase dumps.
//!
//! Produces human-readable representations of the parsed AST, intended for
//! compiler debugging via `ORI_DUMP_AFTER_PARSE=1`. Output goes to stderr.
//!
//! This is NOT a source code formatter — it shows the tree structure that
//! the parser produced, helping developers verify that parsing is correct
//! before the AST flows into type checking.

mod expr;
mod patterns;

use std::fmt::Write;

use ori_ir::StringInterner;
use ori_parse::ParseOutput;

use self::expr::{dump_expr, dump_expr_inline};
use self::patterns::format_parsed_type;

/// Dump the parsed AST to stderr.
///
/// Called when `ORI_DUMP_AFTER_PARSE=1` is set. Produces an indented tree
/// showing module items, function signatures, and expression structure.
#[expect(clippy::unwrap_used, reason = "write! to String is infallible")]
pub fn dump_ast(parse_result: &ParseOutput, interner: &StringInterner, path: &str) {
    let arena = &*parse_result.arena;
    let module = &parse_result.module;
    let mut out = String::with_capacity(4096);

    writeln!(out, "=== AST after parse: {path} ===").unwrap();

    // Imports
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

    // Constants
    for c in &module.consts {
        let name = interner.lookup(c.name);
        write!(out, "Const ${name} = ").unwrap();
        dump_expr_inline(&mut out, c.value, arena, interner);
        writeln!(out).unwrap();
    }

    // Type declarations
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

    // Functions
    for func in &module.functions {
        dump_function(&mut out, func, arena, interner);
    }

    // Tests
    for test in &module.tests {
        let name = interner.lookup(test.name);
        writeln!(out, "Test \"{name}\"").unwrap();
        dump_expr(&mut out, test.body, arena, interner, 1);
    }

    // Extern blocks
    for block in &module.extern_blocks {
        let conv = interner.lookup(block.convention);
        writeln!(out, "Extern \"{conv}\"").unwrap();
    }

    writeln!(out, "=== END AST ===").unwrap();
    eprint!("{out}");
}

/// Dump a function definition.
#[expect(clippy::unwrap_used, reason = "write! to String is infallible")]
fn dump_function(
    out: &mut String,
    func: &ori_ir::Function,
    arena: &ori_ir::ExprArena,
    interner: &StringInterner,
) {
    let name = interner.lookup(func.name);
    let vis = if func.visibility == ori_ir::ast::Visibility::Public {
        "pub "
    } else {
        ""
    };

    // Build parameter list
    let params = arena.get_params(func.params);
    let param_strs: Vec<String> = params
        .iter()
        .map(|p| {
            let pname = interner.lookup(p.name);
            if let Some(ref ty) = p.ty {
                format!("{pname}: {}", format_parsed_type(ty, arena, interner))
            } else {
                pname.to_string()
            }
        })
        .collect();

    // Return type
    let ret = func
        .return_ty
        .as_ref()
        .map(|ty| format!(" -> {}", format_parsed_type(ty, arena, interner)))
        .unwrap_or_default();

    writeln!(
        out,
        "{vis}Function @{name} ({}){ret}",
        param_strs.join(", ")
    )
    .unwrap();

    // Contracts
    for pre in &func.pre_contracts {
        write!(out, "  pre(").unwrap();
        dump_expr_inline(out, pre.condition, arena, interner);
        writeln!(out, ")").unwrap();
    }
    for post in &func.post_contracts {
        write!(out, "  post(").unwrap();
        dump_expr_inline(out, post.condition, arena, interner);
        writeln!(out, ")").unwrap();
    }

    // Body
    dump_expr(out, func.body, arena, interner, 1);
}
