//! Expression and statement dumping for AST phase dumps.
//!
//! Contains the core dispatch table for all `ExprKind` variants.

use std::fmt::{self, Write};

use ori_ir::ast::{ExprKind, Mutability, Stmt, StmtKind, StructLitField};
use ori_ir::{ExprArena, ExprId, Name, StringInterner};

use super::patterns::{dump_binding_pattern, dump_match_pattern, format_label, format_parsed_type};

/// Dump an expression with indentation.
///
/// This is a dispatch table over all `ExprKind` variants — each arm formats
/// its variant and recursively dumps child expressions at increased depth.
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
        kind @ (ExprKind::Int(_)
        | ExprKind::Float(_)
        | ExprKind::Bool(_)
        | ExprKind::String(_)
        | ExprKind::Char(_)
        | ExprKind::Unit
        | ExprKind::None
        | ExprKind::Duration { .. }
        | ExprKind::Size { .. }
        | ExprKind::Ident(_)
        | ExprKind::Const(_)
        | ExprKind::SelfRef
        | ExprKind::FunctionRef(_)
        | ExprKind::HashLength) => dump_leaf(out, kind, interner, &indent),

        kind @ (ExprKind::Binary { .. } | ExprKind::Unary { .. }) => {
            dump_operator(out, kind, arena, interner, depth, &indent)
        }
        kind @ (ExprKind::Call { .. }
        | ExprKind::CallNamed { .. }
        | ExprKind::MethodCall { .. }
        | ExprKind::MethodCallNamed { .. }) => {
            dump_call(out, kind, arena, interner, depth, &indent)
        }
        kind @ (ExprKind::Field { .. } | ExprKind::Index { .. }) => {
            dump_access(out, kind, arena, interner, depth, &indent)
        }

        kind @ (ExprKind::If { .. } | ExprKind::Match { .. } | ExprKind::For { .. }) => {
            dump_branching(out, kind, arena, interner, depth, &indent)
        }
        kind @ (ExprKind::Loop { .. }
        | ExprKind::While { .. }
        | ExprKind::Break { .. }
        | ExprKind::Continue { .. }) => {
            dump_loop_control(out, kind, arena, interner, depth, &indent)
        }

        kind @ (ExprKind::Block { .. } | ExprKind::Let { .. } | ExprKind::Lambda { .. }) => {
            dump_binding(out, kind, arena, interner, depth, &indent)
        }

        kind @ (ExprKind::List(_)
        | ExprKind::Tuple(_)
        | ExprKind::Map(_)
        | ExprKind::Struct { .. }) => {
            dump_basic_collection(out, kind, arena, interner, depth, &indent)
        }
        kind @ (ExprKind::StructWithSpread { .. }
        | ExprKind::ListWithSpread(_)
        | ExprKind::MapWithSpread(_)) => {
            dump_spread_collection(out, kind, arena, interner, depth, &indent)
        }

        kind
        @ (ExprKind::Range { .. } | ExprKind::Ok(_) | ExprKind::Err(_) | ExprKind::Some(_)) => {
            dump_value_form(out, kind, arena, interner, depth, &indent)
        }

        kind @ (ExprKind::Cast { .. }
        | ExprKind::Try(_)
        | ExprKind::Unsafe(_)
        | ExprKind::Await(_)) => dump_type_operation(out, kind, arena, interner, depth, &indent),
        kind @ (ExprKind::Assign { .. }
        | ExprKind::AssignTarget { .. }
        | ExprKind::WithCapability { .. }) => {
            dump_effect(out, kind, arena, interner, depth, &indent)
        }

        kind @ (ExprKind::TemplateFull(_)
        | ExprKind::TemplateLiteral { .. }
        | ExprKind::FunctionSeq(_)
        | ExprKind::FunctionExp(_)
        | ExprKind::Error) => dump_terminal_form(out, kind, arena, interner, depth, &indent),
    }
    .unwrap();
}

fn dump_leaf(
    out: &mut String,
    kind: &ExprKind,
    interner: &StringInterner,
    indent: &str,
) -> fmt::Result {
    match kind {
        ExprKind::Int(n) => writeln!(out, "{indent}Int({n})"),
        ExprKind::Float(bits) => writeln!(out, "{indent}Float({bits:?})"),
        ExprKind::Bool(value) => writeln!(out, "{indent}Bool({value})"),
        ExprKind::String(name) => writeln!(out, "{indent}String(\"{}\")", interner.lookup(*name)),
        ExprKind::Char(value) => writeln!(out, "{indent}Char('{value}')"),
        ExprKind::Unit => writeln!(out, "{indent}Unit"),
        ExprKind::None => writeln!(out, "{indent}None"),
        ExprKind::Duration { value, unit } => writeln!(out, "{indent}Duration({value}{unit:?})"),
        ExprKind::Size { value, unit } => writeln!(out, "{indent}Size({value}{unit:?})"),
        ExprKind::Ident(name) => writeln!(out, "{indent}Ident({})", interner.lookup(*name)),
        ExprKind::Const(name) => writeln!(out, "{indent}Const(${})", interner.lookup(*name)),
        ExprKind::SelfRef => writeln!(out, "{indent}SelfRef"),
        ExprKind::FunctionRef(name) => {
            writeln!(out, "{indent}FunctionRef(@{})", interner.lookup(*name))
        }
        ExprKind::HashLength => writeln!(out, "{indent}HashLength"),
        _ => unreachable!("leaf dumper called with non-leaf expression"),
    }
}

fn dump_operator(
    out: &mut String,
    kind: &ExprKind,
    arena: &ExprArena,
    interner: &StringInterner,
    depth: usize,
    indent: &str,
) -> fmt::Result {
    match kind {
        ExprKind::Binary { op, left, right } => {
            writeln!(out, "{indent}Binary({})", op.as_symbol())?;
            dump_expr(out, *left, arena, interner, depth + 1);
            dump_expr(out, *right, arena, interner, depth + 1);
        }
        ExprKind::Unary { op, operand } => {
            writeln!(out, "{indent}Unary({})", op.as_symbol())?;
            dump_expr(out, *operand, arena, interner, depth + 1);
        }
        _ => unreachable!("operator dumper called with non-operator expression"),
    }
    Ok(())
}

fn dump_call(
    out: &mut String,
    kind: &ExprKind,
    arena: &ExprArena,
    interner: &StringInterner,
    depth: usize,
    indent: &str,
) -> fmt::Result {
    match kind {
        ExprKind::Call { func, args } => {
            writeln!(out, "{indent}Call")?;
            dump_expr(out, *func, arena, interner, depth + 1);
            for arg in arena.get_expr_list(*args) {
                dump_expr(out, *arg, arena, interner, depth + 1);
            }
        }
        ExprKind::CallNamed { func, args } => {
            writeln!(out, "{indent}CallNamed")?;
            dump_expr(out, *func, arena, interner, depth + 1);
            dump_named_args(out, *args, arena, interner, depth, indent)?;
        }
        ExprKind::MethodCall {
            receiver,
            method,
            args,
        } => {
            writeln!(out, "{indent}MethodCall .{}()", interner.lookup(*method))?;
            dump_expr(out, *receiver, arena, interner, depth + 1);
            for arg in arena.get_expr_list(*args) {
                dump_expr(out, *arg, arena, interner, depth + 1);
            }
        }
        ExprKind::MethodCallNamed {
            receiver,
            method,
            args,
        } => {
            writeln!(
                out,
                "{indent}MethodCallNamed .{}()",
                interner.lookup(*method)
            )?;
            dump_expr(out, *receiver, arena, interner, depth + 1);
            dump_named_args(out, *args, arena, interner, depth, indent)?;
        }
        _ => unreachable!("call dumper called with non-call expression"),
    }
    Ok(())
}

fn dump_named_args(
    out: &mut String,
    args: ori_ir::CallArgRange,
    arena: &ExprArena,
    interner: &StringInterner,
    depth: usize,
    indent: &str,
) -> fmt::Result {
    for arg in arena.get_call_args(args) {
        let label = arg
            .name
            .filter(|name| *name != Name::EMPTY)
            .map(|name| format!("{}:", interner.lookup(name)))
            .unwrap_or_default();
        writeln!(out, "{indent}  Arg {label}")?;
        dump_expr(out, arg.value, arena, interner, depth + 2);
    }
    Ok(())
}

fn dump_access(
    out: &mut String,
    kind: &ExprKind,
    arena: &ExprArena,
    interner: &StringInterner,
    depth: usize,
    indent: &str,
) -> fmt::Result {
    match kind {
        ExprKind::Field { receiver, field } => {
            writeln!(out, "{indent}Field .{}", interner.lookup(*field))?;
            dump_expr(out, *receiver, arena, interner, depth + 1);
        }
        ExprKind::Index { receiver, index } => {
            writeln!(out, "{indent}Index")?;
            dump_expr(out, *receiver, arena, interner, depth + 1);
            dump_expr(out, *index, arena, interner, depth + 1);
        }
        _ => unreachable!("access dumper called with non-access expression"),
    }
    Ok(())
}

fn dump_branching(
    out: &mut String,
    kind: &ExprKind,
    arena: &ExprArena,
    interner: &StringInterner,
    depth: usize,
    indent: &str,
) -> fmt::Result {
    match kind {
        ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => {
            writeln!(out, "{indent}If")?;
            dump_expr(out, *cond, arena, interner, depth + 1);
            writeln!(out, "{indent}  Then")?;
            dump_expr(out, *then_branch, arena, interner, depth + 2);
            if else_branch.is_present() {
                writeln!(out, "{indent}  Else")?;
                dump_expr(out, *else_branch, arena, interner, depth + 2);
            }
        }
        ExprKind::Match { scrutinee, arms } => {
            writeln!(out, "{indent}Match")?;
            dump_expr(out, *scrutinee, arena, interner, depth + 1);
            for arm in arena.get_arms(*arms) {
                write!(out, "{indent}  Arm ")?;
                dump_match_pattern(out, &arm.pattern, arena, interner);
                writeln!(out)?;
                if let Some(guard) = arm.guard {
                    writeln!(out, "{indent}    Guard")?;
                    dump_expr(out, guard, arena, interner, depth + 3);
                }
                dump_expr(out, arm.body, arena, interner, depth + 2);
            }
        }
        ExprKind::For {
            label,
            pattern,
            iter,
            guard,
            body,
            is_yield,
        } => {
            let label = format_label(*label, interner);
            let yield_marker = if *is_yield { " yield" } else { "" };
            write!(out, "{indent}For{label}{yield_marker} ")?;
            dump_binding_pattern(out, arena.get_binding_pattern(*pattern), interner);
            writeln!(out, " in")?;
            dump_expr(out, *iter, arena, interner, depth + 1);
            if guard.is_present() {
                writeln!(out, "{indent}  Guard")?;
                dump_expr(out, *guard, arena, interner, depth + 2);
            }
            dump_expr(out, *body, arena, interner, depth + 1);
        }
        _ => unreachable!("branch dumper called with non-branching expression"),
    }
    Ok(())
}

fn dump_loop_control(
    out: &mut String,
    kind: &ExprKind,
    arena: &ExprArena,
    interner: &StringInterner,
    depth: usize,
    indent: &str,
) -> fmt::Result {
    match kind {
        ExprKind::Loop { label, body } => {
            writeln!(out, "{indent}Loop{}", format_label(*label, interner))?;
            dump_expr(out, *body, arena, interner, depth + 1);
        }
        ExprKind::While { label, cond, body } => {
            writeln!(out, "{indent}While{}", format_label(*label, interner))?;
            dump_expr(out, *cond, arena, interner, depth + 1);
            dump_expr(out, *body, arena, interner, depth + 1);
        }
        ExprKind::Break { label, value } => {
            writeln!(out, "{indent}Break{}", format_label(*label, interner))?;
            if value.is_present() {
                dump_expr(out, *value, arena, interner, depth + 1);
            }
        }
        ExprKind::Continue { label, value } => {
            writeln!(out, "{indent}Continue{}", format_label(*label, interner))?;
            if value.is_present() {
                dump_expr(out, *value, arena, interner, depth + 1);
            }
        }
        _ => unreachable!("loop dumper called with non-loop expression"),
    }
    Ok(())
}

fn dump_binding(
    out: &mut String,
    kind: &ExprKind,
    arena: &ExprArena,
    interner: &StringInterner,
    depth: usize,
    indent: &str,
) -> fmt::Result {
    match kind {
        ExprKind::Block { stmts, result } => {
            writeln!(out, "{indent}Block")?;
            dump_stmts(out, *stmts, arena, interner, depth + 1);
            if result.is_present() {
                dump_expr(out, *result, arena, interner, depth + 1);
            }
        }
        ExprKind::Let {
            pattern,
            ty,
            init,
            mutable,
        } => {
            let mutability = match mutable {
                Mutability::Immutable => "$",
                Mutability::Mutable => "",
            };
            write!(out, "{indent}Let {mutability}")?;
            dump_binding_pattern(out, arena.get_binding_pattern(*pattern), interner);
            if ty.is_valid() {
                let parsed_ty = arena.get_parsed_type(*ty);
                write!(out, ": {}", format_parsed_type(parsed_ty, arena, interner))?;
            }
            writeln!(out, " =")?;
            dump_expr(out, *init, arena, interner, depth + 1);
        }
        ExprKind::Lambda {
            params,
            ret_ty,
            body,
        } => {
            let params: Vec<_> = arena
                .get_params(*params)
                .iter()
                .map(|param| interner.lookup(param.name))
                .collect();
            let return_type = if ret_ty.is_valid() {
                let ty = arena.get_parsed_type(*ret_ty);
                format!(" -> {}", format_parsed_type(ty, arena, interner))
            } else {
                String::new()
            };
            writeln!(out, "{indent}Lambda ({}){return_type}", params.join(", "))?;
            dump_expr(out, *body, arena, interner, depth + 1);
        }
        _ => unreachable!("binding dumper called with non-binding expression"),
    }
    Ok(())
}

fn dump_basic_collection(
    out: &mut String,
    kind: &ExprKind,
    arena: &ExprArena,
    interner: &StringInterner,
    depth: usize,
    indent: &str,
) -> fmt::Result {
    match kind {
        ExprKind::List(items) => {
            let items = arena.get_expr_list(*items);
            writeln!(out, "{indent}List [{} items]", items.len())?;
            for item in items {
                dump_expr(out, *item, arena, interner, depth + 1);
            }
        }
        ExprKind::Tuple(items) => {
            let items = arena.get_expr_list(*items);
            writeln!(out, "{indent}Tuple ({} items)", items.len())?;
            for item in items {
                dump_expr(out, *item, arena, interner, depth + 1);
            }
        }
        ExprKind::Map(entries) => {
            let entries = arena.get_map_entries(*entries);
            writeln!(out, "{indent}Map {{{} entries}}", entries.len())?;
            for entry in entries {
                dump_expr(out, entry.key, arena, interner, depth + 1);
                dump_expr(out, entry.value, arena, interner, depth + 1);
            }
        }
        ExprKind::Struct { type_path, fields } => {
            let path = format_parsed_type(arena.get_parsed_type(*type_path), arena, interner);
            writeln!(out, "{indent}Struct {path}")?;
            for init in arena.get_field_inits(*fields) {
                let field = interner.lookup(init.name);
                if let Some(value) = init.value {
                    writeln!(out, "{indent}  {field}:")?;
                    dump_expr(out, value, arena, interner, depth + 2);
                } else {
                    writeln!(out, "{indent}  {field} (shorthand)")?;
                }
            }
        }
        _ => unreachable!("collection dumper called with non-collection expression"),
    }
    Ok(())
}

fn dump_spread_collection(
    out: &mut String,
    kind: &ExprKind,
    arena: &ExprArena,
    interner: &StringInterner,
    depth: usize,
    indent: &str,
) -> fmt::Result {
    match kind {
        ExprKind::StructWithSpread { type_path, fields } => {
            let path = format_parsed_type(arena.get_parsed_type(*type_path), arena, interner);
            writeln!(out, "{indent}StructWithSpread {path}")?;
            for field in arena.get_struct_lit_fields(*fields) {
                match field {
                    StructLitField::Field(init) => {
                        writeln!(out, "{indent}  {}:", interner.lookup(init.name))?;
                        if let Some(value) = init.value {
                            dump_expr(out, value, arena, interner, depth + 2);
                        }
                    }
                    StructLitField::Spread { expr, .. } => {
                        writeln!(out, "{indent}  ...")?;
                        dump_expr(out, *expr, arena, interner, depth + 2);
                    }
                }
            }
        }
        ExprKind::ListWithSpread(elements) => {
            writeln!(out, "{indent}ListWithSpread")?;
            for element in arena.get_list_elements(*elements) {
                match element {
                    ori_ir::ast::ListElement::Expr { expr, .. } => {
                        dump_expr(out, *expr, arena, interner, depth + 1);
                    }
                    ori_ir::ast::ListElement::Spread { expr, .. } => {
                        writeln!(out, "{indent}  ...")?;
                        dump_expr(out, *expr, arena, interner, depth + 2);
                    }
                }
            }
        }
        ExprKind::MapWithSpread(elements) => {
            writeln!(out, "{indent}MapWithSpread")?;
            for element in arena.get_map_elements(*elements) {
                match element {
                    ori_ir::ast::MapElement::Entry(entry) => {
                        dump_expr(out, entry.key, arena, interner, depth + 1);
                        dump_expr(out, entry.value, arena, interner, depth + 1);
                    }
                    ori_ir::ast::MapElement::Spread { expr, .. } => {
                        writeln!(out, "{indent}  ...")?;
                        dump_expr(out, *expr, arena, interner, depth + 2);
                    }
                }
            }
        }
        _ => unreachable!("spread dumper called with non-spread expression"),
    }
    Ok(())
}

fn dump_value_form(
    out: &mut String,
    kind: &ExprKind,
    arena: &ExprArena,
    interner: &StringInterner,
    depth: usize,
    indent: &str,
) -> fmt::Result {
    match kind {
        ExprKind::Range {
            start,
            end,
            step,
            inclusive,
        } => {
            let operator = if *inclusive { "..=" } else { ".." };
            writeln!(out, "{indent}Range({operator})")?;
            if start.is_present() {
                dump_expr(out, *start, arena, interner, depth + 1);
            }
            if end.is_present() {
                dump_expr(out, *end, arena, interner, depth + 1);
            }
            if step.is_present() {
                writeln!(out, "{indent}  step:")?;
                dump_expr(out, *step, arena, interner, depth + 2);
            }
        }
        ExprKind::Ok(inner) | ExprKind::Err(inner) => {
            let name = if matches!(kind, ExprKind::Ok(_)) {
                "Ok"
            } else {
                "Err"
            };
            if inner.is_present() {
                writeln!(out, "{indent}{name}")?;
                dump_expr(out, *inner, arena, interner, depth + 1);
            } else {
                writeln!(out, "{indent}{name}(())")?;
            }
        }
        ExprKind::Some(inner) => {
            writeln!(out, "{indent}Some")?;
            dump_expr(out, *inner, arena, interner, depth + 1);
        }
        _ => unreachable!("value-form dumper called with non-value-form expression"),
    }
    Ok(())
}

fn dump_type_operation(
    out: &mut String,
    kind: &ExprKind,
    arena: &ExprArena,
    interner: &StringInterner,
    depth: usize,
    indent: &str,
) -> fmt::Result {
    match kind {
        ExprKind::Cast {
            expr, ty, fallible, ..
        } => {
            let keyword = if *fallible { "as?" } else { "as" };
            let parsed_ty = arena.get_parsed_type(*ty);
            writeln!(
                out,
                "{indent}Cast({keyword} {})",
                format_parsed_type(parsed_ty, arena, interner)
            )?;
            dump_expr(out, *expr, arena, interner, depth + 1);
        }
        ExprKind::Try(inner) | ExprKind::Unsafe(inner) | ExprKind::Await(inner) => {
            let label = match kind {
                ExprKind::Try(_) => "Try(?)",
                ExprKind::Unsafe(_) => "Unsafe",
                ExprKind::Await(_) => "Await",
                _ => unreachable!(),
            };
            writeln!(out, "{indent}{label}")?;
            dump_expr(out, *inner, arena, interner, depth + 1);
        }
        _ => unreachable!("type-operation dumper called with other expression"),
    }
    Ok(())
}

fn dump_effect(
    out: &mut String,
    kind: &ExprKind,
    arena: &ExprArena,
    interner: &StringInterner,
    depth: usize,
    indent: &str,
) -> fmt::Result {
    match kind {
        ExprKind::Assign { target, value } => {
            writeln!(out, "{indent}Assign")?;
            dump_expr(out, *target, arena, interner, depth + 1);
            dump_expr(out, *value, arena, interner, depth + 1);
        }
        ExprKind::AssignTarget { root, steps } => {
            writeln!(out, "{indent}AssignTarget")?;
            dump_expr(out, *root, arena, interner, depth + 1);
            for step in arena.get_access_steps(*steps) {
                match step {
                    ori_ir::AccessStep::Field(field) => {
                        writeln!(out, "{indent}  .{}", interner.lookup(*field))?;
                    }
                    ori_ir::AccessStep::Index(index) => {
                        dump_expr(out, *index, arena, interner, depth + 1);
                    }
                }
            }
        }
        ExprKind::WithCapability {
            capability,
            provider,
            body,
        } => {
            writeln!(
                out,
                "{indent}WithCapability {}",
                interner.lookup(*capability)
            )?;
            writeln!(out, "{indent}  provider:")?;
            dump_expr(out, *provider, arena, interner, depth + 2);
            writeln!(out, "{indent}  body:")?;
            dump_expr(out, *body, arena, interner, depth + 2);
        }
        _ => unreachable!("effect dumper called with non-effect expression"),
    }
    Ok(())
}

fn dump_terminal_form(
    out: &mut String,
    kind: &ExprKind,
    arena: &ExprArena,
    interner: &StringInterner,
    depth: usize,
    indent: &str,
) -> fmt::Result {
    match kind {
        ExprKind::TemplateFull(name) => {
            writeln!(out, "{indent}Template(`{}`)", interner.lookup(*name))
        }
        ExprKind::TemplateLiteral { head, parts } => {
            writeln!(
                out,
                "{indent}TemplateLiteral(`{}...`)",
                interner.lookup(*head)
            )?;
            for part in arena.get_template_parts(*parts) {
                dump_expr(out, part.expr, arena, interner, depth + 1);
            }
            Ok(())
        }
        ExprKind::FunctionSeq(id) => {
            let label = match arena.get_function_seq(*id) {
                ori_ir::ast::FunctionSeq::Try { .. } => "try",
                ori_ir::ast::FunctionSeq::Match { .. } => "match",
                ori_ir::ast::FunctionSeq::ForPattern { .. } => "for_pattern",
            };
            writeln!(out, "{indent}FunctionSeq({label})")
        }
        ExprKind::FunctionExp(id) => {
            writeln!(
                out,
                "{indent}FunctionExp({:?})",
                arena.get_function_exp(*id).kind
            )
        }
        ExprKind::Error => writeln!(out, "{indent}Error"),
        _ => unreachable!("terminal dumper called with non-terminal expression"),
    }
}

/// Dump an expression inline (single-line, no indentation).
///
/// Used for simple expressions in contexts where a tree dump would be excessive
/// (e.g., contract conditions, constant values).
#[expect(clippy::unwrap_used, reason = "write! to String is infallible")]
pub(crate) fn dump_expr_inline(
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
