//! Compact diagnostic formatting for canonical expressions.

use std::fmt;

use super::{CanExpr, CanNode};

// Keep diagnostics compact and expression-shaped instead of exposing the
// arena carrier field names produced by derived `Debug`.
impl fmt::Debug for CanExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CanExpr::Int(_)
            | CanExpr::Float(_)
            | CanExpr::Bool(_)
            | CanExpr::Str(_)
            | CanExpr::Char(_)
            | CanExpr::Duration { .. }
            | CanExpr::Size { .. }
            | CanExpr::Unit
            | CanExpr::Constant(_)
            | CanExpr::Ident(_)
            | CanExpr::Const(_)
            | CanExpr::SelfRef
            | CanExpr::FunctionRef(_)
            | CanExpr::TypeRef(_)
            | CanExpr::HashLength
            | CanExpr::None
            | CanExpr::Error => fmt_leaf(self, f),
            CanExpr::Binary { .. }
            | CanExpr::Unary { .. }
            | CanExpr::Cast { .. }
            | CanExpr::Call { .. }
            | CanExpr::MethodCall { .. }
            | CanExpr::Field { .. }
            | CanExpr::Index { .. } => fmt_operation(self, f),
            CanExpr::If { .. }
            | CanExpr::Match { .. }
            | CanExpr::For { .. }
            | CanExpr::Loop { .. }
            | CanExpr::Break { .. }
            | CanExpr::Continue { .. }
            | CanExpr::Block { .. }
            | CanExpr::Let { .. }
            | CanExpr::Assign { .. }
            | CanExpr::Lambda { .. }
            | CanExpr::WithCapability { .. } => fmt_control(self, f),
            CanExpr::List(_)
            | CanExpr::Tuple(_)
            | CanExpr::Map(_)
            | CanExpr::Struct { .. }
            | CanExpr::Range { .. }
            | CanExpr::Ok(_)
            | CanExpr::Err(_)
            | CanExpr::Some(_)
            | CanExpr::Try(_)
            | CanExpr::Await(_)
            | CanExpr::Unsafe(_)
            | CanExpr::FunctionExp { .. }
            | CanExpr::FormatWith { .. } => fmt_container(self, f),
        }
    }
}

fn fmt_leaf(expr: &CanExpr, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match expr {
        CanExpr::Int(value) => write!(f, "Int({value})"),
        CanExpr::Float(value) => write!(f, "Float({value})"),
        CanExpr::Bool(value) => write!(f, "Bool({value})"),
        CanExpr::Str(name) => write!(f, "Str({name:?})"),
        CanExpr::Char(value) => write!(f, "Char({value:?})"),
        CanExpr::Duration { value, unit } => write!(f, "Duration({value}, {unit:?})"),
        CanExpr::Size { value, unit } => write!(f, "Size({value}, {unit:?})"),
        CanExpr::Unit => write!(f, "Unit"),
        CanExpr::Constant(id) => write!(f, "Constant({id:?})"),
        CanExpr::Ident(name) => write!(f, "Ident({name:?})"),
        CanExpr::Const(name) => write!(f, "Const({name:?})"),
        CanExpr::SelfRef => write!(f, "SelfRef"),
        CanExpr::FunctionRef(name) => write!(f, "FunctionRef({name:?})"),
        CanExpr::TypeRef(name) => write!(f, "TypeRef({name:?})"),
        CanExpr::HashLength => write!(f, "HashLength"),
        CanExpr::None => write!(f, "None"),
        CanExpr::Error => write!(f, "Error"),
        _ => unreachable!("fmt_leaf called with non-leaf expression"),
    }
}

fn fmt_operation(expr: &CanExpr, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match expr {
        CanExpr::Binary { op, left, right } => write!(f, "Binary({op:?}, {left:?}, {right:?})"),
        CanExpr::Unary { op, operand } => write!(f, "Unary({op:?}, {operand:?})"),
        CanExpr::Cast {
            expr,
            target,
            fallible,
        } => write!(f, "Cast({expr:?}, {target:?}, fallible={fallible})"),
        CanExpr::Call { func, args } => write!(f, "Call({func:?}, {args:?})"),
        CanExpr::MethodCall {
            receiver,
            method,
            args,
        } => write!(f, "MethodCall({receiver:?}, {method:?}, {args:?})"),
        CanExpr::Field { receiver, field } => write!(f, "Field({receiver:?}, {field:?})"),
        CanExpr::Index {
            receiver,
            index,
            dispatch,
        } => write!(f, "Index({receiver:?}, {index:?}, {dispatch:?})"),
        _ => unreachable!("fmt_operation called with non-operation expression"),
    }
}

fn fmt_control(expr: &CanExpr, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match expr {
        CanExpr::If {
            cond,
            then_branch,
            else_branch,
        } => write!(f, "If({cond:?}, {then_branch:?}, {else_branch:?})"),
        CanExpr::Match {
            scrutinee,
            decision_tree,
            arms,
        } => write!(f, "Match({scrutinee:?}, {decision_tree:?}, {arms:?})"),
        CanExpr::For {
            label,
            pattern,
            iter,
            guard,
            body,
            is_yield,
        } => write!(
            f,
            "For({label:?}, {pattern:?}, {iter:?}, {guard:?}, {body:?}, yield={is_yield})"
        ),
        CanExpr::Loop { label, body } => write!(f, "Loop({label:?}, {body:?})"),
        CanExpr::Break { label, value } => write!(f, "Break({label:?}, {value:?})"),
        CanExpr::Continue { label, value } => write!(f, "Continue({label:?}, {value:?})"),
        CanExpr::Block { stmts, result } => write!(f, "Block({stmts:?}, {result:?})"),
        CanExpr::Let {
            pattern,
            init,
            mutable,
        } => write!(f, "Let({pattern:?}, {init:?}, {mutable:?})"),
        CanExpr::Assign { target, value } => write!(f, "Assign({target:?}, {value:?})"),
        CanExpr::Lambda { params, body } => write!(f, "Lambda({params:?}, {body:?})"),
        CanExpr::WithCapability {
            capability,
            provider,
            body,
        } => write!(f, "WithCapability({capability:?}, {provider:?}, {body:?})"),
        _ => unreachable!("fmt_control called with non-control expression"),
    }
}

fn fmt_container(expr: &CanExpr, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match expr {
        CanExpr::List(range) => write!(f, "List({range:?})"),
        CanExpr::Tuple(range) => write!(f, "Tuple({range:?})"),
        CanExpr::Map(range) => write!(f, "Map({range:?})"),
        CanExpr::Struct { name, fields } => write!(f, "Struct({name:?}, {fields:?})"),
        CanExpr::Range {
            start,
            end,
            step,
            inclusive,
        } => write!(
            f,
            "Range({start:?}, {end:?}, {step:?}, inclusive={inclusive})"
        ),
        CanExpr::Ok(value) => write!(f, "Ok({value:?})"),
        CanExpr::Err(value) => write!(f, "Err({value:?})"),
        CanExpr::Some(value) => write!(f, "Some({value:?})"),
        CanExpr::Try(value) => write!(f, "Try({value:?})"),
        CanExpr::Await(value) => write!(f, "Await({value:?})"),
        CanExpr::Unsafe(value) => write!(f, "Unsafe({value:?})"),
        CanExpr::FunctionExp { kind, props } => write!(f, "FunctionExp({kind:?}, {props:?})"),
        CanExpr::FormatWith { expr, spec } => write!(f, "FormatWith({expr:?}, {spec:?})"),
        _ => unreachable!("fmt_container called with non-container expression"),
    }
}

// Match `CanExpr`'s compact diagnostic form while retaining span and type.
impl fmt::Debug for CanNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "CanNode({:?}, {:?}, {:?})",
            self.kind, self.span, self.ty
        )
    }
}
