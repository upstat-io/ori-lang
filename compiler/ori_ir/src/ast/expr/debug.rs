//! `fmt::Debug` rendering for [`ExprKind`] — exhaustive per-variant formatting.

use std::fmt;

use super::ExprKind;

impl fmt::Debug for ExprKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            expr @ (ExprKind::Int(_)
            | ExprKind::Float(_)
            | ExprKind::Bool(_)
            | ExprKind::String(_)
            | ExprKind::Char(_)
            | ExprKind::Duration { .. }
            | ExprKind::Size { .. }
            | ExprKind::Unit
            | ExprKind::Ident(_)
            | ExprKind::Const(_)
            | ExprKind::SelfRef
            | ExprKind::FunctionRef(_)
            | ExprKind::HashLength
            | ExprKind::Binary { .. }
            | ExprKind::Unary { .. }
            | ExprKind::Call { .. }
            | ExprKind::CallNamed { .. }
            | ExprKind::MethodCall { .. }
            | ExprKind::MethodCallNamed { .. }
            | ExprKind::Field { .. }
            | ExprKind::Index { .. }) => fmt_basic_expr(expr, f),
            expr @ (ExprKind::If { .. }
            | ExprKind::Match { .. }
            | ExprKind::For { .. }
            | ExprKind::Loop { .. }
            | ExprKind::While { .. }
            | ExprKind::Block { .. }
            | ExprKind::Let { .. }
            | ExprKind::Lambda { .. }
            | ExprKind::Break { .. }
            | ExprKind::Continue { .. }
            | ExprKind::Assign { .. }
            | ExprKind::AssignTarget { .. }) => fmt_control_expr(expr, f),
            expr @ (ExprKind::List(_)
            | ExprKind::ListWithSpread(_)
            | ExprKind::Map(_)
            | ExprKind::MapWithSpread(_)
            | ExprKind::Struct { .. }
            | ExprKind::StructWithSpread { .. }
            | ExprKind::Tuple(_)
            | ExprKind::Range { .. }
            | ExprKind::Ok(_)
            | ExprKind::Err(_)
            | ExprKind::Some(_)
            | ExprKind::None
            | ExprKind::Await(_)
            | ExprKind::Try(_)
            | ExprKind::Unsafe(_)
            | ExprKind::Cast { .. }
            | ExprKind::WithCapability { .. }
            | ExprKind::FunctionSeq(_)
            | ExprKind::FunctionExp(_)
            | ExprKind::TemplateFull(_)
            | ExprKind::TemplateLiteral { .. }
            | ExprKind::Error) => fmt_aggregate_expr(expr, f),
        }
    }
}

fn fmt_basic_expr(expr: &ExprKind, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match expr {
        ExprKind::Int(n) => write!(f, "Int({n})"),
        ExprKind::Float(bits) => write!(f, "Float({})", f64::from_bits(*bits)),
        ExprKind::Bool(b) => write!(f, "Bool({b})"),
        ExprKind::String(n) => write!(f, "String({n:?})"),
        ExprKind::Char(c) => write!(f, "Char({c:?})"),
        ExprKind::Duration { value, unit } => write!(f, "Duration({value}{unit:?})"),
        ExprKind::Size { value, unit } => write!(f, "Size({value}{unit:?})"),
        ExprKind::Unit => write!(f, "Unit"),
        ExprKind::Ident(n) => write!(f, "Ident({n:?})"),
        ExprKind::Const(n) => write!(f, "Const({n:?})"),
        ExprKind::SelfRef => write!(f, "SelfRef"),
        ExprKind::FunctionRef(n) => write!(f, "FunctionRef({n:?})"),
        ExprKind::HashLength => write!(f, "HashLength"),
        ExprKind::Binary { op, left, right } => {
            write!(f, "Binary({op:?}, {left:?}, {right:?})")
        }
        ExprKind::Unary { op, operand } => write!(f, "Unary({op:?}, {operand:?})"),
        ExprKind::Call { func, args } => write!(f, "Call({func:?}, {args:?})"),
        ExprKind::CallNamed { func, args } => write!(f, "CallNamed({func:?}, {args:?})"),
        ExprKind::MethodCall {
            receiver,
            method,
            args,
        } => {
            write!(f, "MethodCall({receiver:?}, {method:?}, {args:?})")
        }
        ExprKind::MethodCallNamed {
            receiver,
            method,
            args,
        } => {
            write!(f, "MethodCallNamed({receiver:?}, {method:?}, {args:?})")
        }
        ExprKind::Field { receiver, field } => {
            write!(f, "Field({receiver:?}, {field:?})")
        }
        ExprKind::Index { receiver, index } => {
            write!(f, "Index({receiver:?}, {index:?})")
        }
        _ => unreachable!("basic expression classifier is exhaustive"),
    }
}

fn fmt_control_expr(expr: &ExprKind, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match expr {
        ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => {
            write!(f, "If({cond:?}, {then_branch:?}, {else_branch:?})")
        }
        ExprKind::Match { scrutinee, arms } => {
            write!(f, "Match({scrutinee:?}, {arms:?})")
        }
        ExprKind::For {
            label,
            pattern,
            iter,
            guard,
            body,
            is_yield,
        } => {
            write!(
                f,
                "For({label:?}, {pattern:?}, {iter:?}, {guard:?}, {body:?}, yield={is_yield})"
            )
        }
        ExprKind::Loop { label, body } => write!(f, "Loop({label:?}, {body:?})"),
        ExprKind::While { label, cond, body } => {
            write!(f, "While({label:?}, {cond:?}, {body:?})")
        }
        ExprKind::Block { stmts, result } => write!(f, "Block({stmts:?}, {result:?})"),
        ExprKind::Let {
            pattern,
            ty,
            init,
            mutable,
        } => {
            write!(f, "Let({pattern:?}, {ty:?}, {init:?}, {mutable:?})")
        }
        ExprKind::Lambda {
            params,
            ret_ty,
            body,
        } => {
            write!(f, "Lambda({params:?}, {ret_ty:?}, {body:?})")
        }
        ExprKind::Break { label, value } => write!(f, "Break({label:?}, {value:?})"),
        ExprKind::Continue { label, value } => {
            write!(f, "Continue({label:?}, {value:?})")
        }
        ExprKind::Assign { target, value } => write!(f, "Assign({target:?}, {value:?})"),
        ExprKind::AssignTarget { root, steps } => {
            write!(f, "AssignTarget({root:?}, {steps:?})")
        }
        _ => unreachable!("control expression classifier is exhaustive"),
    }
}

fn fmt_aggregate_expr(expr: &ExprKind, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match expr {
        ExprKind::List(exprs) => write!(f, "List({exprs:?})"),
        ExprKind::ListWithSpread(elements) => write!(f, "ListWithSpread({elements:?})"),
        ExprKind::Map(entries) => write!(f, "Map({entries:?})"),
        ExprKind::MapWithSpread(elements) => write!(f, "MapWithSpread({elements:?})"),
        ExprKind::Struct { type_path, fields } => {
            write!(f, "Struct({type_path:?}, {fields:?})")
        }
        ExprKind::StructWithSpread { type_path, fields } => {
            write!(f, "StructWithSpread({type_path:?}, {fields:?})")
        }
        ExprKind::Tuple(exprs) => write!(f, "Tuple({exprs:?})"),
        ExprKind::Range {
            start,
            end,
            step,
            inclusive,
        } => {
            write!(
                f,
                "Range({start:?}, {end:?}, step={step:?}, inclusive={inclusive})"
            )
        }
        ExprKind::Ok(inner) => write!(f, "Ok({inner:?})"),
        ExprKind::Err(inner) => write!(f, "Err({inner:?})"),
        ExprKind::Some(inner) => write!(f, "Some({inner:?})"),
        ExprKind::None => write!(f, "None"),
        ExprKind::Await(inner) => write!(f, "Await({inner:?})"),
        ExprKind::Try(inner) => write!(f, "Try({inner:?})"),
        ExprKind::Unsafe(inner) => write!(f, "Unsafe({inner:?})"),
        ExprKind::Cast { expr, ty, fallible } => {
            let op = if *fallible { "as?" } else { "as" };
            write!(f, "Cast({expr:?} {op} {ty:?})")
        }
        ExprKind::WithCapability {
            capability,
            provider,
            body,
        } => {
            write!(f, "WithCapability({capability:?}, {provider:?}, {body:?})")
        }
        ExprKind::FunctionSeq(seq) => write!(f, "FunctionSeq({seq:?})"),
        ExprKind::FunctionExp(exp) => write!(f, "FunctionExp({exp:?})"),
        ExprKind::TemplateFull(name) => write!(f, "TemplateFull({name:?})"),
        ExprKind::TemplateLiteral { head, parts } => {
            write!(f, "TemplateLiteral({head:?}, {parts:?})")
        }
        ExprKind::Error => write!(f, "Error"),
        _ => unreachable!("aggregate expression classifier is exhaustive"),
    }
}
