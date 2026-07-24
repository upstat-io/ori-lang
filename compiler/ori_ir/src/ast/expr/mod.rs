//! Expression Types
//!
//! Core expression nodes and variants.
//!
//! # Specification
//!
//! - Syntax: `docs/ori_lang/v2026/spec/grammar.ebnf` § EXPRESSIONS
//! - Semantics: `docs/ori_lang/v2026/spec/operator-rules.md`
//! - Prose: `docs/ori_lang/v2026/spec/09-expressions.md`
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
    AccessStepRange, ArmRange, CallArgRange, FieldInitRange, ListElementRange, MapElementRange,
    MapEntryRange, StructLitFieldRange, TemplatePartRange,
};
use crate::token::{DurationUnit, SizeUnit};
use crate::{
    BindingPatternId, ExprId, ExprRange, FunctionExpId, FunctionSeqId, Mutability, Name,
    ParsedTypeId, Span, Spanned, StmtRange,
};

mod debug;

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

/// A single access step in an assignment target's chain.
///
/// `state.items[i] = x` decomposes a root (`state`) plus an ordered chain of
/// steps (`.items`, `[i]`). Steps are arena-allocated and referenced by an
/// [`AccessStepRange`] so [`ExprKind`] stays `Copy` and within its byte budget.
///
/// # Salsa Compatibility
/// Has all required traits: Copy, Clone, Eq, `PartialEq`, Hash, Debug
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub enum AccessStep {
    /// Index step: `[expr]`.
    Index(ExprId),
    /// Field step: `.name`.
    Field(Name),
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
    ///
    /// Call-site type arguments (`receiver.method<T>(args)`) are NOT inlined here
    /// — they are rare, so storing them on the hot node would bloat every
    /// expression. They live in the arena `method_call_type_args` side-table keyed
    /// by this expression's `ExprId` and are consumed by method-generic inference.
    MethodCall {
        receiver: ExprId,
        method: Name,
        args: ExprRange,
    },

    /// Method call with named args: receiver.method(a: 1, b: 2)
    ///
    /// Call-site type arguments live in the arena `method_call_type_args`
    /// side-table keyed by this expression's `ExprId` (see `MethodCall`).
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

    /// For loop: `for pattern in iter do body` or `for:label pattern in iter do body`
    For {
        /// `Name::EMPTY` = no label.
        label: Name,
        pattern: BindingPatternId,
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

    /// While loop: `while cond do body` or `while:label cond do body`.
    ///
    /// Sugar for `loop { if !cond then break; body }`; desugared in `ori_canon`.
    While {
        /// `Name::EMPTY` = no label.
        label: Name,
        cond: ExprId,
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

    /// Struct literal: Point { x: 0, y: 0 } or module-qualified geom.Point { .. }.
    ///
    /// `type_path` is the parsed type-path head (`type_path = identifier
    /// { "." identifier }`): `Named` for a bare name, `AssociatedType`-chain for
    /// a module-qualified path. Shared with the type-annotation representation so
    /// one resolver serves both positions.
    Struct {
        type_path: ParsedTypeId,
        fields: FieldInitRange,
    },

    /// Struct literal with spread: Point { ...base, x: 10 } or geom.Point { ...base }.
    ///
    /// Uses `StructLitFieldRange` which can contain both field inits and spreads.
    /// The "later wins" semantics means spreads and explicit fields are applied
    /// in order, with later values overwriting earlier ones.
    StructWithSpread {
        type_path: ParsedTypeId,
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

    /// Assignment target as a root plus an ordered access-step chain.
    ///
    /// Models the left side of `state.items[i] = x` as `root` (`state`) plus
    /// `steps` (`.items`, `[i]`). The type-directed desugar (a later phase)
    /// eliminates this node; until then it mirrors the index/field-assignment
    /// path. `steps` is an arena range, never a `Vec`, to keep `ExprKind` `Copy`
    /// and within its byte budget.
    AssignTarget {
        root: ExprId,
        steps: AccessStepRange,
    },

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

// Size assertions guard ExprKind/Expr layout against accidental regressions.
#[cfg(target_pointer_width = "64")]
mod size_asserts {
    use super::{Expr, ExprKind};
    crate::static_assert_size!(ExprKind, 24);
    crate::static_assert_size!(Expr, 32);
}
