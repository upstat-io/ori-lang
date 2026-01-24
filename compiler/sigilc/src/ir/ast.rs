//! Flat AST types using arena allocation.
//!
//! Per design spec A-data-structuresmd:
//! - No Box<Expr>, use ExprId(u32) indices
//! - Contiguous arrays for cache locality
//! - All types have Salsa-required traits (Clone, Eq, Hash, Debug)

use super::{Name, Span, TypeId, ExprId, ExprRange, StmtRange, Spanned};
use super::token::{DurationUnit, SizeUnit};
use std::fmt;
use std::hash::{Hash, Hasher};

/// Expression node.
///
/// # Salsa Compatibility
/// Has all required traits: Clone, Eq, PartialEq, Hash, Debug
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
/// Has all required traits: Clone, Eq, PartialEq, Hash, Debug
#[derive(Clone, Eq, PartialEq, Hash)]
pub enum ExprKind {
    // ===== Literals (no children) =====

    /// Integer literal: 42, 1_000
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
        ty: Option<TypeId>,
        init: ExprId,
        mutable: bool,
    },

    /// Lambda: params -> body
    Lambda {
        params: ParamRange,
        ret_ty: Option<TypeId>,
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
            ExprKind::Int(n) => write!(f, "Int({})", n),
            ExprKind::Float(bits) => write!(f, "Float({})", f64::from_bits(*bits)),
            ExprKind::Bool(b) => write!(f, "Bool({})", b),
            ExprKind::String(n) => write!(f, "String({:?})", n),
            ExprKind::Char(c) => write!(f, "Char({:?})", c),
            ExprKind::Duration { value, unit } => write!(f, "Duration({}{:?})", value, unit),
            ExprKind::Size { value, unit } => write!(f, "Size({}{:?})", value, unit),
            ExprKind::Unit => write!(f, "Unit"),
            ExprKind::Ident(n) => write!(f, "Ident({:?})", n),
            ExprKind::Config(n) => write!(f, "Config({:?})", n),
            ExprKind::SelfRef => write!(f, "SelfRef"),
            ExprKind::FunctionRef(n) => write!(f, "FunctionRef({:?})", n),
            ExprKind::HashLength => write!(f, "HashLength"),
            ExprKind::Binary { op, left, right } => {
                write!(f, "Binary({:?}, {:?}, {:?})", op, left, right)
            }
            ExprKind::Unary { op, operand } => write!(f, "Unary({:?}, {:?})", op, operand),
            ExprKind::Call { func, args } => write!(f, "Call({:?}, {:?})", func, args),
            ExprKind::CallNamed { func, args } => write!(f, "CallNamed({:?}, {:?})", func, args),
            ExprKind::MethodCall { receiver, method, args } => {
                write!(f, "MethodCall({:?}, {:?}, {:?})", receiver, method, args)
            }
            ExprKind::Field { receiver, field } => {
                write!(f, "Field({:?}, {:?})", receiver, field)
            }
            ExprKind::Index { receiver, index } => {
                write!(f, "Index({:?}, {:?})", receiver, index)
            }
            ExprKind::If { cond, then_branch, else_branch } => {
                write!(f, "If({:?}, {:?}, {:?})", cond, then_branch, else_branch)
            }
            ExprKind::Match { scrutinee, arms } => {
                write!(f, "Match({:?}, {:?})", scrutinee, arms)
            }
            ExprKind::For { binding, iter, guard, body, is_yield } => {
                write!(f, "For({:?}, {:?}, {:?}, {:?}, yield={})", binding, iter, guard, body, is_yield)
            }
            ExprKind::Loop { body } => write!(f, "Loop({:?})", body),
            ExprKind::Block { stmts, result } => write!(f, "Block({:?}, {:?})", stmts, result),
            ExprKind::Let { pattern, ty, init, mutable } => {
                write!(f, "Let({:?}, {:?}, {:?}, mutable={})", pattern, ty, init, mutable)
            }
            ExprKind::Lambda { params, ret_ty, body } => {
                write!(f, "Lambda({:?}, {:?}, {:?})", params, ret_ty, body)
            }
            ExprKind::List(exprs) => write!(f, "List({:?})", exprs),
            ExprKind::Map(entries) => write!(f, "Map({:?})", entries),
            ExprKind::Struct { name, fields } => write!(f, "Struct({:?}, {:?})", name, fields),
            ExprKind::Tuple(exprs) => write!(f, "Tuple({:?})", exprs),
            ExprKind::Range { start, end, inclusive } => {
                write!(f, "Range({:?}, {:?}, inclusive={})", start, end, inclusive)
            }
            ExprKind::Ok(inner) => write!(f, "Ok({:?})", inner),
            ExprKind::Err(inner) => write!(f, "Err({:?})", inner),
            ExprKind::Some(inner) => write!(f, "Some({:?})", inner),
            ExprKind::None => write!(f, "None"),
            ExprKind::Return(val) => write!(f, "Return({:?})", val),
            ExprKind::Break(val) => write!(f, "Break({:?})", val),
            ExprKind::Continue => write!(f, "Continue"),
            ExprKind::Await(inner) => write!(f, "Await({:?})", inner),
            ExprKind::Try(inner) => write!(f, "Try({:?})", inner),
            ExprKind::Assign { target, value } => write!(f, "Assign({:?}, {:?})", target, value),
            ExprKind::FunctionSeq(seq) => write!(f, "FunctionSeq({:?})", seq),
            ExprKind::FunctionExp(exp) => write!(f, "FunctionExp({:?})", exp),
            ExprKind::Error => write!(f, "Error"),
        }
    }
}

/// Binary operators.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub enum BinaryOp {
    // Arithmetic
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    FloorDiv,

    // Comparison
    Eq,
    NotEq,
    Lt,
    LtEq,
    Gt,
    GtEq,

    // Logical
    And,
    Or,

    // Bitwise
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,

    // Other
    Range,
    RangeInclusive,
    Coalesce,
}

/// Unary operators.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub enum UnaryOp {
    Neg,
    Not,
    BitNot,
    Try,
}

/// Statement node.
#[derive(Clone, Eq, PartialEq, Hash)]
pub struct Stmt {
    pub kind: StmtKind,
    pub span: Span,
}

impl Stmt {
    pub fn new(kind: StmtKind, span: Span) -> Self {
        Stmt { kind, span }
    }
}

impl fmt::Debug for Stmt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?} @ {:?}", self.kind, self.span)
    }
}

impl Spanned for Stmt {
    fn span(&self) -> Span {
        self.span
    }
}

/// Statement kinds.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum StmtKind {
    /// Expression statement
    Expr(ExprId),

    /// Let binding (also available as expression)
    Let {
        pattern: BindingPattern,
        ty: Option<TypeId>,
        init: ExprId,
        mutable: bool,
    },
}

/// Parameter in a function or lambda.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct Param {
    pub name: Name,
    pub ty: Option<TypeId>,
    pub span: Span,
}

impl Spanned for Param {
    fn span(&self) -> Span {
        self.span
    }
}

/// Range of parameters.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Default)]
#[repr(C)]
pub struct ParamRange {
    pub start: u32,
    pub len: u16,
}

impl ParamRange {
    pub const EMPTY: ParamRange = ParamRange { start: 0, len: 0 };

    #[inline]
    pub const fn new(start: u32, len: u16) -> Self {
        ParamRange { start, len }
    }

    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[inline]
    pub const fn len(&self) -> usize {
        self.len as usize
    }
}

impl fmt::Debug for ParamRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ParamRange({}..{})", self.start, self.start + self.len as u32)
    }
}

/// Function definition.
#[derive(Clone, Eq, PartialEq, Hash)]
pub struct Function {
    pub name: Name,
    pub params: ParamRange,
    pub return_ty: Option<TypeId>,
    pub body: ExprId,
    pub span: Span,
    pub is_public: bool,
}

impl fmt::Debug for Function {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Function {{ name: {:?}, params: {:?}, ret: {:?}, public: {} }}",
            self.name, self.params, self.return_ty, self.is_public
        )
    }
}

impl Spanned for Function {
    fn span(&self) -> Span {
        self.span
    }
}

/// Test definition.
#[derive(Clone, Eq, PartialEq, Hash)]
pub struct TestDef {
    pub name: Name,
    pub targets: Vec<Name>,
    pub params: ParamRange,
    pub return_ty: Option<TypeId>,
    pub body: ExprId,
    pub span: Span,
    /// If set, this test is skipped with the given reason.
    pub skip_reason: Option<Name>,
    /// If set, this test expects compilation to fail with an error
    /// containing this substring.
    pub compile_fail_expected: Option<Name>,
    /// If set, this test expects runtime failure with an error
    /// containing this substring.
    pub fail_expected: Option<Name>,
}

impl fmt::Debug for TestDef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "TestDef {{ name: {:?}, targets: {:?}, skip: {:?}, compile_fail: {:?}, fail: {:?} }}",
            self.name, self.targets, self.skip_reason, self.compile_fail_expected, self.fail_expected
        )
    }
}

impl Spanned for TestDef {
    fn span(&self) -> Span {
        self.span
    }
}

// =============================================================================
// Imports
// =============================================================================

/// A use/import statement.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct UseDef {
    /// Import path - either relative ('./math', '../utils') or module (std.math)
    pub path: ImportPath,
    /// Items being imported
    pub items: Vec<UseItem>,
    /// Source span
    pub span: Span,
}

/// Import path type.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum ImportPath {
    /// Relative path: './math', '../utils/helpers'
    Relative(Name),
    /// Module path: std.math, std.collections
    Module(Vec<Name>),
}

/// A single imported item.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct UseItem {
    /// Name of the item being imported
    pub name: Name,
    /// Optional alias: `name as alias`
    pub alias: Option<Name>,
    /// Whether this is a private import (::name)
    pub is_private: bool,
}

/// A parsed module (collection of items).
#[derive(Clone, Eq, PartialEq, Hash, Default)]
pub struct Module {
    /// Import statements
    pub imports: Vec<UseDef>,
    /// Function definitions
    pub functions: Vec<Function>,
    /// Test definitions
    pub tests: Vec<TestDef>,
}

impl Module {
    pub fn new() -> Self {
        Module {
            imports: Vec::new(),
            functions: Vec::new(),
            tests: Vec::new(),
        }
    }
}

impl fmt::Debug for Module {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Module {{ {} functions, {} tests }}", self.functions.len(), self.tests.len())
    }
}

// =============================================================================
// Binding and Match Patterns
// =============================================================================

/// Binding pattern for let expressions.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum BindingPattern {
    /// Simple name binding: let x = ...
    Name(Name),
    /// Tuple destructuring: let (a, b) = ...
    Tuple(Vec<BindingPattern>),
    /// Struct destructuring: let { x, y } = ...
    Struct {
        fields: Vec<(Name, Option<BindingPattern>)>,
    },
    /// List destructuring: let [head, ..tail] = ...
    List {
        elements: Vec<BindingPattern>,
        rest: Option<Name>,
    },
    /// Wildcard: let _ = ...
    Wildcard,
}

/// Match pattern for match expressions.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum MatchPattern {
    /// Wildcard: _
    Wildcard,
    /// Binding: x
    Binding(Name),
    /// Literal: 42, "hello", true
    Literal(ExprId),
    /// Variant: Some(x), Ok(value)
    Variant {
        name: Name,
        inner: Option<Box<MatchPattern>>,
    },
    /// Struct: { x, y }
    Struct {
        fields: Vec<(Name, Option<MatchPattern>)>,
    },
    /// Tuple: (a, b)
    Tuple(Vec<MatchPattern>),
    /// List: [a, b, ..rest]
    List {
        elements: Vec<MatchPattern>,
        rest: Option<Name>,
    },
    /// Range: 1..10
    Range {
        start: Option<ExprId>,
        end: Option<ExprId>,
        inclusive: bool,
    },
    /// Or pattern: A | B
    Or(Vec<MatchPattern>),
    /// At pattern: x @ Some(_)
    At {
        name: Name,
        pattern: Box<MatchPattern>,
    },
}

/// Match arm.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct MatchArm {
    pub pattern: MatchPattern,
    pub guard: Option<ExprId>,
    pub body: ExprId,
    pub span: Span,
}

// =============================================================================
// Collection Literals
// =============================================================================

/// Map entry in a map literal.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct MapEntry {
    pub key: ExprId,
    pub value: ExprId,
    pub span: Span,
}

/// Field initializer in a struct literal.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct FieldInit {
    pub name: Name,
    pub value: Option<ExprId>,
    pub span: Span,
}

// =============================================================================
// Arena Range Types
// =============================================================================

/// Range of match arms in the arena.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Default)]
#[repr(C)]
pub struct ArmRange {
    pub start: u32,
    pub len: u16,
}

impl ArmRange {
    pub const EMPTY: ArmRange = ArmRange { start: 0, len: 0 };

    #[inline]
    pub const fn new(start: u32, len: u16) -> Self {
        ArmRange { start, len }
    }

    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[inline]
    pub const fn len(&self) -> usize {
        self.len as usize
    }
}

impl fmt::Debug for ArmRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ArmRange({}..{})", self.start, self.start + self.len as u32)
    }
}

/// Range of map entries in the arena.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Default)]
#[repr(C)]
pub struct MapEntryRange {
    pub start: u32,
    pub len: u16,
}

impl MapEntryRange {
    pub const EMPTY: MapEntryRange = MapEntryRange { start: 0, len: 0 };

    #[inline]
    pub const fn new(start: u32, len: u16) -> Self {
        MapEntryRange { start, len }
    }

    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[inline]
    pub const fn len(&self) -> usize {
        self.len as usize
    }
}

impl fmt::Debug for MapEntryRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MapEntryRange({}..{})", self.start, self.start + self.len as u32)
    }
}

/// Range of field initializers in the arena.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Default)]
#[repr(C)]
pub struct FieldInitRange {
    pub start: u32,
    pub len: u16,
}

impl FieldInitRange {
    pub const EMPTY: FieldInitRange = FieldInitRange { start: 0, len: 0 };

    #[inline]
    pub const fn new(start: u32, len: u16) -> Self {
        FieldInitRange { start, len }
    }

    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[inline]
    pub const fn len(&self) -> usize {
        self.len as usize
    }
}

impl fmt::Debug for FieldInitRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "FieldInitRange({}..{})", self.start, self.start + self.len as u32)
    }
}

// =============================================================================
// function_seq Types
// =============================================================================

/// Element within a function_seq (run/try).
///
/// Can be either a let binding or a statement expression.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum SeqBinding {
    /// let [mut] pattern [: Type] = expr
    Let {
        pattern: BindingPattern,
        ty: Option<TypeId>,
        value: ExprId,
        mutable: bool,
        span: Span,
    },
    /// Statement expression (evaluated for side effects, e.g., assignment)
    Stmt {
        expr: ExprId,
        span: Span,
    },
}

impl Spanned for SeqBinding {
    fn span(&self) -> Span {
        match self {
            SeqBinding::Let { span, .. } => *span,
            SeqBinding::Stmt { span, .. } => *span,
        }
    }
}

/// Range of sequence bindings in the arena.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Default)]
#[repr(C)]
pub struct SeqBindingRange {
    pub start: u32,
    pub len: u16,
}

impl SeqBindingRange {
    pub const EMPTY: SeqBindingRange = SeqBindingRange { start: 0, len: 0 };

    #[inline]
    pub const fn new(start: u32, len: u16) -> Self {
        SeqBindingRange { start, len }
    }

    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[inline]
    pub const fn len(&self) -> usize {
        self.len as usize
    }
}

impl fmt::Debug for SeqBindingRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SeqBindingRange({}..{})", self.start, self.start + self.len as u32)
    }
}

/// Sequential expression construct (function_seq).
///
/// Contains a sequence of expressions where order matters.
/// NOT a function call - fundamentally different structure.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum FunctionSeq {
    /// run(let x = a, let y = b, result)
    Run {
        bindings: SeqBindingRange,
        result: ExprId,
        span: Span,
    },

    /// try(let x = fallible()?, let y = other()?, Ok(x + y))
    Try {
        bindings: SeqBindingRange,
        result: ExprId,
        span: Span,
    },

    /// match(scrutinee, Pattern -> expr, ...)
    Match {
        scrutinee: ExprId,
        arms: ArmRange,
        span: Span,
    },
}

impl FunctionSeq {
    pub fn span(&self) -> Span {
        match self {
            FunctionSeq::Run { span, .. } => *span,
            FunctionSeq::Try { span, .. } => *span,
            FunctionSeq::Match { span, .. } => *span,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            FunctionSeq::Run { .. } => "run",
            FunctionSeq::Try { .. } => "try",
            FunctionSeq::Match { .. } => "match",
        }
    }
}

impl Spanned for FunctionSeq {
    fn span(&self) -> Span {
        self.span()
    }
}

// =============================================================================
// function_exp Types
// =============================================================================

/// Named expression for function_exp.
///
/// Represents: `name: expr`
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct NamedExpr {
    pub name: Name,
    pub value: ExprId,
    pub span: Span,
}

impl Spanned for NamedExpr {
    fn span(&self) -> Span {
        self.span
    }
}

/// Range of named expressions in the arena.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Default)]
#[repr(C)]
pub struct NamedExprRange {
    pub start: u32,
    pub len: u16,
}

impl NamedExprRange {
    pub const EMPTY: NamedExprRange = NamedExprRange { start: 0, len: 0 };

    #[inline]
    pub const fn new(start: u32, len: u16) -> Self {
        NamedExprRange { start, len }
    }

    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[inline]
    pub const fn len(&self) -> usize {
        self.len as usize
    }
}

impl fmt::Debug for NamedExprRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NamedExprRange({}..{})", self.start, self.start + self.len as u32)
    }
}

/// Kind of function_exp.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub enum FunctionExpKind {
    Map,
    Filter,
    Fold,
    Find,
    Collect,
    Recurse,
    Parallel,
    Spawn,
    Timeout,
    Retry,
    Cache,
    Validate,
    With,
    // Core patterns
    Assert,
    AssertEq,
    AssertNe,
    Len,
    IsEmpty,
    IsSome,
    IsNone,
    IsOk,
    IsErr,
    Print,
    Panic,
    Compare,
    Min,
    Max,
}

impl FunctionExpKind {
    pub fn name(self) -> &'static str {
        match self {
            FunctionExpKind::Map => "map",
            FunctionExpKind::Filter => "filter",
            FunctionExpKind::Fold => "fold",
            FunctionExpKind::Find => "find",
            FunctionExpKind::Collect => "collect",
            FunctionExpKind::Recurse => "recurse",
            FunctionExpKind::Parallel => "parallel",
            FunctionExpKind::Spawn => "spawn",
            FunctionExpKind::Timeout => "timeout",
            FunctionExpKind::Retry => "retry",
            FunctionExpKind::Cache => "cache",
            FunctionExpKind::Validate => "validate",
            FunctionExpKind::With => "with",
            FunctionExpKind::Assert => "assert",
            FunctionExpKind::AssertEq => "assert_eq",
            FunctionExpKind::AssertNe => "assert_ne",
            FunctionExpKind::Len => "len",
            FunctionExpKind::IsEmpty => "is_empty",
            FunctionExpKind::IsSome => "is_some",
            FunctionExpKind::IsNone => "is_none",
            FunctionExpKind::IsOk => "is_ok",
            FunctionExpKind::IsErr => "is_err",
            FunctionExpKind::Print => "print",
            FunctionExpKind::Panic => "panic",
            FunctionExpKind::Compare => "compare",
            FunctionExpKind::Min => "min",
            FunctionExpKind::Max => "max",
        }
    }
}

/// Named expression construct (function_exp).
///
/// Contains named expressions (`name: value`).
/// Requires named property syntax - positional not allowed.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct FunctionExp {
    pub kind: FunctionExpKind,
    pub props: NamedExprRange,
    pub span: Span,
}

impl Spanned for FunctionExp {
    fn span(&self) -> Span {
        self.span
    }
}

// =============================================================================
// Call Arguments
// =============================================================================

/// Named argument for function calls.
///
/// Single-param functions can use positional (name is None).
/// Multi-param functions require named arguments.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct CallArg {
    pub name: Option<Name>,
    pub value: ExprId,
    pub span: Span,
}

impl Spanned for CallArg {
    fn span(&self) -> Span {
        self.span
    }
}

/// Range of call arguments in the arena.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Default)]
#[repr(C)]
pub struct CallArgRange {
    pub start: u32,
    pub len: u16,
}

impl CallArgRange {
    pub const EMPTY: CallArgRange = CallArgRange { start: 0, len: 0 };

    #[inline]
    pub const fn new(start: u32, len: u16) -> Self {
        CallArgRange { start, len }
    }

    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[inline]
    pub const fn len(&self) -> usize {
        self.len as usize
    }
}

impl fmt::Debug for CallArgRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CallArgRange({}..{})", self.start, self.start + self.len as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expr_kind_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();

        set.insert(ExprKind::Int(42));
        set.insert(ExprKind::Int(42));
        set.insert(ExprKind::Int(43));
        set.insert(ExprKind::Bool(true));

        assert_eq!(set.len(), 3);
    }

    #[test]
    fn test_binary_op() {
        let op = BinaryOp::Add;
        assert_eq!(op, BinaryOp::Add);
        assert_ne!(op, BinaryOp::Sub);
    }

    #[test]
    fn test_expr_spanned() {
        let expr = Expr::new(ExprKind::Int(42), Span::new(0, 2));
        assert_eq!(expr.span().start, 0);
        assert_eq!(expr.span().end, 2);
    }

    #[test]
    fn test_module_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();

        let m1 = Module::new();
        let m2 = Module::new();

        set.insert(m1);
        set.insert(m2);

        assert_eq!(set.len(), 1);
    }

    #[test]
    fn test_function_exp_kind() {
        assert_eq!(FunctionExpKind::Map, FunctionExpKind::Map);
        assert_ne!(FunctionExpKind::Map, FunctionExpKind::Filter);
        assert_eq!(FunctionExpKind::Map.name(), "map");
    }
}
