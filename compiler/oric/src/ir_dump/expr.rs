//! Expression and statement dumping for typed IR phase dumps.
//!
//! Contains the core dispatch table for all `ExprKind` variants, annotating
//! each node with its resolved type from the type checker.

use std::fmt::Write;

use ori_ir::ast::{ExprKind, StmtKind, StructLitField};
use ori_ir::{ExprArena, ExprId, Name, StringInterner};
use ori_types::{Idx, Pool, Tag, TypedModule};

use crate::ast_dump::patterns::{dump_binding_pattern, dump_match_pattern, format_label};

/// Format the type annotation suffix for an expression.
///
/// Returns ` : type_name` if the expression has a resolved type, or
/// ` : ?` if the expression is untyped (error nodes, out of range).
fn type_of(id: ExprId, typed: &TypedModule, pool: &Pool, interner: &StringInterner) -> String {
    match typed.expr_type(id.index()) {
        Some(idx) => {
            let ts = pool.format_type_resolved(idx, interner);
            let hint = unification_hint(idx, pool);
            format!(" : {ts}{hint}")
        }
        None => " : ?".to_string(),
    }
}

/// Classify a method call's dispatch target based on the receiver's type.
///
/// Returns a dispatch hint string like `"  [builtin: list]"` or `""`.
/// Uses the receiver type's tag to determine if the method is a builtin
/// (primitive or collection method) or a user-defined method (inherent/trait).
fn dispatch_hint(receiver_id: ExprId, typed: &TypedModule, pool: &Pool) -> String {
    let Some(receiver_idx) = typed.expr_type(receiver_id.index()) else {
        return String::new();
    };
    let tag = pool.tag(receiver_idx);

    // Builtin types have compiler-provided methods
    if tag.is_primitive() || tag.is_container() {
        let category = tag_to_method_category(tag);
        format!("  [builtin: {category}]")
    } else {
        String::new()
    }
}

/// Map a Pool Tag to the type category string used in method dispatch.
///
/// These names match the type names used in `ori_registry::BUILTIN_TYPES`.
fn tag_to_method_category(tag: Tag) -> &'static str {
    match tag {
        Tag::Int => "int",
        Tag::Float => "float",
        Tag::Bool => "bool",
        Tag::Str => "str",
        Tag::Char => "char",
        Tag::Byte => "byte",
        Tag::Unit => "unit",
        Tag::Never => "Never",
        Tag::Duration => "Duration",
        Tag::Size => "Size",
        Tag::Ordering => "Ordering",
        Tag::Error => "error",
        Tag::List => "list",
        Tag::Map => "map",
        Tag::Set => "set",
        Tag::Option => "Option",
        Tag::Result => "Result",
        Tag::Range => "range",
        Tag::Channel => "Channel",
        Tag::Tuple => "tuple",
        Tag::Iterator => "Iterator",
        Tag::DoubleEndedIterator => "DoubleEndedIterator",
        _ => "builtin",
    }
}

/// Classify a type's unification status for display.
///
/// Returns ` (unresolved)` for type variables that weren't fully unified
/// to a concrete type. Variables that were linked (unified) are NOT flagged —
/// `format_type_resolved` already follows the link chain to show the concrete type.
fn unification_hint(idx: Idx, pool: &Pool) -> &'static str {
    let tag = pool.tag(idx);
    if !tag.is_type_variable() {
        return "";
    }
    // Check if this variable was unified (linked) to a concrete type.
    // VarState::Link means it was resolved; Unbound means it wasn't.
    let var_id = pool.data(idx);
    match pool.var_state(var_id) {
        ori_types::VarState::Link { .. } => "",
        _ => " (unresolved)",
    }
}

/// Dump an expression with indentation and type annotations.
///
/// Each node shows its `ExprKind` variant followed by ` : resolved_type`.
/// Child expressions are recursively dumped at increased depth.
#[expect(
    clippy::too_many_lines,
    reason = "dispatch table over ~30 ExprKind variants"
)]
#[expect(clippy::unwrap_used, reason = "write! to String is infallible")]
pub(super) fn dump_expr(
    out: &mut String,
    id: ExprId,
    arena: &ExprArena,
    typed: &TypedModule,
    pool: &Pool,
    interner: &StringInterner,
    depth: usize,
) {
    if !id.is_present() {
        return;
    }
    let indent = "  ".repeat(depth);
    let ty = type_of(id, typed, pool, interner);
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
            dump_expr(out, *left, arena, typed, pool, interner, depth + 1);
            dump_expr(out, *right, arena, typed, pool, interner, depth + 1);
            return;
        }
        ExprKind::Unary { op, operand } => {
            writeln!(out, "{indent}Unary({}){ty}", op.as_symbol()).unwrap();
            dump_expr(out, *operand, arena, typed, pool, interner, depth + 1);
            return;
        }

        // Calls
        ExprKind::Call { func, args } => {
            writeln!(out, "{indent}Call{ty}").unwrap();
            dump_expr(out, *func, arena, typed, pool, interner, depth + 1);
            for arg_id in arena.get_expr_list(*args) {
                dump_expr(out, *arg_id, arena, typed, pool, interner, depth + 1);
            }
            return;
        }
        ExprKind::CallNamed { func, args } => {
            writeln!(out, "{indent}CallNamed{ty}").unwrap();
            dump_expr(out, *func, arena, typed, pool, interner, depth + 1);
            for arg in arena.get_call_args(*args) {
                let label = arg
                    .name
                    .filter(|n| *n != Name::EMPTY)
                    .map(|n| format!("{}:", interner.lookup(n)))
                    .unwrap_or_default();
                writeln!(out, "{indent}  Arg {label}").unwrap();
                dump_expr(out, arg.value, arena, typed, pool, interner, depth + 2);
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
            let hint = dispatch_hint(*receiver, typed, pool);
            writeln!(out, "{indent}MethodCall .{method_name}(){ty}{hint}").unwrap();
            dump_expr(out, *receiver, arena, typed, pool, interner, depth + 1);
            for arg_id in arena.get_expr_list(*args) {
                dump_expr(out, *arg_id, arena, typed, pool, interner, depth + 1);
            }
            return;
        }
        ExprKind::MethodCallNamed {
            receiver,
            method,
            args,
        } => {
            let method_name = interner.lookup(*method);
            let hint = dispatch_hint(*receiver, typed, pool);
            writeln!(out, "{indent}MethodCallNamed .{method_name}(){ty}{hint}").unwrap();
            dump_expr(out, *receiver, arena, typed, pool, interner, depth + 1);
            for arg in arena.get_call_args(*args) {
                let label = arg
                    .name
                    .filter(|n| *n != Name::EMPTY)
                    .map(|n| format!("{}:", interner.lookup(n)))
                    .unwrap_or_default();
                writeln!(out, "{indent}  Arg {label}").unwrap();
                dump_expr(out, arg.value, arena, typed, pool, interner, depth + 2);
            }
            return;
        }

        // Field / Index
        ExprKind::Field { receiver, field } => {
            let field_name = interner.lookup(*field);
            writeln!(out, "{indent}Field .{field_name}{ty}").unwrap();
            dump_expr(out, *receiver, arena, typed, pool, interner, depth + 1);
            return;
        }
        ExprKind::Index { receiver, index } => {
            writeln!(out, "{indent}Index{ty}").unwrap();
            dump_expr(out, *receiver, arena, typed, pool, interner, depth + 1);
            dump_expr(out, *index, arena, typed, pool, interner, depth + 1);
            return;
        }

        // Control flow
        ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => {
            writeln!(out, "{indent}If{ty}").unwrap();
            dump_expr(out, *cond, arena, typed, pool, interner, depth + 1);
            writeln!(out, "{indent}  Then").unwrap();
            dump_expr(out, *then_branch, arena, typed, pool, interner, depth + 2);
            if else_branch.is_present() {
                writeln!(out, "{indent}  Else").unwrap();
                dump_expr(out, *else_branch, arena, typed, pool, interner, depth + 2);
            }
            return;
        }
        ExprKind::Match { scrutinee, arms } => {
            writeln!(out, "{indent}Match{ty}").unwrap();
            dump_expr(out, *scrutinee, arena, typed, pool, interner, depth + 1);
            for arm in arena.get_arms(*arms) {
                write!(out, "{indent}  Arm ").unwrap();
                dump_match_pattern(out, &arm.pattern, arena, interner);
                writeln!(out).unwrap();
                if let Some(guard) = arm.guard {
                    writeln!(out, "{indent}    Guard").unwrap();
                    dump_expr(out, guard, arena, typed, pool, interner, depth + 3);
                }
                dump_expr(out, arm.body, arena, typed, pool, interner, depth + 2);
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
            dump_expr(out, *iter, arena, typed, pool, interner, depth + 1);
            if guard.is_present() {
                writeln!(out, "{indent}  Guard").unwrap();
                dump_expr(out, *guard, arena, typed, pool, interner, depth + 2);
            }
            dump_expr(out, *body, arena, typed, pool, interner, depth + 1);
            return;
        }
        ExprKind::Loop { label, body } => {
            let label_str = format_label(*label, interner);
            writeln!(out, "{indent}Loop{label_str}{ty}").unwrap();
            dump_expr(out, *body, arena, typed, pool, interner, depth + 1);
            return;
        }
        ExprKind::Break { label, value } => {
            let label_str = format_label(*label, interner);
            writeln!(out, "{indent}Break{label_str}{ty}").unwrap();
            if value.is_present() {
                dump_expr(out, *value, arena, typed, pool, interner, depth + 1);
            }
            return;
        }
        ExprKind::Continue { label, value } => {
            let label_str = format_label(*label, interner);
            writeln!(out, "{indent}Continue{label_str}{ty}").unwrap();
            if value.is_present() {
                dump_expr(out, *value, arena, typed, pool, interner, depth + 1);
            }
            return;
        }

        // Blocks and bindings
        ExprKind::Block { stmts, result } => {
            writeln!(out, "{indent}Block{ty}").unwrap();
            dump_stmts(out, *stmts, arena, typed, pool, interner, depth + 1);
            if result.is_present() {
                dump_expr(out, *result, arena, typed, pool, interner, depth + 1);
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
            let init_ty = type_of(*init, typed, pool, interner);
            write!(out, "{indent}Let ").unwrap();
            dump_binding_pattern(out, arena.get_binding_pattern(*pattern), interner);
            writeln!(out, "{init_ty} =").unwrap();
            dump_expr(out, *init, arena, typed, pool, interner, depth + 1);
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
            dump_expr(out, *body, arena, typed, pool, interner, depth + 1);
            return;
        }

        // Collections
        ExprKind::List(items) => {
            let exprs = arena.get_expr_list(*items);
            writeln!(out, "{indent}List [{} items]{ty}", exprs.len()).unwrap();
            for item_id in exprs {
                dump_expr(out, *item_id, arena, typed, pool, interner, depth + 1);
            }
            return;
        }
        ExprKind::Tuple(items) => {
            let exprs = arena.get_expr_list(*items);
            writeln!(out, "{indent}Tuple ({} items){ty}", exprs.len()).unwrap();
            for item_id in exprs {
                dump_expr(out, *item_id, arena, typed, pool, interner, depth + 1);
            }
            return;
        }
        ExprKind::Map(entries) => {
            let entries_list = arena.get_map_entries(*entries);
            writeln!(out, "{indent}Map {{{} entries}}{ty}", entries_list.len()).unwrap();
            for entry in entries_list {
                dump_expr(out, entry.key, arena, typed, pool, interner, depth + 1);
                dump_expr(out, entry.value, arena, typed, pool, interner, depth + 1);
            }
            return;
        }
        ExprKind::Struct { name, fields } => {
            let sname = interner.lookup(*name);
            writeln!(out, "{indent}Struct {sname}{ty}").unwrap();
            for init in arena.get_field_inits(*fields) {
                let fname = interner.lookup(init.name);
                if let Some(val) = init.value {
                    writeln!(out, "{indent}  {fname}:").unwrap();
                    dump_expr(out, val, arena, typed, pool, interner, depth + 2);
                } else {
                    writeln!(out, "{indent}  {fname} (shorthand)").unwrap();
                }
            }
            return;
        }
        ExprKind::StructWithSpread { name, fields } => {
            let sname = interner.lookup(*name);
            writeln!(out, "{indent}StructWithSpread {sname}{ty}").unwrap();
            for field in arena.get_struct_lit_fields(*fields) {
                match field {
                    StructLitField::Field(init) => {
                        let fname = interner.lookup(init.name);
                        writeln!(out, "{indent}  {fname}:").unwrap();
                        if let Some(val) = init.value {
                            dump_expr(out, val, arena, typed, pool, interner, depth + 2);
                        }
                    }
                    StructLitField::Spread { expr, .. } => {
                        writeln!(out, "{indent}  ...").unwrap();
                        dump_expr(out, *expr, arena, typed, pool, interner, depth + 2);
                    }
                }
            }
            return;
        }
        ExprKind::ListWithSpread(elements) => {
            writeln!(out, "{indent}ListWithSpread{ty}").unwrap();
            for elem in arena.get_list_elements(*elements) {
                match elem {
                    ori_ir::ast::ListElement::Expr { expr, .. } => {
                        dump_expr(out, *expr, arena, typed, pool, interner, depth + 1);
                    }
                    ori_ir::ast::ListElement::Spread { expr, .. } => {
                        writeln!(out, "{indent}  ...").unwrap();
                        dump_expr(out, *expr, arena, typed, pool, interner, depth + 2);
                    }
                }
            }
            return;
        }
        ExprKind::MapWithSpread(elements) => {
            writeln!(out, "{indent}MapWithSpread{ty}").unwrap();
            for elem in arena.get_map_elements(*elements) {
                match elem {
                    ori_ir::ast::MapElement::Entry(entry) => {
                        dump_expr(out, entry.key, arena, typed, pool, interner, depth + 1);
                        dump_expr(out, entry.value, arena, typed, pool, interner, depth + 1);
                    }
                    ori_ir::ast::MapElement::Spread { expr, .. } => {
                        writeln!(out, "{indent}  ...").unwrap();
                        dump_expr(out, *expr, arena, typed, pool, interner, depth + 2);
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
            writeln!(out, "{indent}Range({kind}){ty}").unwrap();
            if start.is_present() {
                dump_expr(out, *start, arena, typed, pool, interner, depth + 1);
            }
            if end.is_present() {
                dump_expr(out, *end, arena, typed, pool, interner, depth + 1);
            }
            if step.is_present() {
                writeln!(out, "{indent}  step:").unwrap();
                dump_expr(out, *step, arena, typed, pool, interner, depth + 2);
            }
            return;
        }

        // Result / Option
        ExprKind::Ok(inner) => {
            if inner.is_present() {
                writeln!(out, "{indent}Ok{ty}").unwrap();
                dump_expr(out, *inner, arena, typed, pool, interner, depth + 1);
            } else {
                writeln!(out, "{indent}Ok(()){ty}").unwrap();
            }
            return;
        }
        ExprKind::Err(inner) => {
            if inner.is_present() {
                writeln!(out, "{indent}Err{ty}").unwrap();
                dump_expr(out, *inner, arena, typed, pool, interner, depth + 1);
            } else {
                writeln!(out, "{indent}Err(()){ty}").unwrap();
            }
            return;
        }
        ExprKind::Some(inner) => {
            writeln!(out, "{indent}Some{ty}").unwrap();
            dump_expr(out, *inner, arena, typed, pool, interner, depth + 1);
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
            dump_expr(out, *expr, arena, typed, pool, interner, depth + 1);
            return;
        }
        ExprKind::Try(inner) => {
            writeln!(out, "{indent}Try(?){ty}").unwrap();
            dump_expr(out, *inner, arena, typed, pool, interner, depth + 1);
            return;
        }
        ExprKind::Unsafe(inner) => {
            writeln!(out, "{indent}Unsafe{ty}").unwrap();
            dump_expr(out, *inner, arena, typed, pool, interner, depth + 1);
            return;
        }
        ExprKind::Await(inner) => {
            writeln!(out, "{indent}Await{ty}").unwrap();
            dump_expr(out, *inner, arena, typed, pool, interner, depth + 1);
            return;
        }
        ExprKind::Assign { target, value } => {
            writeln!(out, "{indent}Assign{ty}").unwrap();
            dump_expr(out, *target, arena, typed, pool, interner, depth + 1);
            dump_expr(out, *value, arena, typed, pool, interner, depth + 1);
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
            dump_expr(out, *provider, arena, typed, pool, interner, depth + 2);
            writeln!(out, "{indent}  body:").unwrap();
            dump_expr(out, *body, arena, typed, pool, interner, depth + 2);
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
                dump_expr(out, part.expr, arena, typed, pool, interner, depth + 1);
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
fn dump_stmts(
    out: &mut String,
    stmts: ori_ir::StmtRange,
    arena: &ExprArena,
    typed: &TypedModule,
    pool: &Pool,
    interner: &StringInterner,
    depth: usize,
) {
    let indent = "  ".repeat(depth);
    for stmt in arena.get_stmt_range(stmts) {
        match &stmt.kind {
            StmtKind::Expr(id) => dump_expr(out, *id, arena, typed, pool, interner, depth),
            StmtKind::Let {
                pattern,
                ty: _,
                init,
                mutable: _,
            } => {
                // Mutability is shown per-binding by dump_binding_pattern
                let init_ty = type_of(*init, typed, pool, interner);
                write!(out, "{indent}Let ").unwrap();
                dump_binding_pattern(out, arena.get_binding_pattern(*pattern), interner);
                writeln!(out, "{init_ty} =").unwrap();
                dump_expr(out, *init, arena, typed, pool, interner, depth + 1);
            }
        }
    }
}
