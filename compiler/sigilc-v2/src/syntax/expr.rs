//! Expression types for the flattened AST.

use crate::intern::Name;
use super::{
    Span, ExprId, ExprRange, StmtRange, ArmRange, ParamRange,
    MapEntryRange, FieldInitRange, PatternArgsId, TypeExprId,
    BinaryOp, UnaryOp,
    token::{DurationUnit, SizeUnit},
};

/// Expression node with span.
#[derive(Clone)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

impl Expr {
    pub fn new(kind: ExprKind, span: Span) -> Self {
        Expr { kind, span }
    }
}

impl std::fmt::Debug for Expr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?} @ {}", self.kind, self.span)
    }
}

/// Expression variants.
///
/// All children are indices into the arena, not boxes.
/// This provides significant memory savings and cache locality.
#[derive(Clone, Debug)]
pub enum ExprKind {
    // ===== Literals (no children) =====

    /// Integer literal: 42, 1_000
    Int(i64),

    /// Float literal: 3.14, 2.5e-8
    Float(f64),

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

    // ===== References =====

    /// Variable reference
    Ident(Name),

    /// Config reference: $name
    Config(Name),

    /// Self reference: self
    SelfRef,

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

    /// Function call: func(args...)
    Call {
        func: ExprId,
        args: ExprRange,
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
    /// Inside brackets, # refers to receiver.len()
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

    /// Pattern match: match(value, arms...)
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

    /// Let binding: let name = init
    Let {
        pattern: BindingPattern,
        ty: Option<TypeExprId>,
        init: ExprId,
        mutable: bool,
    },

    /// Lambda: params -> body
    Lambda {
        params: ParamRange,
        ret_ty: Option<TypeExprId>,
        body: ExprId,
    },

    // ===== Patterns (first-class) =====

    /// Pattern invocation: map(.over: x, .transform: f)
    Pattern {
        kind: PatternKind,
        args: PatternArgsId,
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

    /// Unit: ()
    Unit,

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

    /// Range: start..end or start..=end
    Range {
        start: Option<ExprId>,
        end: Option<ExprId>,
        inclusive: bool,
    },

    /// Hash in index context (refers to length)
    HashLength,

    /// Function reference: @name
    FunctionRef(Name),

    /// Assignment: target = value
    Assign {
        target: ExprId,
        value: ExprId,
    },

    /// Placeholder for error recovery
    Error,
}

/// Built-in pattern kinds.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum PatternKind {
    /// run(stmt1, stmt2, ..., result)
    Run,
    /// try(expr?, fallback)
    Try,
    /// match(value, pat -> expr, ...)
    Match,
    /// map(.over: items, .transform: fn)
    Map,
    /// filter(.over: items, .predicate: fn)
    Filter,
    /// fold(.over: items, .init: val, .op: fn)
    Fold,
    /// find(.over: items, .where: fn)
    Find,
    /// collect(.range: 0..10, .transform: fn)
    Collect,
    /// recurse(.cond: base_case, .base: val, .step: self(...))
    Recurse,
    /// parallel(.task1: expr1, .task2: expr2)
    Parallel,
    /// timeout(.op: expr, .after: duration)
    Timeout,
    /// retry(.op: expr, .attempts: n, .backoff: strategy)
    Retry,
    /// cache(.key: k, .compute: fn)
    Cache,
    /// validate(.value: v, .rules: [...])
    Validate,
}

impl PatternKind {
    /// Get the pattern name as a string.
    pub fn name(self) -> &'static str {
        match self {
            PatternKind::Run => "run",
            PatternKind::Try => "try",
            PatternKind::Match => "match",
            PatternKind::Map => "map",
            PatternKind::Filter => "filter",
            PatternKind::Fold => "fold",
            PatternKind::Find => "find",
            PatternKind::Collect => "collect",
            PatternKind::Recurse => "recurse",
            PatternKind::Parallel => "parallel",
            PatternKind::Timeout => "timeout",
            PatternKind::Retry => "retry",
            PatternKind::Cache => "cache",
            PatternKind::Validate => "validate",
        }
    }
}

/// Binding pattern for let expressions.
#[derive(Clone, Debug)]
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

/// Statement in a block.
#[derive(Clone, Debug)]
pub struct Stmt {
    pub kind: StmtKind,
    pub span: Span,
}

/// Statement kinds.
#[derive(Clone, Debug)]
pub enum StmtKind {
    /// Expression statement
    Expr(ExprId),
    /// Let binding
    Let {
        pattern: BindingPattern,
        ty: Option<TypeExprId>,
        init: ExprId,
        mutable: bool,
    },
}

/// Match arm.
#[derive(Clone, Debug)]
pub struct MatchArm {
    /// Pattern to match
    pub pattern: MatchPattern,
    /// Optional guard: x.match(guard_expr)
    pub guard: Option<ExprId>,
    /// Body expression
    pub body: ExprId,
    /// Span of the arm
    pub span: Span,
}

/// Match pattern.
#[derive(Clone, Debug)]
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

/// Map entry in a map literal.
#[derive(Clone, Debug)]
pub struct MapEntry {
    pub key: ExprId,
    pub value: ExprId,
    pub span: Span,
}

/// Field initializer in a struct literal.
#[derive(Clone, Debug)]
pub struct FieldInit {
    pub name: Name,
    /// None for shorthand: Point { x, y }
    pub value: Option<ExprId>,
    pub span: Span,
}

/// Function parameter.
#[derive(Clone, Debug)]
pub struct Param {
    pub name: Name,
    pub ty: Option<TypeExprId>,
    pub default: Option<ExprId>,
    pub span: Span,
}

/// Named pattern argument: .name: value
#[derive(Clone, Debug)]
pub struct PatternArg {
    pub name: Name,
    pub value: ExprId,
    pub span: Span,
}

/// Arguments to a pattern invocation.
#[derive(Clone, Debug)]
pub struct PatternArgs {
    /// Named arguments: .over: items, .transform: fn
    pub named: Vec<PatternArg>,
    /// Positional arguments (for run pattern)
    pub positional: ExprRange,
    /// Span of the arguments
    pub span: Span,
}

/// Type expression (unparsed, will be resolved later).
#[derive(Clone, Debug)]
pub struct TypeExpr {
    pub kind: TypeExprKind,
    pub span: Span,
}

/// Type expression kinds.
#[derive(Clone, Debug)]
pub enum TypeExprKind {
    /// Named type: int, MyStruct, Option<T>
    Named {
        name: Name,
        type_args: Vec<TypeExpr>,
    },
    /// Function type: (int, int) -> int
    Function {
        params: Vec<TypeExpr>,
        ret: Box<TypeExpr>,
    },
    /// Tuple type: (int, str)
    Tuple(Vec<TypeExpr>),
    /// List type: [int]
    List(Box<TypeExpr>),
    /// Map type: {str: int}
    Map {
        key: Box<TypeExpr>,
        value: Box<TypeExpr>,
    },
    /// Reference type: &T, &mut T
    Ref {
        inner: Box<TypeExpr>,
        mutable: bool,
    },
    /// Inferred type: _
    Infer,
    /// Error placeholder
    Error,
}
