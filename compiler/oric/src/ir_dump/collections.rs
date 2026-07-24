//! Aggregate-literal rendering for typed-IR expression dumps.
//!
//! The collection / struct `ExprKind` arms (`List`, `Tuple`, `Map`, `Struct`,
//! and their spread variants) of the `ir_dump` expression walker. Each renders
//! the aggregate header then recurses into its children via
//! [`super::expr::dump_expr`]; the caller precomputes the `indent` + `ty`
//! annotation and threads them in so each helper renders without recomputing.

#![expect(clippy::unwrap_used, reason = "write! to String is infallible")]

use std::fmt::Write;

use ori_ir::ast::StructLitField;
use ori_ir::{
    ExprRange, FieldInitRange, ListElementRange, MapElementRange, MapEntryRange, ParsedTypeId,
    StructLitFieldRange,
};

use super::expr::dump_expr;
use super::DumpCtx;

/// Render a `List` literal: `List [N items]` then each element.
pub(super) fn dump_list(
    out: &mut String,
    items: ExprRange,
    ctx: &DumpCtx,
    depth: usize,
    indent: &str,
    ty: &str,
) {
    let DumpCtx { arena, .. } = *ctx;
    let exprs = arena.get_expr_list(items);
    writeln!(out, "{indent}List [{} items]{ty}", exprs.len()).unwrap();
    for item_id in exprs {
        dump_expr(out, *item_id, ctx, depth + 1);
    }
}

/// Render a `Tuple` literal: `Tuple (N items)` then each element.
pub(super) fn dump_tuple(
    out: &mut String,
    items: ExprRange,
    ctx: &DumpCtx,
    depth: usize,
    indent: &str,
    ty: &str,
) {
    let DumpCtx { arena, .. } = *ctx;
    let exprs = arena.get_expr_list(items);
    writeln!(out, "{indent}Tuple ({} items){ty}", exprs.len()).unwrap();
    for item_id in exprs {
        dump_expr(out, *item_id, ctx, depth + 1);
    }
}

/// Render a `Map` literal: `Map {N entries}` then each key/value pair.
pub(super) fn dump_map(
    out: &mut String,
    entries: MapEntryRange,
    ctx: &DumpCtx,
    depth: usize,
    indent: &str,
    ty: &str,
) {
    let DumpCtx { arena, .. } = *ctx;
    let entries_list = arena.get_map_entries(entries);
    writeln!(out, "{indent}Map {{{} entries}}{ty}", entries_list.len()).unwrap();
    for entry in entries_list {
        dump_expr(out, entry.key, ctx, depth + 1);
        dump_expr(out, entry.value, ctx, depth + 1);
    }
}

/// Render a `Struct` literal: `Struct Name` then each field init / shorthand.
pub(super) fn dump_struct(
    out: &mut String,
    type_path: ParsedTypeId,
    fields: FieldInitRange,
    ctx: &DumpCtx,
    depth: usize,
    indent: &str,
    ty: &str,
) {
    let DumpCtx {
        arena, interner, ..
    } = *ctx;
    let sname = crate::ast_dump::patterns::format_parsed_type(
        arena.get_parsed_type(type_path),
        arena,
        interner,
    );
    writeln!(out, "{indent}Struct {sname}{ty}").unwrap();
    for init in arena.get_field_inits(fields) {
        let fname = interner.lookup(init.name);
        if let Some(val) = init.value {
            writeln!(out, "{indent}  {fname}:").unwrap();
            dump_expr(out, val, ctx, depth + 2);
        } else {
            writeln!(out, "{indent}  {fname} (shorthand)").unwrap();
        }
    }
}

/// Render a `StructWithSpread` literal: each explicit field or `...` spread.
pub(super) fn dump_struct_with_spread(
    out: &mut String,
    type_path: ParsedTypeId,
    fields: StructLitFieldRange,
    ctx: &DumpCtx,
    depth: usize,
    indent: &str,
    ty: &str,
) {
    let DumpCtx {
        arena, interner, ..
    } = *ctx;
    let sname = crate::ast_dump::patterns::format_parsed_type(
        arena.get_parsed_type(type_path),
        arena,
        interner,
    );
    writeln!(out, "{indent}StructWithSpread {sname}{ty}").unwrap();
    for field in arena.get_struct_lit_fields(fields) {
        match field {
            StructLitField::Field(init) => {
                let fname = interner.lookup(init.name);
                writeln!(out, "{indent}  {fname}:").unwrap();
                if let Some(val) = init.value {
                    dump_expr(out, val, ctx, depth + 2);
                }
            }
            StructLitField::Spread { expr, .. } => {
                writeln!(out, "{indent}  ...").unwrap();
                dump_expr(out, *expr, ctx, depth + 2);
            }
        }
    }
}

/// Render a `ListWithSpread` literal: each element or `...` spread.
pub(super) fn dump_list_with_spread(
    out: &mut String,
    elements: ListElementRange,
    ctx: &DumpCtx,
    depth: usize,
    indent: &str,
    ty: &str,
) {
    let DumpCtx { arena, .. } = *ctx;
    writeln!(out, "{indent}ListWithSpread{ty}").unwrap();
    for elem in arena.get_list_elements(elements) {
        match elem {
            ori_ir::ast::ListElement::Expr { expr, .. } => {
                dump_expr(out, *expr, ctx, depth + 1);
            }
            ori_ir::ast::ListElement::Spread { expr, .. } => {
                writeln!(out, "{indent}  ...").unwrap();
                dump_expr(out, *expr, ctx, depth + 2);
            }
        }
    }
}

/// Render a `MapWithSpread` literal: each entry or `...` spread.
pub(super) fn dump_map_with_spread(
    out: &mut String,
    elements: MapElementRange,
    ctx: &DumpCtx,
    depth: usize,
    indent: &str,
    ty: &str,
) {
    let DumpCtx { arena, .. } = *ctx;
    writeln!(out, "{indent}MapWithSpread{ty}").unwrap();
    for elem in arena.get_map_elements(elements) {
        match elem {
            ori_ir::ast::MapElement::Entry(entry) => {
                dump_expr(out, entry.key, ctx, depth + 1);
                dump_expr(out, entry.value, ctx, depth + 1);
            }
            ori_ir::ast::MapElement::Spread { expr, .. } => {
                writeln!(out, "{indent}  ...").unwrap();
                dump_expr(out, *expr, ctx, depth + 2);
            }
        }
    }
}
