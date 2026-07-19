//! Canonical expression types — sugar-free, type-annotated expression nodes.
//!
//! [`CanExpr`] is the core expression enum consumed by both backends.
//! [`CanNode`] pairs a `CanExpr` with its source span and resolved type.
//! Supporting value and diagnostic types include [`CanMapEntry`](super::CanMapEntry),
//! [`CanField`](super::CanField), [`ConstValue`](super::ConstValue),
//! and [`PatternProblem`](super::PatternProblem).

mod debug;

use std::hash::{Hash, Hasher};

use crate::{
    BinaryOp, DurationUnit, FunctionExpKind, Mutability, Name, SizeUnit, Span, TypeId, UnaryOp,
};

use super::ids::{CanBindingPatternId, CanFieldRange, CanId, CanMapEntryRange, CanRange};
use super::patterns::{CanNamedExprRange, CanParamRange};
use super::pools::{ConstantId, DecisionTreeId};
use super::result::MethodProducerId;

/// Type-checker-selected dispatch route for one index expression.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum IndexDispatch {
    /// Primitive List, Map, str, or Tuple indexing.
    Builtin,
    /// Polymorphic indexing whose concrete route remains specialization-dependent.
    Deferred,
    /// Exact user-defined `Index` producer selected during type checking.
    Selected(MethodProducerId),
    /// Invalid indexing retained only while diagnostics prevent execution.
    Error,
}

/// Canonical expression node — sugar-free, type-annotated, pattern-compiled.
///
/// This is NOT `ExprKind` with variants removed. It is a **distinct type** with
/// distinct semantics. Backends pattern-match on `CanExpr` exhaustively with
/// no sugar handling.
///
/// # Examples
///
/// These `ExprKind` variants have no `CanExpr` equivalent — they are desugared
/// during lowering (`ori_canon::desugar`):
///
/// | `ExprKind` variant | Desugared to |
/// |------------------|--------------|
/// | `CallNamed` | `Call` (args reordered to positional) |
/// | `MethodCallNamed` | `MethodCall` (args reordered) |
/// | `TemplateFull` | `Str` |
/// | `TemplateLiteral` | `Str` + `.to_str()` / `FormatWith` + `.concat()` chain |
/// | `ListWithSpread` | `List` + `.concat()` chains |
/// | `MapWithSpread` | `Map` + `.merge()` chains |
/// | `StructWithSpread` | `Struct` with all fields resolved via `Field` access |
///
/// # Size
///
/// Target: ≤ 24 bytes (same as `ExprKind`). Verified by `static_assert_size!`.
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub enum CanExpr {
    // Literals
    /// Integer literal: `42`, `1_000`
    Int(i64),
    /// Float literal as bits: `3.14`, `2.5e-8`
    Float(u64),
    /// Boolean literal: `true`, `false`
    Bool(bool),
    /// String literal (interned): `"hello"`
    Str(Name),
    /// Character literal: `'a'`, `'\n'`
    Char(char),
    /// Duration literal: `100ms`, `5s`, `2h`
    Duration { value: u64, unit: DurationUnit },
    /// Size literal: `4kb`, `10mb`
    Size { value: u64, unit: SizeUnit },
    /// Unit literal: `()`
    Unit,

    // Compile-Time Constant
    /// A value folded at compile time. Index into [`ConstantPool`](super::pools::ConstantPool).
    Constant(ConstantId),

    // References
    /// Variable reference: `x`
    Ident(Name),
    /// Constant reference: `$name`
    Const(Name),
    /// Self reference: `self`
    SelfRef,
    /// Function reference: `@name`
    FunctionRef(Name),
    /// Type reference for associated calls: `Duration`, `Size`, or a user type.
    /// Canonicalization emits it when an identifier resolves to a type name, so
    /// evaluation needs no cross-layer lookup. The environment retains priority
    /// for variable shadowing; otherwise evaluation produces `Value::TypeRef`.
    TypeRef(Name),
    /// Hash in index context (refers to length): `#`
    HashLength,

    // Operators
    /// Binary operation: `left op right`
    Binary {
        op: BinaryOp,
        left: CanId,
        right: CanId,
    },
    /// Unary operation: `op operand`
    Unary { op: UnaryOp, operand: CanId },
    /// Type cast: `expr as Type` (infallible) or `expr as? Type` (fallible).
    ///
    /// Stores the target type name (e.g. "int", "float", "str") instead of
    /// `ParsedTypeId`. The evaluator dispatches on the name; the LLVM backend
    /// uses the resolved `TypeId` from `CanNode.ty`.
    Cast {
        expr: CanId,
        target: Name,
        fallible: bool,
    },

    // Calls (always positional — named args already reordered)
    /// Function call with positional arguments.
    Call { func: CanId, args: CanRange },
    /// Method call with positional arguments.
    MethodCall {
        receiver: CanId,
        method: Name,
        args: CanRange,
    },

    // Access
    /// Field access: `receiver.field`
    Field { receiver: CanId, field: Name },
    /// Index access: `receiver[index]`.
    ///
    /// `dispatch` freezes the type checker's builtin, deferred, selected, or
    /// invalid route so semantic consumers never reconstruct it from type shape.
    Index {
        receiver: CanId,
        index: CanId,
        dispatch: IndexDispatch,
    },

    // Control Flow
    /// Conditional: `if cond then else`. INVALID `else_branch` = unit block.
    If {
        cond: CanId,
        then_branch: CanId,
        else_branch: CanId,
    },
    /// Pattern match with pre-compiled decision tree.
    Match {
        scrutinee: CanId,
        decision_tree: DecisionTreeId,
        arms: CanRange,
    },
    /// For loop/comprehension: `for[:label] pattern in iter [if guard] do/yield body`.
    /// INVALID guard = no guard. `Name::EMPTY` label = no label.
    For {
        label: Name,
        pattern: CanBindingPatternId,
        iter: CanId,
        guard: CanId,
        body: CanId,
        is_yield: bool,
    },
    /// Infinite loop: `loop[:label] { body }`. `Name::EMPTY` label = no label.
    Loop { label: Name, body: CanId },
    /// Break from loop (INVALID = no value). `Name::EMPTY` label = no label.
    Break { label: Name, value: CanId },
    /// Continue loop (INVALID = no value). `Name::EMPTY` label = no label.
    Continue { label: Name, value: CanId },

    // Bindings
    /// Block: `{ stmts; result }`. INVALID result = unit block.
    Block { stmts: CanRange, result: CanId },
    /// Let binding: `let pattern = init`.
    ///
    /// Type info is on `CanNode.ty`; no `ParsedTypeId` needed.
    Let {
        pattern: CanBindingPatternId,
        init: CanId,
        mutable: Mutability,
    },
    /// Assignment: `target = value`
    Assign { target: CanId, value: CanId },

    // Functions
    /// Lambda: `params -> body`.
    ///
    /// Return type is on `CanNode.ty`; no `ParsedTypeId` needed.
    Lambda { params: CanParamRange, body: CanId },

    // Collections (no spread variants — already expanded)
    /// List literal: `[a, b, c]`
    List(CanRange),
    /// Tuple literal: `(a, b, c)`
    Tuple(CanRange),
    /// Map literal: `{k: v, ...}`
    Map(CanMapEntryRange),
    /// Struct literal: `Point { x: 0, y: 0 }`
    Struct { name: Name, fields: CanFieldRange },
    /// Range: `start..end` or `start..=end` or `start..end by step`.
    /// INVALID = unbounded.
    Range {
        start: CanId,
        end: CanId,
        step: CanId,
        inclusive: bool,
    },

    // Algebraic
    /// Ok variant: `Ok(value)`. INVALID = `Ok(())`.
    Ok(CanId),
    /// Err variant: `Err(value)`. INVALID = `Err(())`.
    Err(CanId),
    /// Some variant: `Some(value)`.
    Some(CanId),
    /// None variant.
    None,

    // Error Handling
    /// Error propagation: `expr?`
    Try(CanId),
    /// Await async operation: `await expr`
    Await(CanId),

    // Safety
    /// Unsafe block: `unsafe { expr }` — discharges `Unsafe` capability.
    /// Transparent at runtime — evaluates to inner expression.
    Unsafe(CanId),

    // Capabilities
    /// Capability injection: `with Http = provider in body`
    WithCapability {
        capability: Name,
        provider: CanId,
        body: CanId,
    },

    // Special Forms
    /// Named function expression: `print`, `panic`, `todo`, etc.
    ///
    /// Inlined from `FunctionExpId` — the kind and canonical props are
    /// stored directly, eliminating the `ExprArena` side-table reference.
    FunctionExp {
        kind: FunctionExpKind,
        props: CanNamedExprRange,
    },

    // Formatting
    /// Format a value with a format specification: `{expr:spec}` in template strings.
    ///
    /// Emitted by canonicalization when a template interpolation has a format spec.
    /// The spec is the raw interned string (e.g., `"08x"`, `">10.2f"`), parsed
    /// at evaluation/codegen time. Produces `str`.
    FormatWith { expr: CanId, spec: Name },

    // Error Recovery
    /// Parse/type error placeholder. Propagates silently through lowering.
    Error,
}

// CanExpr: 24 bytes on 64-bit (same as ExprKind).
// Largest variants are Duration/Size (u64 forces 8-byte alignment).
static_assert_size!(CanExpr, 24);

/// A canonical expression node with source location and resolved type.
///
/// Unlike [`Expr`](crate::Expr), each node carries its resolved type directly.
/// [`TypeId`] shares the index layout of `ori_types::Idx`; canonical lowering
/// populates it from the type checker's expression-type map.
#[derive(Copy, Clone, Eq, PartialEq)]
pub struct CanNode {
    /// The expression variant.
    pub kind: CanExpr,
    /// Source location for error reporting.
    pub span: Span,
    /// Resolved type from the type checker.
    pub ty: TypeId,
}

impl CanNode {
    /// Create a new canonical node.
    #[inline]
    #[must_use]
    pub const fn new(kind: CanExpr, span: Span, ty: TypeId) -> Self {
        Self { kind, span, ty }
    }
}

impl Hash for CanNode {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.kind.hash(state);
        self.span.hash(state);
        self.ty.hash(state);
    }
}
