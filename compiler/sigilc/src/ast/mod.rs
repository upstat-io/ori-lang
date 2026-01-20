// AST (Abstract Syntax Tree) definitions for Sigil

use std::collections::HashMap;

/// A complete source file / module
#[derive(Debug, Clone)]
pub struct Module {
    pub name: String,
    pub items: Vec<Item>,
}

/// Top-level items in a module
#[derive(Debug, Clone)]
pub enum Item {
    /// Config variable: $name = value
    Config(ConfigDef),

    /// Type definition: type Name = ... or type Name { ... }
    TypeDef(TypeDef),

    /// Function definition: @name (...) -> Type = ...
    Function(FunctionDef),

    /// Test definition: @name tests @target (...) -> void = ...
    Test(TestDef),

    /// Use statement: use path { items }
    Use(UseDef),
}

/// Test definition
#[derive(Debug, Clone)]
pub struct TestDef {
    pub name: String,
    pub target: String, // The function being tested
    pub body: Expr,
    pub span: Span,
}

/// Config definition: $name: Type = value
#[derive(Debug, Clone)]
pub struct ConfigDef {
    pub name: String,
    pub ty: Option<TypeExpr>,
    pub value: Expr,
    pub span: Span,
}

/// Type definition
#[derive(Debug, Clone)]
pub struct TypeDef {
    pub public: bool,
    pub name: String,
    pub params: Vec<String>, // Generic parameters
    pub kind: TypeDefKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum TypeDefKind {
    /// Newtype alias: type UserId = str
    Alias(TypeExpr),

    /// Struct: type User { id: UserId, name: str }
    Struct(Vec<Field>),

    /// Enum/Sum type: type Error = NotFound | Invalid { msg: str }
    Enum(Vec<Variant>),
}

#[derive(Debug, Clone)]
pub struct Field {
    pub name: String,
    pub ty: TypeExpr,
}

#[derive(Debug, Clone)]
pub struct Variant {
    pub name: String,
    pub fields: Vec<Field>, // Empty for unit variants
}

/// Function definition
#[derive(Debug, Clone)]
pub struct FunctionDef {
    pub public: bool,
    pub name: String,
    pub type_params: Vec<String>, // Generic type parameters
    pub params: Vec<Param>,
    pub return_type: TypeExpr,
    pub body: Expr,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub ty: TypeExpr,
}

/// Use/import definition
#[derive(Debug, Clone)]
pub struct UseDef {
    pub path: Vec<String>,
    pub items: Vec<UseItem>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct UseItem {
    pub name: String,
    pub alias: Option<String>,
}

/// Type expressions
#[derive(Debug, Clone)]
pub enum TypeExpr {
    /// Named type: int, str, User, etc.
    Named(String),

    /// Generic type application: Result T E, List T
    Generic(String, Vec<TypeExpr>),

    /// Optional type: ?T
    Optional(Box<TypeExpr>),

    /// List type: [T]
    List(Box<TypeExpr>),

    /// Map type: {K: V}
    Map(Box<TypeExpr>, Box<TypeExpr>),

    /// Tuple type: (T, U)
    Tuple(Vec<TypeExpr>),

    /// Function type: T -> U
    Function(Box<TypeExpr>, Box<TypeExpr>),

    /// Anonymous record/struct type: { field1: T1, field2: T2 }
    Record(Vec<(String, TypeExpr)>),
}

/// Expressions
#[derive(Debug, Clone)]
pub enum Expr {
    /// Literals
    Int(i64),
    Float(f64),
    String(String),
    Bool(bool),
    Nil,

    /// Identifier: x, user, etc.
    Ident(String),

    /// Config reference: $timeout
    Config(String),

    /// Length placeholder: # inside array index
    /// arr[# - 1] means arr[length - 1]
    LengthPlaceholder,

    /// List literal: [1, 2, 3]
    List(Vec<Expr>),

    /// Map literal: {"a": 1, "b": 2}
    MapLiteral(Vec<(Expr, Expr)>),

    /// Tuple: (a, b)
    Tuple(Vec<Expr>),

    /// Struct construction: User { id: x, name: y }
    Struct {
        name: String,
        fields: Vec<(String, Expr)>,
    },

    /// Field access: user.name
    Field(Box<Expr>, String),

    /// Index access: arr[0]
    Index(Box<Expr>, Box<Expr>),

    /// Function call: f(x, y)
    Call {
        func: Box<Expr>,
        args: Vec<Expr>,
    },

    /// Method call: x.method(y)
    MethodCall {
        receiver: Box<Expr>,
        method: String,
        args: Vec<Expr>,
    },

    /// Binary operation: a + b
    Binary {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },

    /// Unary operation: !x, -y
    Unary {
        op: UnaryOp,
        operand: Box<Expr>,
    },

    /// Lambda: x -> x + 1
    Lambda {
        params: Vec<String>,
        body: Box<Expr>,
    },

    /// Match expression
    Match(Box<MatchExpr>),

    /// If expression
    If {
        condition: Box<Expr>,
        then_branch: Box<Expr>,
        else_branch: Option<Box<Expr>>,
    },

    /// For loop
    For {
        binding: String,
        iterator: Box<Expr>,
        body: Box<Expr>,
    },

    /// Assignment: x := value
    Assign {
        target: String,
        value: Box<Expr>,
    },

    /// Block/sequence: run(expr1, expr2, ...)
    Block(Vec<Expr>),

    /// Range: 1..10
    Range {
        start: Box<Expr>,
        end: Box<Expr>,
    },

    /// Pattern-based function calls
    Pattern(PatternExpr),

    /// Result constructors
    Ok(Box<Expr>),
    Err(Box<Expr>),
    Some(Box<Expr>),
    None_,

    /// Null coalesce: x ?? default
    Coalesce {
        value: Box<Expr>,
        default: Box<Expr>,
    },

    /// Unwrap: x.unwrap()
    Unwrap(Box<Expr>),
}

#[derive(Debug, Clone)]
pub enum PatternExpr {
    /// fold(collection, init, op)
    Fold {
        collection: Box<Expr>,
        init: Box<Expr>,
        op: Box<Expr>,
    },

    /// map(collection, transform)
    Map {
        collection: Box<Expr>,
        transform: Box<Expr>,
    },

    /// filter(collection, predicate)
    Filter {
        collection: Box<Expr>,
        predicate: Box<Expr>,
    },

    /// collect(range, transform)
    Collect {
        range: Box<Expr>,
        transform: Box<Expr>,
    },

    /// recurse(condition, base_value, step) with optional memoization and parallelism
    /// When condition is true, returns base_value; otherwise evaluates step
    /// step can use `self(...)` for recursive calls
    Recurse {
        condition: Box<Expr>,    // Base case condition (e.g., n <= 1)
        base_value: Box<Expr>,   // Value to return when condition is true
        step: Box<Expr>,         // Recursive step using self()
        memo: bool,              // Enable memoization when true
        parallel_threshold: i64, // Parallelize when n > threshold (0 = no parallelism)
    },

    /// iterate(.over: x, .direction: dir, .into: init, .with: op)
    Iterate {
        over: Box<Expr>,
        direction: IterDirection,
        into: Box<Expr>,
        with: Box<Expr>,
    },

    /// transform(input, step1, step2, ...)
    Transform { input: Box<Expr>, steps: Vec<Expr> },

    /// count(collection, predicate)
    Count {
        collection: Box<Expr>,
        predicate: Box<Expr>,
    },

    /// parallel(.name: expr, .name2: expr2, ...) - concurrent execution
    /// Returns a struct with named fields containing results
    Parallel {
        branches: Vec<(String, Expr)>, // Named branches to execute concurrently
        timeout: Option<Box<Expr>>,    // Optional timeout duration
        on_error: OnError,             // Error handling strategy
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnError {
    FailFast,   // Cancel siblings on first error (default)
    CollectAll, // Wait for all, collect errors
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IterDirection {
    Forward,
    Backward,
}

#[derive(Debug, Clone)]
pub struct MatchExpr {
    pub scrutinee: Expr,
    pub arms: Vec<MatchArm>,
}

#[derive(Debug, Clone)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub body: Expr,
}

#[derive(Debug, Clone)]
pub enum Pattern {
    /// Wildcard: _
    Wildcard,

    /// Literal: 5, "hello", true
    Literal(Expr),

    /// Binding: x
    Binding(String),

    /// Variant: Ok { value }, Err { error }
    Variant {
        name: String,
        fields: Vec<(String, Pattern)>,
    },

    /// Condition: expr (for match guards)
    Condition(Expr),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    IntDiv,
    Mod,
    Eq,
    NotEq,
    Lt,
    LtEq,
    Gt,
    GtEq,
    And,
    Or,
    Pipe,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Neg,
    Not,
}

/// Source span
pub type Span = std::ops::Range<usize>;
