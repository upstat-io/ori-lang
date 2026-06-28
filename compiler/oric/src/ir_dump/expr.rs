//! Expression and statement dumping for typed IR phase dumps.
//!
//! Contains the core dispatch table for all `ExprKind` variants, annotating
//! each node with its resolved type from the type checker.

use std::fmt::Write;

use ori_ir::ast::{ExprKind, StmtKind};
use ori_ir::{ExprId, Name};

use super::type_annot::{dispatch_hint, type_of};
use super::{collections, DumpCtx};
use crate::ast_dump::patterns::{dump_binding_pattern, dump_match_pattern, format_label};

/// Dump an expression with indentation and type annotations.
///
/// Each node shows its `ExprKind` variant followed by ` : resolved_type`.
/// Child expressions are recursively dumped at increased depth.
#[expect(
    clippy::too_many_lines,
    reason = "dispatch table over ~30 ExprKind variants"
)]
#[expect(clippy::unwrap_used, reason = "write! to String is infallible")]
pub(super) fn dump_expr(out: &mut String, id: ExprId, ctx: &DumpCtx, depth: usize) {
    if !id.is_present() {
        return;
    }
    let DumpCtx {
        arena, interner, ..
    } = *ctx;
    let indent = "  ".repeat(depth);
    let ty = type_of(id, ctx);
    let kind = arena.expr_kind(id);

    match kind {
        // Literals
        ExprKind::Int(n) => writeln!(out, "{indent}Int({n}){ty}"),
        ExprKind::Float(bits) => writeln!(out, "{indent}Float({bits:?}){ty}"),
        ExprKind::Bool(b) => writeln!(out, "{indent}Bool({b}){ty}"),
        ExprKind::String(s) => {
            let val = interner.lookup(*s);
            writeln!(out, "{indent}String(\"{val}\"){ty}")
        }
        ExprKind::Char(c) => writeln!(out, "{indent}Char('{c}'){ty}"),
        ExprKind::Unit => writeln!(out, "{indent}Unit{ty}"),
        ExprKind::None => writeln!(out, "{indent}None{ty}"),
        ExprKind::Duration { value, unit } => {
            writeln!(out, "{indent}Duration({value}{unit:?}){ty}")
        }
        ExprKind::Size { value, unit } => writeln!(out, "{indent}Size({value}{unit:?}){ty}"),

        // Identifiers
        ExprKind::Ident(name) => {
            writeln!(out, "{indent}Ident({}){ty}", interner.lookup(*name))
        }
        ExprKind::Const(name) => {
            writeln!(out, "{indent}Const(${}){ty}", interner.lookup(*name))
        }
        ExprKind::SelfRef => writeln!(out, "{indent}SelfRef{ty}"),
        ExprKind::FunctionRef(name) => {
            writeln!(out, "{indent}FunctionRef(@{}){ty}", interner.lookup(*name))
        }
        ExprKind::HashLength => writeln!(out, "{indent}HashLength{ty}"),

        // Binary / Unary
        ExprKind::Binary { op, left, right } => {
            writeln!(out, "{indent}Binary({}){ty}", op.as_symbol()).unwrap();
            dump_expr(out, *left, ctx, depth + 1);
            dump_expr(out, *right, ctx, depth + 1);
            return;
        }
        ExprKind::Unary { op, operand } => {
            writeln!(out, "{indent}Unary({}){ty}", op.as_symbol()).unwrap();
            dump_expr(out, *operand, ctx, depth + 1);
            return;
        }

        // Calls
        ExprKind::Call { func, args } => {
            writeln!(out, "{indent}Call{ty}").unwrap();
            dump_expr(out, *func, ctx, depth + 1);
            for arg_id in arena.get_expr_list(*args) {
                dump_expr(out, *arg_id, ctx, depth + 1);
            }
            return;
        }
        ExprKind::CallNamed { func, args } => {
            writeln!(out, "{indent}CallNamed{ty}").unwrap();
            dump_expr(out, *func, ctx, depth + 1);
            for arg in arena.get_call_args(*args) {
                let label = arg
                    .name
                    .filter(|n| *n != Name::EMPTY)
                    .map(|n| format!("{}:", interner.lookup(n)))
                    .unwrap_or_default();
                writeln!(out, "{indent}  Arg {label}").unwrap();
                dump_expr(out, arg.value, ctx, depth + 2);
            }
            return;
        }

        // Method calls — show dispatch classification
        ExprKind::MethodCall {
            receiver,
            method,
            args,
        } => {
            let method_name = interner.lookup(*method);
            let hint = dispatch_hint(*receiver, ctx);
            writeln!(out, "{indent}MethodCall .{method_name}(){ty}{hint}").unwrap();
            dump_expr(out, *receiver, ctx, depth + 1);
            for arg_id in arena.get_expr_list(*args) {
                dump_expr(out, *arg_id, ctx, depth + 1);
            }
            return;
        }
        ExprKind::MethodCallNamed {
            receiver,
            method,
            args,
        } => {
            let method_name = interner.lookup(*method);
            let hint = dispatch_hint(*receiver, ctx);
            writeln!(out, "{indent}MethodCallNamed .{method_name}(){ty}{hint}").unwrap();
            dump_expr(out, *receiver, ctx, depth + 1);
            for arg in arena.get_call_args(*args) {
                let label = arg
                    .name
                    .filter(|n| *n != Name::EMPTY)
                    .map(|n| format!("{}:", interner.lookup(n)))
                    .unwrap_or_default();
                writeln!(out, "{indent}  Arg {label}").unwrap();
                dump_expr(out, arg.value, ctx, depth + 2);
            }
            return;
        }

        // Field / Index
        ExprKind::Field { receiver, field } => {
            let field_name = interner.lookup(*field);
            writeln!(out, "{indent}Field .{field_name}{ty}").unwrap();
            dump_expr(out, *receiver, ctx, depth + 1);
            return;
        }
        ExprKind::Index { receiver, index } => {
            writeln!(out, "{indent}Index{ty}").unwrap();
            dump_expr(out, *receiver, ctx, depth + 1);
            dump_expr(out, *index, ctx, depth + 1);
            return;
        }

        // Control flow
        ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => {
            writeln!(out, "{indent}If{ty}").unwrap();
            dump_expr(out, *cond, ctx, depth + 1);
            writeln!(out, "{indent}  Then").unwrap();
            dump_expr(out, *then_branch, ctx, depth + 2);
            if else_branch.is_present() {
                writeln!(out, "{indent}  Else").unwrap();
                dump_expr(out, *else_branch, ctx, depth + 2);
            }
            return;
        }
        ExprKind::Match { scrutinee, arms } => {
            writeln!(out, "{indent}Match{ty}").unwrap();
            dump_expr(out, *scrutinee, ctx, depth + 1);
            for arm in arena.get_arms(*arms) {
                write!(out, "{indent}  Arm ").unwrap();
                dump_match_pattern(out, &arm.pattern, arena, interner);
                writeln!(out).unwrap();
                if let Some(guard) = arm.guard {
                    writeln!(out, "{indent}    Guard").unwrap();
                    dump_expr(out, guard, ctx, depth + 3);
                }
                dump_expr(out, arm.body, ctx, depth + 2);
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
            writeln!(out, " in{ty}").unwrap();
            dump_expr(out, *iter, ctx, depth + 1);
            if guard.is_present() {
                writeln!(out, "{indent}  Guard").unwrap();
                dump_expr(out, *guard, ctx, depth + 2);
            }
            dump_expr(out, *body, ctx, depth + 1);
            return;
        }
        ExprKind::Loop { label, body } => {
            let label_str = format_label(*label, interner);
            writeln!(out, "{indent}Loop{label_str}{ty}").unwrap();
            dump_expr(out, *body, ctx, depth + 1);
            return;
        }
        ExprKind::While { label, cond, body } => {
            let label_str = format_label(*label, interner);
            writeln!(out, "{indent}While{label_str}{ty}").unwrap();
            dump_expr(out, *cond, ctx, depth + 1);
            dump_expr(out, *body, ctx, depth + 1);
            return;
        }
        ExprKind::Break { label, value } => {
            let label_str = format_label(*label, interner);
            writeln!(out, "{indent}Break{label_str}{ty}").unwrap();
            if value.is_present() {
                dump_expr(out, *value, ctx, depth + 1);
            }
            return;
        }
        ExprKind::Continue { label, value } => {
            let label_str = format_label(*label, interner);
            writeln!(out, "{indent}Continue{label_str}{ty}").unwrap();
            if value.is_present() {
                dump_expr(out, *value, ctx, depth + 1);
            }
            return;
        }

        // Blocks and bindings
        ExprKind::Block { stmts, result } => {
            writeln!(out, "{indent}Block{ty}").unwrap();
            dump_stmts(out, *stmts, ctx, depth + 1);
            if result.is_present() {
                dump_expr(out, *result, ctx, depth + 1);
            }
            return;
        }
        ExprKind::Let {
            pattern,
            ty: _,
            init,
            mutable: _,
        } => {
            // Mutability is shown per-binding by dump_binding_pattern (e.g., $x vs x)
            let init_ty = type_of(*init, ctx);
            write!(out, "{indent}Let ").unwrap();
            dump_binding_pattern(out, arena.get_binding_pattern(*pattern), interner);
            writeln!(out, "{init_ty} =").unwrap();
            dump_expr(out, *init, ctx, depth + 1);
            return;
        }
        ExprKind::Lambda {
            params,
            ret_ty: _,
            body,
        } => {
            let param_list: Vec<String> = arena
                .get_params(*params)
                .iter()
                .map(|p| interner.lookup(p.name).to_string())
                .collect();
            writeln!(out, "{indent}Lambda ({}){ty}", param_list.join(", ")).unwrap();
            dump_expr(out, *body, ctx, depth + 1);
            return;
        }

        // Collections (aggregate-literal rendering lives in `collections`)
        ExprKind::List(items) => {
            collections::dump_list(out, *items, ctx, depth, &indent, &ty);
            return;
        }
        ExprKind::Tuple(items) => {
            collections::dump_tuple(out, *items, ctx, depth, &indent, &ty);
            return;
        }
        ExprKind::Map(entries) => {
            collections::dump_map(out, *entries, ctx, depth, &indent, &ty);
            return;
        }
        ExprKind::Struct { name, fields } => {
            collections::dump_struct(out, *name, *fields, ctx, depth, &indent, &ty);
            return;
        }
        ExprKind::StructWithSpread { name, fields } => {
            collections::dump_struct_with_spread(out, *name, *fields, ctx, depth, &indent, &ty);
            return;
        }
        ExprKind::ListWithSpread(elements) => {
            collections::dump_list_with_spread(out, *elements, ctx, depth, &indent, &ty);
            return;
        }
        ExprKind::MapWithSpread(elements) => {
            collections::dump_map_with_spread(out, *elements, ctx, depth, &indent, &ty);
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
            writeln!(out, "{indent}Range({kind}){ty}").unwrap();
            if start.is_present() {
                dump_expr(out, *start, ctx, depth + 1);
            }
            if end.is_present() {
                dump_expr(out, *end, ctx, depth + 1);
            }
            if step.is_present() {
                writeln!(out, "{indent}  step:").unwrap();
                dump_expr(out, *step, ctx, depth + 2);
            }
            return;
        }

        // Result / Option
        ExprKind::Ok(inner) => {
            if inner.is_present() {
                writeln!(out, "{indent}Ok{ty}").unwrap();
                dump_expr(out, *inner, ctx, depth + 1);
            } else {
                writeln!(out, "{indent}Ok(()){ty}").unwrap();
            }
            return;
        }
        ExprKind::Err(inner) => {
            if inner.is_present() {
                writeln!(out, "{indent}Err{ty}").unwrap();
                dump_expr(out, *inner, ctx, depth + 1);
            } else {
                writeln!(out, "{indent}Err(()){ty}").unwrap();
            }
            return;
        }
        ExprKind::Some(inner) => {
            writeln!(out, "{indent}Some{ty}").unwrap();
            dump_expr(out, *inner, ctx, depth + 1);
            return;
        }

        // Type operations
        ExprKind::Cast {
            expr,
            ty: cast_ty,
            fallible,
            ..
        } => {
            let keyword = if *fallible { "as?" } else { "as" };
            let parsed_ty = arena.get_parsed_type(*cast_ty);
            writeln!(
                out,
                "{indent}Cast({keyword} {}){ty}",
                crate::ast_dump::patterns::format_parsed_type(parsed_ty, arena, interner)
            )
            .unwrap();
            dump_expr(out, *expr, ctx, depth + 1);
            return;
        }
        ExprKind::Try(inner) => {
            writeln!(out, "{indent}Try(?){ty}").unwrap();
            dump_expr(out, *inner, ctx, depth + 1);
            return;
        }
        ExprKind::Unsafe(inner) => {
            writeln!(out, "{indent}Unsafe{ty}").unwrap();
            dump_expr(out, *inner, ctx, depth + 1);
            return;
        }
        ExprKind::Await(inner) => {
            writeln!(out, "{indent}Await{ty}").unwrap();
            dump_expr(out, *inner, ctx, depth + 1);
            return;
        }
        ExprKind::Assign { target, value } => {
            writeln!(out, "{indent}Assign{ty}").unwrap();
            dump_expr(out, *target, ctx, depth + 1);
            dump_expr(out, *value, ctx, depth + 1);
            return;
        }
        ExprKind::AssignTarget { root, steps } => {
            writeln!(out, "{indent}AssignTarget{ty}").unwrap();
            dump_expr(out, *root, ctx, depth + 1);
            for step in arena.get_access_steps(*steps) {
                match step {
                    ori_ir::AccessStep::Field(field) => {
                        let field_name = interner.lookup(*field);
                        writeln!(out, "{indent}  .{field_name}").unwrap();
                    }
                    ori_ir::AccessStep::Index(index) => {
                        dump_expr(out, *index, ctx, depth + 1);
                    }
                }
            }
            return;
        }

        // Capabilities
        ExprKind::WithCapability {
            capability,
            provider,
            body,
        } => {
            let cap_name = interner.lookup(*capability);
            writeln!(out, "{indent}WithCapability {cap_name}{ty}").unwrap();
            writeln!(out, "{indent}  provider:").unwrap();
            dump_expr(out, *provider, ctx, depth + 2);
            writeln!(out, "{indent}  body:").unwrap();
            dump_expr(out, *body, ctx, depth + 2);
            return;
        }

        // Templates
        ExprKind::TemplateFull(s) => {
            let val = interner.lookup(*s);
            writeln!(out, "{indent}Template(`{val}`){ty}")
        }
        ExprKind::TemplateLiteral { head, parts } => {
            let head_str = interner.lookup(*head);
            writeln!(out, "{indent}TemplateLiteral(`{head_str}...`){ty}").unwrap();
            for part in arena.get_template_parts(*parts) {
                dump_expr(out, part.expr, ctx, depth + 1);
            }
            return;
        }

        // Function patterns
        ExprKind::FunctionSeq(id) => {
            let seq = arena.get_function_seq(*id);
            match seq {
                ori_ir::ast::FunctionSeq::Try { .. } => {
                    writeln!(out, "{indent}FunctionSeq(try){ty}")
                }
                ori_ir::ast::FunctionSeq::Match { .. } => {
                    writeln!(out, "{indent}FunctionSeq(match){ty}")
                }
                ori_ir::ast::FunctionSeq::ForPattern { .. } => {
                    writeln!(out, "{indent}FunctionSeq(for_pattern){ty}")
                }
            }
        }
        ExprKind::FunctionExp(id) => {
            let exp = arena.get_function_exp(*id);
            writeln!(out, "{indent}FunctionExp({:?}){ty}", exp.kind)
        }

        ExprKind::Error => writeln!(out, "{indent}Error"),
    }
    .unwrap();
}

/// Dump statements in a block with type annotations.
#[expect(clippy::unwrap_used, reason = "write! to String is infallible")]
fn dump_stmts(out: &mut String, stmts: ori_ir::StmtRange, ctx: &DumpCtx, depth: usize) {
    let DumpCtx {
        arena, interner, ..
    } = *ctx;
    let indent = "  ".repeat(depth);
    for stmt in arena.get_stmt_range(stmts) {
        match &stmt.kind {
            StmtKind::Expr(id) => dump_expr(out, *id, ctx, depth),
            StmtKind::Let {
                pattern,
                ty: _,
                init,
                mutable: _,
            } => {
                // Mutability is shown per-binding by dump_binding_pattern
                let init_ty = type_of(*init, ctx);
                write!(out, "{indent}Let ").unwrap();
                dump_binding_pattern(out, arena.get_binding_pattern(*pattern), interner);
                writeln!(out, "{init_ty} =").unwrap();
                dump_expr(out, *init, ctx, depth + 1);
            }
        }
    }
}
