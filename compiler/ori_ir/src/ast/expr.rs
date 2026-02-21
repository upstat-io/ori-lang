//! Expression Types
//!
//! Core expression nodes and variants.
//!
//! # Specification
//!
//! - Syntax: `docs/ori_lang/0.1-alpha/spec/grammar.ebnf` § EXPRESSIONS
//! - Semantics: `docs/ori_lang/0.1-alpha/spec/operator-rules.md`
//! - Prose: `docs/ori_lang/0.1-alpha/spec/09-expressions.md`
//!
//! # Design Notes
//! Per design spec A-data-structures.md:
//! - No `Box<Expr>`, use `ExprId(u32)` indices
//! - Contiguous arrays for cache locality
//! - All types have Salsa-required traits (Clone, Eq, Hash, Debug)

use std::fmt;
use std::hash::{Hash, Hasher};

use super::operators::{BinaryOp, UnaryOp};
use super::ranges::{
    ArmRange, CallArgRange, FieldInitRange, ListElementRange, MapElementRange, MapEntryRange,
    StructLitFieldRange, TemplatePartRange,
};
use crate::token::{DurationUnit, SizeUnit};
use crate::{
    BindingPatternId, ExprId, ExprRange, FunctionExpId, FunctionSeqId, Mutability, Name,
    ParsedTypeId, Span, Spanned, StmtRange,
};

/// Expression node.
///
/// # Salsa Compatibility
/// Has all required traits: Clone, Eq, `PartialEq`, Hash, Debug
#[derive(Copy, Clone, Eq, PartialEq)]
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

/// A single interpolation segment in a template literal.
///
/// Each part represents: `{expr:format_spec}text_after`
/// The `text_after` is the text between this interpolation's `}` and the
/// next `{` (or closing backtick).
///
/// # Salsa Compatibility
/// Has all required traits: Copy, Clone, Eq, `PartialEq`, Hash, Debug
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct TemplatePart {
    /// The interpolated expression.
    pub expr: ExprId,
    /// Raw format spec text (interned). `Name::EMPTY` if no format spec.
    pub format_spec: Name,
    /// Text segment after this interpolation (from `TemplateMiddle`/`TemplateTail`).
    pub text_after: Name,
}

/// Expression variants.
///
/// All children are indices, not boxes. Per design:
/// "No `Box<Expr>`, use `ExprId(u32)` indices"
///
/// # Salsa Compatibility
/// Has all required traits: Clone, Eq, `PartialEq`, Hash, Debug
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub enum ExprKind {
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
    Duration { value: u64, unit: DurationUnit },

    /// Size: 4kb, 10mb
    Size { value: u64, unit: SizeUnit },

    /// Unit: ()
    Unit,

    /// Variable reference
    Ident(Name),

    /// Constant reference: $name
    Const(Name),

    /// Self reference: self
    SelfRef,

    /// Function reference: @name
    FunctionRef(Name),

    /// Hash in index context (refers to length): #
    HashLength,

    /// Binary operation: left op right
    Binary {
        op: BinaryOp,
        left: ExprId,
        right: ExprId,
    },

    /// Unary operation: op operand
    Unary { op: UnaryOp, operand: ExprId },

    /// Function call with positional args: func(arg)
    /// Only valid for single-param functions.
    Call { func: ExprId, args: ExprRange },

    /// Function call with named args: func(a: 1, b: 2)
    /// Required for multi-param functions.
    CallNamed { func: ExprId, args: CallArgRange },

    /// Method call: receiver.method(args...)
    MethodCall {
        receiver: ExprId,
        method: Name,
        args: ExprRange,
    },

    /// Method call with named args: receiver.method(a: 1, b: 2)
    MethodCallNamed {
        receiver: ExprId,
        method: Name,
        args: CallArgRange,
    },

    /// Field access: receiver.field
    Field { receiver: ExprId, field: Name },

    /// Index access: `receiver[index]`
    Index { receiver: ExprId, index: ExprId },

    /// Conditional: if cond then t else e
    If {
        cond: ExprId,
        then_branch: ExprId,
        /// `ExprId::INVALID` = no else branch.
        else_branch: ExprId,
    },

    /// Match expression (statement form): match value { arms }
    Match { scrutinee: ExprId, arms: ArmRange },

    /// For loop: `for x in iter do body` or `for:label x in iter do body`
    For {
        /// `Name::EMPTY` = no label.
        label: Name,
        binding: Name,
        iter: ExprId,
        /// `ExprId::INVALID` = no guard.
        guard: ExprId,
        body: ExprId,
        is_yield: bool,
    },

    /// Loop: `loop(body)` or `loop:label(body)`
    Loop {
        /// `Name::EMPTY` = no label.
        label: Name,
        body: ExprId,
    },

    /// Block: { stmts; result }
    Block {
        stmts: StmtRange,
        /// `ExprId::INVALID` = no result (unit block).
        result: ExprId,
    },

    /// Let binding: let pattern = init
    ///
    /// Pattern is arena-allocated via `BindingPatternId`.
    Let {
        pattern: BindingPatternId,
        /// Type annotation (`ParsedTypeId::INVALID` = no annotation).
        ty: ParsedTypeId,
        init: ExprId,
        mutable: Mutability,
    },

    /// Lambda: params -> body
    Lambda {
        params: super::ranges::ParamRange,
        /// Return type annotation (`ParsedTypeId::INVALID` = no annotation).
        ret_ty: ParsedTypeId,
        body: ExprId,
    },

    /// List literal: [a, b, c]
    List(ExprRange),

    /// List literal with spread: [...a, x, ...b]
    ///
    /// Uses `ListElementRange` which can contain both regular values and spreads.
    /// Spread elements are expanded at runtime, concatenating their contents
    /// into the resulting list in order.
    ListWithSpread(ListElementRange),

    /// Map literal: {k: v, ...}
    Map(MapEntryRange),

    /// Map literal with spread: {...base, k: v}
    ///
    /// Uses `MapElementRange` which can contain both entries and spreads.
    /// The "later wins" semantics means spreads and explicit entries are applied
    /// in order, with later values overwriting earlier ones.
    MapWithSpread(MapElementRange),

    /// Struct literal: Point { x: 0, y: 0 }
    Struct { name: Name, fields: FieldInitRange },

    /// Struct literal with spread: Point { ...base, x: 10 }
    ///
    /// Uses `StructLitFieldRange` which can contain both field inits and spreads.
    /// The "later wins" semantics means spreads and explicit fields are applied
    /// in order, with later values overwriting earlier ones.
    StructWithSpread {
        name: Name,
        fields: StructLitFieldRange,
    },

    /// Tuple: (a, b, c)
    Tuple(ExprRange),

    /// Range: start..end or start..=end or start..end by step
    Range {
        /// `ExprId::INVALID` = unbounded start.
        start: ExprId,
        /// `ExprId::INVALID` = unbounded end.
        end: ExprId,
        /// `ExprId::INVALID` = no step.
        step: ExprId,
        inclusive: bool,
    },

    /// Ok(value) — `ExprId::INVALID` = `Ok(())`.
    Ok(ExprId),

    /// Err(value) — `ExprId::INVALID` = `Err(())`.
    Err(ExprId),

    /// Some(value)
    Some(ExprId),

    /// None
    None,

    /// Break from loop: `break`, `break value`, `break:label`, `break:label value`.
    /// `Name::EMPTY` = no label, `ExprId::INVALID` = no value.
    Break { label: Name, value: ExprId },

    /// Continue loop: `continue`, `continue value`, `continue:label`, `continue:label value`.
    /// `Name::EMPTY` = no label, `ExprId::INVALID` = no value.
    /// Value is only valid in `for...yield` context (substitutes the element).
    /// Error E0861 if value provided in `loop()` context.
    Continue { label: Name, value: ExprId },

    /// Await async operation
    Await(ExprId),

    /// Propagate error: expr?
    Try(ExprId),

    /// Unsafe block: `unsafe { expr }`
    ///
    /// Discharges the `Unsafe` capability within its scope.
    /// The inner `ExprId` points to a `Block` expression.
    /// At runtime, evaluates to the inner expression (transparent).
    Unsafe(ExprId),

    /// Type cast: `expr as type` (infallible) or `expr as? type` (fallible)
    ///
    /// - `as`: Infallible conversion (e.g., `42 as float`)
    /// - `as?`: Fallible conversion returning `Option<T>` (e.g., `"42" as? int`)
    Cast {
        expr: ExprId,
        /// Target type (arena-allocated).
        ty: ParsedTypeId,
        /// True for `as?` (fallible), false for `as` (infallible)
        fallible: bool,
    },

    /// Assignment: target = value
    Assign { target: ExprId, value: ExprId },

    /// Capability provision: with Http = `RealHttp` { ... } in body
    WithCapability {
        /// The capability name (e.g., Http)
        capability: Name,
        /// The provider expression (e.g., `RealHttp` { `base_url`: "..." })
        provider: ExprId,
        /// The body expression where the capability is in scope
        body: ExprId,
    },

    /// Sequential expression construct: run, try, match
    ///
    /// Contains a sequence of expressions where order matters.
    /// Positional expressions allowed (it's a sequence, not parameters).
    /// Arena-allocated via `FunctionSeqId` for compact `ExprKind`.
    FunctionSeq(FunctionSeqId),

    /// Named expression construct: map, filter, fold, etc.
    ///
    /// Contains named expressions (`name: value`).
    /// Requires named property syntax - positional not allowed.
    /// Arena-allocated via `FunctionExpId` for compact `ExprKind`.
    FunctionExp(FunctionExpId),

    /// Template literal without interpolation: `` `hello world` ``
    TemplateFull(Name),

    /// Template literal with interpolation: `` `hello {name}!` ``
    TemplateLiteral {
        /// Text from the `TemplateHead` token (before first interpolation).
        head: Name,
        /// Interpolation parts (expression + optional format spec + text after).
        parts: TemplatePartRange,
    },

    /// Parse error placeholder
    Error,
}

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
                binding,
                iter,
                guard,
                body,
                is_yield,
            } => {
                write!(
                    f,
                    "For({label:?}, {binding:?}, {iter:?}, {guard:?}, {body:?}, yield={is_yield})"
                )
            }
            ExprKind::Loop { label, body } => write!(f, "Loop({label:?}, {body:?})"),
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

// Size assertions to prevent accidental regressions.
// Phase 1 target: ExprKind ~24 bytes, Expr ~32 bytes.
// These assertions will be tightened as each step lands.
#[cfg(target_pointer_width = "64")]
mod size_asserts {
    use super::{Expr, ExprKind};
    crate::static_assert_size!(ExprKind, 24);
    crate::static_assert_size!(Expr, 32);
}
