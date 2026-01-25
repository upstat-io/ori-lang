//! Expression Types
//!
//! Core expression nodes and variants.
//!
//! # Design Notes
//! Per design spec A-data-structures.md:
//! - No Box<Expr>, use ExprId(u32) indices
//! - Contiguous arrays for cache locality
//! - All types have Salsa-required traits (Clone, Eq, Hash, Debug)

use std::fmt;
use std::hash::{Hash, Hasher};

use crate::{Name, Span, ExprId, ExprRange, StmtRange, Spanned, ParsedType};
use crate::token::{DurationUnit, SizeUnit};
use super::operators::{BinaryOp, UnaryOp};
use super::ranges::{ArmRange, MapEntryRange, FieldInitRange, CallArgRange};
use super::patterns::{BindingPattern, FunctionSeq, FunctionExp};

/// Expression node.
///
/// # Salsa Compatibility
/// Has all required traits: Clone, Eq, `PartialEq`, Hash, Debug
#[derive(Clone, Eq, PartialEq)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

impl Expr {
    pub fn new(kind: ExprKind, span: Span) -> Self {
        Expr { kind, span }
    }
}

impl Hash for Expr {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.kind.hash(state);
        self.span.hash(state);
    }
}

impl fmt::Debug for Expr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?} @ {:?}", self.kind, self.span)
    }
}

impl Spanned for Expr {
    fn span(&self) -> Span {
        self.span
    }
}

/// Expression variants.
///
/// All children are indices, not boxes. Per design:
/// "No Box<Expr>, use ExprId(u32) indices"
///
/// # Salsa Compatibility
/// Has all required traits: Clone, Eq, `PartialEq`, Hash, Debug
#[derive(Clone, Eq, PartialEq, Hash)]
pub enum ExprKind {
    // ===== Literals (no children) =====

    /// Integer literal: 42, `1_000`
    Int(i64),

    /// Float literal: 3.14, 2.5e-8 (stored as bits for Hash)
    Float(u64),

    /// Boolean literal: true, false
    Bool(bool),

    /// String literal (interned)
    String(Name),

    /// Char literal: 'a', '\n'
    Char(char),

    /// Duration: 100ms, 5s, 2h
    Duration {
        value: u64,
        unit: DurationUnit,
    },

    /// Size: 4kb, 10mb
    Size {
        value: u64,
        unit: SizeUnit,
    },

    /// Unit: ()
    Unit,

    // ===== References =====

    /// Variable reference
    Ident(Name),

    /// Config reference: $name
    Config(Name),

    /// Self reference: self
    SelfRef,

    /// Function reference: @name
    FunctionRef(Name),

    /// Hash in index context (refers to length): #
    HashLength,

    // ===== Compound expressions =====

    /// Binary operation: left op right
    Binary {
        op: BinaryOp,
        left: ExprId,
        right: ExprId,
    },

    /// Unary operation: op operand
    Unary {
        op: UnaryOp,
        operand: ExprId,
    },

    /// Function call with positional args: func(arg)
    /// Only valid for single-param functions.
    Call {
        func: ExprId,
        args: ExprRange,
    },

    /// Function call with named args: func(a: 1, b: 2)
    /// Required for multi-param functions.
    CallNamed {
        func: ExprId,
        args: CallArgRange,
    },

    /// Method call: receiver.method(args...)
    MethodCall {
        receiver: ExprId,
        method: Name,
        args: ExprRange,
    },

    /// Field access: receiver.field
    Field {
        receiver: ExprId,
        field: Name,
    },

    /// Index access: receiver[index]
    Index {
        receiver: ExprId,
        index: ExprId,
    },

    // ===== Control flow =====

    /// Conditional: if cond then t else e
    If {
        cond: ExprId,
        then_branch: ExprId,
        else_branch: Option<ExprId>,
    },

    /// Match expression (statement form): match value { arms }
    Match {
        scrutinee: ExprId,
        arms: ArmRange,
    },

    /// For loop: for x in iter do/yield body
    For {
        binding: Name,
        iter: ExprId,
        guard: Option<ExprId>,
        body: ExprId,
        is_yield: bool,
    },

    /// Loop: loop(body)
    Loop {
        body: ExprId,
    },

    /// Block: { stmts; result }
    Block {
        stmts: StmtRange,
        result: Option<ExprId>,
    },

    // ===== Binding =====

    /// Let binding: let pattern = init
    Let {
        pattern: BindingPattern,
        /// Optional type annotation.
        ty: Option<ParsedType>,
        init: ExprId,
        mutable: bool,
    },

    /// Lambda: params -> body
    Lambda {
        params: super::ranges::ParamRange,
        /// Optional return type annotation.
        ret_ty: Option<ParsedType>,
        body: ExprId,
    },

    // ===== Collections =====

    /// List literal: [a, b, c]
    List(ExprRange),

    /// Map literal: {k: v, ...}
    Map(MapEntryRange),

    /// Struct literal: Point { x: 0, y: 0 }
    Struct {
        name: Name,
        fields: FieldInitRange,
    },

    /// Tuple: (a, b, c)
    Tuple(ExprRange),

    /// Range: start..end or start..=end
    Range {
        start: Option<ExprId>,
        end: Option<ExprId>,
        inclusive: bool,
    },

    // ===== Variant constructors =====

    /// Ok(value)
    Ok(Option<ExprId>),

    /// Err(value)
    Err(Option<ExprId>),

    /// Some(value)
    Some(ExprId),

    /// None
    None,

    // ===== Control =====

    /// Return from function
    Return(Option<ExprId>),

    /// Break from loop
    Break(Option<ExprId>),

    /// Continue loop
    Continue,

    /// Await async operation
    Await(ExprId),

    /// Propagate error: expr?
    Try(ExprId),

    /// Assignment: target = value
    Assign {
        target: ExprId,
        value: ExprId,
    },

    // ===== function_seq / function_exp =====

    /// Sequential expression construct: run, try, match
    ///
    /// Contains a sequence of expressions where order matters.
    /// Positional expressions allowed (it's a sequence, not parameters).
    FunctionSeq(FunctionSeq),

    /// Named expression construct: map, filter, fold, etc.
    ///
    /// Contains named expressions (`name: value`).
    /// Requires named property syntax - positional not allowed.
    FunctionExp(FunctionExp),

    // ===== Error recovery =====

    /// Parse error placeholder
    Error,
}

impl fmt::Debug for ExprKind {
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
            ExprKind::Config(n) => write!(f, "Config({n:?})"),
            ExprKind::SelfRef => write!(f, "SelfRef"),
            ExprKind::FunctionRef(n) => write!(f, "FunctionRef({n:?})"),
            ExprKind::HashLength => write!(f, "HashLength"),
            ExprKind::Binary { op, left, right } => {
                write!(f, "Binary({op:?}, {left:?}, {right:?})")
            }
            ExprKind::Unary { op, operand } => write!(f, "Unary({op:?}, {operand:?})"),
            ExprKind::Call { func, args } => write!(f, "Call({func:?}, {args:?})"),
            ExprKind::CallNamed { func, args } => write!(f, "CallNamed({func:?}, {args:?})"),
            ExprKind::MethodCall { receiver, method, args } => {
                write!(f, "MethodCall({receiver:?}, {method:?}, {args:?})")
            }
            ExprKind::Field { receiver, field } => {
                write!(f, "Field({receiver:?}, {field:?})")
            }
            ExprKind::Index { receiver, index } => {
                write!(f, "Index({receiver:?}, {index:?})")
            }
            ExprKind::If { cond, then_branch, else_branch } => {
                write!(f, "If({cond:?}, {then_branch:?}, {else_branch:?})")
            }
            ExprKind::Match { scrutinee, arms } => {
                write!(f, "Match({scrutinee:?}, {arms:?})")
            }
            ExprKind::For { binding, iter, guard, body, is_yield } => {
                write!(f, "For({binding:?}, {iter:?}, {guard:?}, {body:?}, yield={is_yield})")
            }
            ExprKind::Loop { body } => write!(f, "Loop({body:?})"),
            ExprKind::Block { stmts, result } => write!(f, "Block({stmts:?}, {result:?})"),
            ExprKind::Let { pattern, ty, init, mutable } => {
                write!(f, "Let({pattern:?}, {ty:?}, {init:?}, mutable={mutable})")
            }
            ExprKind::Lambda { params, ret_ty, body } => {
                write!(f, "Lambda({params:?}, {ret_ty:?}, {body:?})")
            }
            ExprKind::List(exprs) => write!(f, "List({exprs:?})"),
            ExprKind::Map(entries) => write!(f, "Map({entries:?})"),
            ExprKind::Struct { name, fields } => write!(f, "Struct({name:?}, {fields:?})"),
            ExprKind::Tuple(exprs) => write!(f, "Tuple({exprs:?})"),
            ExprKind::Range { start, end, inclusive } => {
                write!(f, "Range({start:?}, {end:?}, inclusive={inclusive})")
            }
            ExprKind::Ok(inner) => write!(f, "Ok({inner:?})"),
            ExprKind::Err(inner) => write!(f, "Err({inner:?})"),
            ExprKind::Some(inner) => write!(f, "Some({inner:?})"),
            ExprKind::None => write!(f, "None"),
            ExprKind::Return(val) => write!(f, "Return({val:?})"),
            ExprKind::Break(val) => write!(f, "Break({val:?})"),
            ExprKind::Continue => write!(f, "Continue"),
            ExprKind::Await(inner) => write!(f, "Await({inner:?})"),
            ExprKind::Try(inner) => write!(f, "Try({inner:?})"),
            ExprKind::Assign { target, value } => write!(f, "Assign({target:?}, {value:?})"),
            ExprKind::FunctionSeq(seq) => write!(f, "FunctionSeq({seq:?})"),
            ExprKind::FunctionExp(exp) => write!(f, "FunctionExp({exp:?})"),
            ExprKind::Error => write!(f, "Error"),
        }
    }
}
