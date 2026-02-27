//! Expression and statement dumping for AST phase dumps.
//!
//! Contains the core dispatch table for all `ExprKind` variants.

use std::fmt::Write;

use ori_ir::ast::{ExprKind, Mutability, Stmt, StmtKind, StructLitField};
use ori_ir::{ExprArena, ExprId, Name, StringInterner};

use super::patterns::{dump_binding_pattern, dump_match_pattern, format_label, format_parsed_type};

/// Dump an expression with indentation.
///
/// This is a dispatch table over all `ExprKind` variants — each arm formats
/// its variant and recursively dumps child expressions at increased depth.
#[expect(
    clippy::too_many_lines,
    reason = "dispatch table over ~30 ExprKind variants"
)]
#[expect(clippy::unwrap_used, reason = "write! to String is infallible")]
pub(super) fn dump_expr(
    out: &mut String,
    id: ExprId,
    arena: &ExprArena,
    interner: &StringInterner,
    depth: usize,
) {
    if !id.is_present() {
        return;
    }
    let indent = "  ".repeat(depth);
    let kind = arena.expr_kind(id);
    match kind {
        // Literals
        ExprKind::Int(n) => writeln!(out, "{indent}Int({n})"),
        ExprKind::Float(bits) => {
            writeln!(out, "{indent}Float({bits:?})")
        }
        ExprKind::Bool(b) => writeln!(out, "{indent}Bool({b})"),
        ExprKind::String(s) => {
            let val = interner.lookup(*s);
            writeln!(out, "{indent}String(\"{val}\")")
        }
        ExprKind::Char(c) => writeln!(out, "{indent}Char('{c}')"),
        ExprKind::Unit => writeln!(out, "{indent}Unit"),
        ExprKind::None => writeln!(out, "{indent}None"),
        ExprKind::Duration { value, unit } => {
            writeln!(out, "{indent}Duration({value}{unit:?})")
        }
        ExprKind::Size { value, unit } => {
            writeln!(out, "{indent}Size({value}{unit:?})")
        }

        // Identifiers
        ExprKind::Ident(name) => writeln!(out, "{indent}Ident({})", interner.lookup(*name)),
        ExprKind::Const(name) => writeln!(out, "{indent}Const(${})", interner.lookup(*name)),
        ExprKind::SelfRef => writeln!(out, "{indent}SelfRef"),
        ExprKind::FunctionRef(name) => {
            writeln!(out, "{indent}FunctionRef(@{})", interner.lookup(*name))
        }
        ExprKind::HashLength => writeln!(out, "{indent}HashLength"),

        // Binary / Unary
        ExprKind::Binary { op, left, right } => {
            writeln!(out, "{indent}Binary({})", op.as_symbol()).unwrap();
            dump_expr(out, *left, arena, interner, depth + 1);
            dump_expr(out, *right, arena, interner, depth + 1);
            return;
        }
        ExprKind::Unary { op, operand } => {
            writeln!(out, "{indent}Unary({})", op.as_symbol()).unwrap();
            dump_expr(out, *operand, arena, interner, depth + 1);
            return;
        }

        // Calls
        ExprKind::Call { func, args } => {
            writeln!(out, "{indent}Call").unwrap();
            dump_expr(out, *func, arena, interner, depth + 1);
            for arg_id in arena.get_expr_list(*args) {
                dump_expr(out, *arg_id, arena, interner, depth + 1);
            }
            return;
        }
        ExprKind::CallNamed { func, args } => {
            writeln!(out, "{indent}CallNamed").unwrap();
            dump_expr(out, *func, arena, interner, depth + 1);
            for arg in arena.get_call_args(*args) {
                let label = arg
                    .name
                    .filter(|n| *n != Name::EMPTY)
                    .map(|n| format!("{}:", interner.lookup(n)))
                    .unwrap_or_default();
                writeln!(out, "{indent}  Arg {label}").unwrap();
                dump_expr(out, arg.value, arena, interner, depth + 2);
            }
            return;
        }

        // Method calls
        ExprKind::MethodCall {
            receiver,
            method,
            args,
        } => {
            let method_name = interner.lookup(*method);
            writeln!(out, "{indent}MethodCall .{method_name}()").unwrap();
            dump_expr(out, *receiver, arena, interner, depth + 1);
            for arg_id in arena.get_expr_list(*args) {
                dump_expr(out, *arg_id, arena, interner, depth + 1);
            }
            return;
        }
        ExprKind::MethodCallNamed {
            receiver,
            method,
            args,
        } => {
            let method_name = interner.lookup(*method);
            writeln!(out, "{indent}MethodCallNamed .{method_name}()").unwrap();
            dump_expr(out, *receiver, arena, interner, depth + 1);
            for arg in arena.get_call_args(*args) {
                let label = arg
                    .name
                    .filter(|n| *n != Name::EMPTY)
                    .map(|n| format!("{}:", interner.lookup(n)))
                    .unwrap_or_default();
                writeln!(out, "{indent}  Arg {label}").unwrap();
                dump_expr(out, arg.value, arena, interner, depth + 2);
            }
            return;
        }

        // Field / Index
        ExprKind::Field { receiver, field } => {
            let field_name = interner.lookup(*field);
            writeln!(out, "{indent}Field .{field_name}").unwrap();
            dump_expr(out, *receiver, arena, interner, depth + 1);
            return;
        }
        ExprKind::Index { receiver, index } => {
            writeln!(out, "{indent}Index").unwrap();
            dump_expr(out, *receiver, arena, interner, depth + 1);
            dump_expr(out, *index, arena, interner, depth + 1);
            return;
        }

        // Control flow
        ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => {
            writeln!(out, "{indent}If").unwrap();
            dump_expr(out, *cond, arena, interner, depth + 1);
            writeln!(out, "{indent}  Then").unwrap();
            dump_expr(out, *then_branch, arena, interner, depth + 2);
            if else_branch.is_present() {
                writeln!(out, "{indent}  Else").unwrap();
                dump_expr(out, *else_branch, arena, interner, depth + 2);
            }
            return;
        }
        ExprKind::Match { scrutinee, arms } => {
            writeln!(out, "{indent}Match").unwrap();
            dump_expr(out, *scrutinee, arena, interner, depth + 1);
            for arm in arena.get_arms(*arms) {
                write!(out, "{indent}  Arm ").unwrap();
                dump_match_pattern(out, &arm.pattern, arena, interner);
                writeln!(out).unwrap();
                if let Some(guard) = arm.guard {
                    writeln!(out, "{indent}    Guard").unwrap();
                    dump_expr(out, guard, arena, interner, depth + 3);
                }
                dump_expr(out, arm.body, arena, interner, depth + 2);
            }
            return;
        }
        ExprKind::For {
            label,
            pattern,
            iter,
            guard,
            body,
            is_yield,
        } => {
            let label_str = format_label(*label, interner);
            let yield_str = if *is_yield { " yield" } else { "" };
            write!(out, "{indent}For{label_str}{yield_str} ").unwrap();
            dump_binding_pattern(out, arena.get_binding_pattern(*pattern), interner);
            writeln!(out, " in").unwrap();
            dump_expr(out, *iter, arena, interner, depth + 1);
            if guard.is_present() {
                writeln!(out, "{indent}  Guard").unwrap();
                dump_expr(out, *guard, arena, interner, depth + 2);
            }
            dump_expr(out, *body, arena, interner, depth + 1);
            return;
        }
        ExprKind::Loop { label, body } => {
            let label_str = format_label(*label, interner);
            writeln!(out, "{indent}Loop{label_str}").unwrap();
            dump_expr(out, *body, arena, interner, depth + 1);
            return;
        }
        ExprKind::Break { label, value } => {
            let label_str = format_label(*label, interner);
            writeln!(out, "{indent}Break{label_str}").unwrap();
            if value.is_present() {
                dump_expr(out, *value, arena, interner, depth + 1);
            }
            return;
        }
        ExprKind::Continue { label, value } => {
            let label_str = format_label(*label, interner);
            writeln!(out, "{indent}Continue{label_str}").unwrap();
            if value.is_present() {
                dump_expr(out, *value, arena, interner, depth + 1);
            }
            return;
        }

        // Blocks and bindings
        ExprKind::Block { stmts, result } => {
            writeln!(out, "{indent}Block").unwrap();
            dump_stmts(out, *stmts, arena, interner, depth + 1);
            if result.is_present() {
                dump_expr(out, *result, arena, interner, depth + 1);
            }
            return;
        }
        ExprKind::Let {
            pattern,
            ty,
            init,
            mutable,
        } => {
            let mut_str = match mutable {
                Mutability::Immutable => "$",
                Mutability::Mutable => "",
            };
            write!(out, "{indent}Let {mut_str}").unwrap();
            dump_binding_pattern(out, arena.get_binding_pattern(*pattern), interner);
            if ty.is_valid() {
                let parsed_ty = arena.get_parsed_type(*ty);
                write!(out, ": {}", format_parsed_type(parsed_ty, arena, interner)).unwrap();
            }
            writeln!(out, " =").unwrap();
            dump_expr(out, *init, arena, interner, depth + 1);
            return;
        }
        ExprKind::Lambda {
            params,
            ret_ty,
            body,
        } => {
            let param_list: Vec<String> = arena
                .get_params(*params)
                .iter()
                .map(|p| interner.lookup(p.name).to_string())
                .collect();
            let ret = if ret_ty.is_valid() {
                let ty = arena.get_parsed_type(*ret_ty);
                format!(" -> {}", format_parsed_type(ty, arena, interner))
            } else {
                String::new()
            };
            writeln!(out, "{indent}Lambda ({}){ret}", param_list.join(", ")).unwrap();
            dump_expr(out, *body, arena, interner, depth + 1);
            return;
        }

        // Collections
        ExprKind::List(items) => {
            let exprs = arena.get_expr_list(*items);
            writeln!(out, "{indent}List [{} items]", exprs.len()).unwrap();
            for item_id in exprs {
                dump_expr(out, *item_id, arena, interner, depth + 1);
            }
            return;
        }
        ExprKind::Tuple(items) => {
            let exprs = arena.get_expr_list(*items);
            writeln!(out, "{indent}Tuple ({} items)", exprs.len()).unwrap();
            for item_id in exprs {
                dump_expr(out, *item_id, arena, interner, depth + 1);
            }
            return;
        }
        ExprKind::Map(entries) => {
            let entries_list = arena.get_map_entries(*entries);
            writeln!(out, "{indent}Map {{{} entries}}", entries_list.len()).unwrap();
            for entry in entries_list {
                dump_expr(out, entry.key, arena, interner, depth + 1);
                dump_expr(out, entry.value, arena, interner, depth + 1);
            }
            return;
        }
        ExprKind::Struct { name, fields } => {
            let sname = interner.lookup(*name);
            writeln!(out, "{indent}Struct {sname}").unwrap();
            for init in arena.get_field_inits(*fields) {
                let fname = interner.lookup(init.name);
                if let Some(val) = init.value {
                    writeln!(out, "{indent}  {fname}:").unwrap();
                    dump_expr(out, val, arena, interner, depth + 2);
                } else {
                    writeln!(out, "{indent}  {fname} (shorthand)").unwrap();
                }
            }
            return;
        }
        ExprKind::StructWithSpread { name, fields } => {
            let sname = interner.lookup(*name);
            writeln!(out, "{indent}StructWithSpread {sname}").unwrap();
            for field in arena.get_struct_lit_fields(*fields) {
                match field {
                    StructLitField::Field(init) => {
                        let fname = interner.lookup(init.name);
                        writeln!(out, "{indent}  {fname}:").unwrap();
                        if let Some(val) = init.value {
                            dump_expr(out, val, arena, interner, depth + 2);
                        }
                    }
                    StructLitField::Spread { expr, .. } => {
                        writeln!(out, "{indent}  ...").unwrap();
                        dump_expr(out, *expr, arena, interner, depth + 2);
                    }
                }
            }
            return;
        }
        ExprKind::ListWithSpread(elements) => {
            writeln!(out, "{indent}ListWithSpread").unwrap();
            for elem in arena.get_list_elements(*elements) {
                match elem {
                    ori_ir::ast::ListElement::Expr { expr, .. } => {
                        dump_expr(out, *expr, arena, interner, depth + 1);
                    }
                    ori_ir::ast::ListElement::Spread { expr, .. } => {
                        writeln!(out, "{indent}  ...").unwrap();
                        dump_expr(out, *expr, arena, interner, depth + 2);
                    }
                }
            }
            return;
        }
        ExprKind::MapWithSpread(elements) => {
            writeln!(out, "{indent}MapWithSpread").unwrap();
            for elem in arena.get_map_elements(*elements) {
                match elem {
                    ori_ir::ast::MapElement::Entry(entry) => {
                        dump_expr(out, entry.key, arena, interner, depth + 1);
                        dump_expr(out, entry.value, arena, interner, depth + 1);
                    }
                    ori_ir::ast::MapElement::Spread { expr, .. } => {
                        writeln!(out, "{indent}  ...").unwrap();
                        dump_expr(out, *expr, arena, interner, depth + 2);
                    }
                }
            }
            return;
        }

        // Range
        ExprKind::Range {
            start,
            end,
            step,
            inclusive,
        } => {
            let kind = if *inclusive { "..=" } else { ".." };
            writeln!(out, "{indent}Range({kind})").unwrap();
            if start.is_present() {
                dump_expr(out, *start, arena, interner, depth + 1);
            }
            if end.is_present() {
                dump_expr(out, *end, arena, interner, depth + 1);
            }
            if step.is_present() {
                writeln!(out, "{indent}  step:").unwrap();
                dump_expr(out, *step, arena, interner, depth + 2);
            }
            return;
        }

        // Result / Option
        ExprKind::Ok(inner) => {
            if inner.is_present() {
                writeln!(out, "{indent}Ok").unwrap();
                dump_expr(out, *inner, arena, interner, depth + 1);
            } else {
                writeln!(out, "{indent}Ok(())").unwrap();
            }
            return;
        }
        ExprKind::Err(inner) => {
            if inner.is_present() {
                writeln!(out, "{indent}Err").unwrap();
                dump_expr(out, *inner, arena, interner, depth + 1);
            } else {
                writeln!(out, "{indent}Err(())").unwrap();
            }
            return;
        }
        ExprKind::Some(inner) => {
            writeln!(out, "{indent}Some").unwrap();
            dump_expr(out, *inner, arena, interner, depth + 1);
            return;
        }

        // Type operations
        ExprKind::Cast {
            expr, ty, fallible, ..
        } => {
            let keyword = if *fallible { "as?" } else { "as" };
            let parsed_ty = arena.get_parsed_type(*ty);
            writeln!(
                out,
                "{indent}Cast({keyword} {})",
                format_parsed_type(parsed_ty, arena, interner)
            )
            .unwrap();
            dump_expr(out, *expr, arena, interner, depth + 1);
            return;
        }
        ExprKind::Try(inner) => {
            writeln!(out, "{indent}Try(?)").unwrap();
            dump_expr(out, *inner, arena, interner, depth + 1);
            return;
        }
        ExprKind::Unsafe(inner) => {
            writeln!(out, "{indent}Unsafe").unwrap();
            dump_expr(out, *inner, arena, interner, depth + 1);
            return;
        }
        ExprKind::Await(inner) => {
            writeln!(out, "{indent}Await").unwrap();
            dump_expr(out, *inner, arena, interner, depth + 1);
            return;
        }
        ExprKind::Assign { target, value } => {
            writeln!(out, "{indent}Assign").unwrap();
            dump_expr(out, *target, arena, interner, depth + 1);
            dump_expr(out, *value, arena, interner, depth + 1);
            return;
        }

        // Capabilities
        ExprKind::WithCapability {
            capability,
            provider,
            body,
        } => {
            let cap_name = interner.lookup(*capability);
            writeln!(out, "{indent}WithCapability {cap_name}").unwrap();
            writeln!(out, "{indent}  provider:").unwrap();
            dump_expr(out, *provider, arena, interner, depth + 2);
            writeln!(out, "{indent}  body:").unwrap();
            dump_expr(out, *body, arena, interner, depth + 2);
            return;
        }

        // Templates
        ExprKind::TemplateFull(s) => {
            let val = interner.lookup(*s);
            writeln!(out, "{indent}Template(`{val}`)")
        }
        ExprKind::TemplateLiteral { head, parts } => {
            let head_str = interner.lookup(*head);
            writeln!(out, "{indent}TemplateLiteral(`{head_str}...`)").unwrap();
            for part in arena.get_template_parts(*parts) {
                dump_expr(out, part.expr, arena, interner, depth + 1);
            }
            return;
        }

        // Function patterns (complex pattern forms)
        ExprKind::FunctionSeq(id) => {
            let seq = arena.get_function_seq(*id);
            match seq {
                ori_ir::ast::FunctionSeq::Try { .. } => {
                    writeln!(out, "{indent}FunctionSeq(try)")
                }
                ori_ir::ast::FunctionSeq::Match { .. } => {
                    writeln!(out, "{indent}FunctionSeq(match)")
                }
                ori_ir::ast::FunctionSeq::ForPattern { .. } => {
                    writeln!(out, "{indent}FunctionSeq(for_pattern)")
                }
            }
        }
        ExprKind::FunctionExp(id) => {
            let exp = arena.get_function_exp(*id);
            writeln!(out, "{indent}FunctionExp({:?})", exp.kind)
        }

        ExprKind::Error => writeln!(out, "{indent}Error"),
    }
    .unwrap();
}

/// Dump an expression inline (single-line, no indentation).
///
/// Used for simple expressions in contexts where a tree dump would be excessive
/// (e.g., contract conditions, constant values).
#[expect(clippy::unwrap_used, reason = "write! to String is infallible")]
pub(super) fn dump_expr_inline(
    out: &mut String,
    id: ExprId,
    arena: &ExprArena,
    interner: &StringInterner,
) {
    if !id.is_present() {
        write!(out, "<none>").unwrap();
        return;
    }
    let kind = arena.expr_kind(id);
    match kind {
        ExprKind::Int(n) => write!(out, "{n}"),
        ExprKind::Float(bits) => write!(out, "Float({bits:?})"),
        ExprKind::Bool(b) => write!(out, "{b}"),
        ExprKind::String(s) => write!(out, "\"{}\"", interner.lookup(*s)),
        ExprKind::Char(c) => write!(out, "'{c}'"),
        ExprKind::Unit => write!(out, "()"),
        ExprKind::None => write!(out, "None"),
        ExprKind::Ident(n) => write!(out, "{}", interner.lookup(*n)),
        ExprKind::Const(n) => write!(out, "${}", interner.lookup(*n)),
        ExprKind::FunctionRef(n) => write!(out, "@{}", interner.lookup(*n)),
        ExprKind::Binary { op, left, right } => {
            dump_expr_inline(out, *left, arena, interner);
            write!(out, " {} ", op.as_symbol()).unwrap();
            dump_expr_inline(out, *right, arena, interner);
            return;
        }
        _ => write!(out, "<expr>"),
    }
    .unwrap();
}

/// Dump statements in a block.
fn dump_stmts(
    out: &mut String,
    stmts: ori_ir::StmtRange,
    arena: &ExprArena,
    interner: &StringInterner,
    depth: usize,
) {
    let indent = "  ".repeat(depth);
    for stmt in arena.get_stmt_range(stmts) {
        dump_stmt(out, stmt, &indent, arena, interner, depth);
    }
}

/// Dump a single statement.
#[expect(clippy::unwrap_used, reason = "write! to String is infallible")]
fn dump_stmt(
    out: &mut String,
    stmt: &Stmt,
    indent: &str,
    arena: &ExprArena,
    interner: &StringInterner,
    depth: usize,
) {
    match &stmt.kind {
        StmtKind::Expr(id) => dump_expr(out, *id, arena, interner, depth),
        StmtKind::Let {
            pattern,
            ty,
            init,
            mutable,
        } => {
            let mut_str = match mutable {
                Mutability::Immutable => "$",
                Mutability::Mutable => "",
            };
            write!(out, "{indent}Let {mut_str}").unwrap();
            dump_binding_pattern(out, arena.get_binding_pattern(*pattern), interner);
            if ty.is_valid() {
                let parsed_ty = arena.get_parsed_type(*ty);
                write!(out, ": {}", format_parsed_type(parsed_ty, arena, interner)).unwrap();
            }
            writeln!(out, " =").unwrap();
            dump_expr(out, *init, arena, interner, depth + 1);
        }
    }
}
