//! Debug formatting for [`CanExpr`].

use std::fmt;

use super::expr::CanExpr;

impl fmt::Debug for CanExpr {
    #[expect(clippy::too_many_lines, reason = "exhaustive CanExpr Debug formatting")]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CanExpr::Int(v) => write!(f, "Int({v})"),
            CanExpr::Float(v) => write!(f, "Float({v})"),
            CanExpr::Bool(v) => write!(f, "Bool({v})"),
            CanExpr::Str(n) => write!(f, "Str({n:?})"),
            CanExpr::Char(c) => write!(f, "Char({c:?})"),
            CanExpr::Duration { value, unit } => write!(f, "Duration({value}, {unit:?})"),
            CanExpr::Size { value, unit } => write!(f, "Size({value}, {unit:?})"),
            CanExpr::Unit => write!(f, "Unit"),
            CanExpr::Constant(id) => write!(f, "Constant({id:?})"),
            CanExpr::Ident(n) => write!(f, "Ident({n:?})"),
            CanExpr::Const(n) => write!(f, "Const({n:?})"),
            CanExpr::SelfRef => write!(f, "SelfRef"),
            CanExpr::FunctionRef(n) => write!(f, "FunctionRef({n:?})"),
            CanExpr::TypeRef(n) => write!(f, "TypeRef({n:?})"),
            CanExpr::HashLength => write!(f, "HashLength"),
            CanExpr::Binary { op, left, right } => {
                write!(f, "Binary({op:?}, {left:?}, {right:?})")
            }
            CanExpr::Unary { op, operand } => write!(f, "Unary({op:?}, {operand:?})"),
            CanExpr::Cast {
                expr,
                target,
                fallible,
            } => {
                write!(f, "Cast({expr:?}, {target:?}, fallible={fallible})")
            }
            CanExpr::Call { func, args } => write!(f, "Call({func:?}, {args:?})"),
            CanExpr::MethodCall {
                receiver,
                method,
                args,
            } => {
                write!(f, "MethodCall({receiver:?}, {method:?}, {args:?})")
            }
            CanExpr::Field { receiver, field } => {
                write!(f, "Field({receiver:?}, {field:?})")
            }
            CanExpr::Index { receiver, index } => {
                write!(f, "Index({receiver:?}, {index:?})")
            }
            CanExpr::If {
                cond,
                then_branch,
                else_branch,
            } => {
                write!(f, "If({cond:?}, {then_branch:?}, {else_branch:?})")
            }
            CanExpr::Match {
                scrutinee,
                decision_tree,
                arms,
            } => {
                write!(f, "Match({scrutinee:?}, {decision_tree:?}, {arms:?})")
            }
            CanExpr::For {
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
            CanExpr::Loop { label, body } => write!(f, "Loop({label:?}, {body:?})"),
            CanExpr::Break { label, value } => write!(f, "Break({label:?}, {value:?})"),
            CanExpr::Continue { label, value } => write!(f, "Continue({label:?}, {value:?})"),
            CanExpr::Block { stmts, result } => write!(f, "Block({stmts:?}, {result:?})"),
            CanExpr::Let {
                pattern,
                init,
                mutable,
            } => {
                write!(f, "Let({pattern:?}, {init:?}, {mutable:?})")
            }
            CanExpr::Assign { target, value } => write!(f, "Assign({target:?}, {value:?})"),
            CanExpr::Lambda { params, body } => {
                write!(f, "Lambda({params:?}, {body:?})")
            }
            CanExpr::List(r) => write!(f, "List({r:?})"),
            CanExpr::Tuple(r) => write!(f, "Tuple({r:?})"),
            CanExpr::Map(r) => write!(f, "Map({r:?})"),
            CanExpr::Struct { name, fields } => write!(f, "Struct({name:?}, {fields:?})"),
            CanExpr::Range {
                start,
                end,
                step,
                inclusive,
            } => {
                write!(
                    f,
                    "Range({start:?}, {end:?}, {step:?}, inclusive={inclusive})"
                )
            }
            CanExpr::Ok(v) => write!(f, "Ok({v:?})"),
            CanExpr::Err(v) => write!(f, "Err({v:?})"),
            CanExpr::Some(v) => write!(f, "Some({v:?})"),
            CanExpr::None => write!(f, "None"),
            CanExpr::Try(v) => write!(f, "Try({v:?})"),
            CanExpr::Await(v) => write!(f, "Await({v:?})"),
            CanExpr::Unsafe(v) => write!(f, "Unsafe({v:?})"),
            CanExpr::WithCapability {
                capability,
                provider,
                body,
            } => {
                write!(f, "WithCapability({capability:?}, {provider:?}, {body:?})")
            }
            CanExpr::FunctionExp { kind, props } => {
                write!(f, "FunctionExp({kind:?}, {props:?})")
            }
            CanExpr::FormatWith { expr, spec } => {
                write!(f, "FormatWith({expr:?}, {spec:?})")
            }
            CanExpr::Error => write!(f, "Error"),
        }
    }
}
