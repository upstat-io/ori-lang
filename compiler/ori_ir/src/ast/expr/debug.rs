//! `fmt::Debug` rendering for [`ExprKind`] — exhaustive per-variant formatting.

use std::fmt;

use super::ExprKind;

impl fmt::Debug for ExprKind {
    #[expect(
        clippy::too_many_lines,
        reason = "exhaustive ExprKind Debug formatting"
    )]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
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
            ExprKind::List(exprs) => write!(f, "List({exprs:?})"),
            ExprKind::ListWithSpread(elements) => write!(f, "ListWithSpread({elements:?})"),
            ExprKind::Map(entries) => write!(f, "Map({entries:?})"),
            ExprKind::MapWithSpread(elements) => write!(f, "MapWithSpread({elements:?})"),
            ExprKind::Struct { name, fields } => write!(f, "Struct({name:?}, {fields:?})"),
            ExprKind::StructWithSpread { name, fields } => {
                write!(f, "StructWithSpread({name:?}, {fields:?})")
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
            ExprKind::Break { label, value } => write!(f, "Break({label:?}, {value:?})"),
            ExprKind::Continue { label, value } => {
                write!(f, "Continue({label:?}, {value:?})")
            }
            ExprKind::Await(inner) => write!(f, "Await({inner:?})"),
            ExprKind::Try(inner) => write!(f, "Try({inner:?})"),
            ExprKind::Unsafe(inner) => write!(f, "Unsafe({inner:?})"),
            ExprKind::Cast { expr, ty, fallible } => {
                let op = if *fallible { "as?" } else { "as" };
                write!(f, "Cast({expr:?} {op} {ty:?})")
            }
            ExprKind::Assign { target, value } => write!(f, "Assign({target:?}, {value:?})"),
            ExprKind::AssignTarget { root, steps } => {
                write!(f, "AssignTarget({root:?}, {steps:?})")
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
        }
    }
}
