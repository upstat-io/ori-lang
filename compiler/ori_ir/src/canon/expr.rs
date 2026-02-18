//! Canonical expression types — sugar-free, type-annotated expression nodes.
//!
//! [`CanExpr`] is the core expression enum consumed by both backends.
//! [`CanNode`] pairs a `CanExpr` with its source span and resolved type.
//! Supporting types ([`CanMapEntry`], [`CanField`], [`ConstValue`],
//! [`PatternProblem`]) are co-located here because they are primarily
//! referenced by `CanExpr` variants or canonicalization output.

use std::fmt;
use std::hash::{Hash, Hasher};

use crate::{BinaryOp, DurationUnit, FunctionExpKind, Name, SizeUnit, Span, TypeId, UnaryOp};

use super::ids::{CanBindingPatternId, CanFieldRange, CanId, CanMapEntryRange, CanRange};
use super::patterns::{CanNamedExprRange, CanParamRange};
use super::pools::{ConstantId, DecisionTreeId};

/// Canonical expression node — sugar-free, type-annotated, pattern-compiled.
///
/// This is NOT `ExprKind` with variants removed. It is a **distinct type** with
/// distinct semantics. Backends pattern-match on `CanExpr` exhaustively —
/// no `unreachable!()` arms, no sugar handling.
///
/// # Sugar Variants Absent
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
    /// Type reference for associated function calls: `Duration`, `Size`, user types.
    ///
    /// Emitted during canonicalization when an identifier resolves to a type name
    /// (via `TypedModule::type_def` or builtin type check). Eliminates the need for
    /// the evaluator to perform name resolution (phase bleeding) and acquire a
    /// `UserMethodRegistry` read lock on every identifier evaluation.
    ///
    /// The evaluator checks the environment first (for variable shadowing), then
    /// produces `Value::TypeRef`. Method dispatch on the resulting `TypeRef` routes
    /// to associated functions.
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
    /// Index access: `receiver[index]`
    Index { receiver: CanId, index: CanId },

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
    /// For loop/comprehension: `for[:label] binding in iter [if guard] do/yield body`.
    /// INVALID guard = no guard. `Name::EMPTY` label = no label.
    For {
        label: Name,
        binding: Name,
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
        mutable: bool,
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
            CanExpr::Loop { label, body } => write!(f, "Loop({label:?}, {body:?})"),
            CanExpr::Break { label, value } => write!(f, "Break({label:?}, {value:?})"),
            CanExpr::Continue { label, value } => write!(f, "Continue({label:?}, {value:?})"),
            CanExpr::Block { stmts, result } => write!(f, "Block({stmts:?}, {result:?})"),
            CanExpr::Let {
                pattern,
                init,
                mutable,
            } => {
                write!(f, "Let({pattern:?}, {init:?}, mut={mutable})")
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

/// A canonical expression node with source location and resolved type.
///
/// Unlike [`Expr`](crate::Expr) (which has no type information), each `CanNode`
/// carries the resolved type from the type checker. This means backends don't
/// need to look up types separately — they're right there on the node.
///
/// Following Roc's pattern where every `can::Expr` carries type variables.
///
/// # Note on `ty` field
///
/// The `ty` field uses [`TypeId`] which shares the same index layout as
/// `ori_types::Idx`. The lowering pass in `ori_canon` populates this field
/// from the type checker's `expr_types` map.
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

impl fmt::Debug for CanNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "CanNode({:?}, {:?}, {:?})",
            self.kind, self.span, self.ty
        )
    }
}

/// A map entry in canonical form: key-value pair.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct CanMapEntry {
    pub key: CanId,
    pub value: CanId,
}

/// A struct field initializer in canonical form: name-value pair.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct CanField {
    pub name: Name,
    pub value: CanId,
}

/// A compile-time constant value stored in a [`ConstantPool`](super::pools::ConstantPool).
///
/// These are produced by constant folding during canonicalization
/// (Section 04). Only values that can be fully determined at compile
/// time are represented here.
#[derive(Clone, Debug, PartialEq)]
pub enum ConstValue {
    Int(i64),
    Float(u64),
    Bool(bool),
    Str(Name),
    Char(char),
    Unit,
    Duration { value: u64, unit: DurationUnit },
    Size { value: u64, unit: SizeUnit },
}

impl Eq for ConstValue {}

impl Hash for ConstValue {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            ConstValue::Int(v) => v.hash(state),
            ConstValue::Float(v) => v.hash(state),
            ConstValue::Bool(v) => v.hash(state),
            ConstValue::Str(v) => v.hash(state),
            ConstValue::Char(v) => v.hash(state),
            ConstValue::Unit => {}
            ConstValue::Duration { value, unit } => {
                value.hash(state);
                unit.hash(state);
            }
            ConstValue::Size { value, unit } => {
                value.hash(state);
                unit.hash(state);
            }
        }
    }
}

/// A pattern-related problem detected during canonicalization.
///
/// These are produced by the exhaustiveness checker after decision tree
/// compilation. Both variants carry spans for rich diagnostic rendering.
///
/// # Salsa Compatibility
///
/// Derives `Clone, Eq, PartialEq, Hash, Debug` for Salsa query return types.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum PatternProblem {
    /// A match expression does not cover all possible values.
    NonExhaustive {
        /// Span of the `match` keyword / expression.
        match_span: Span,
        /// Human-readable descriptions of missing patterns (e.g. `"false"`, `"_"`).
        missing: Vec<String>,
    },
    /// A match arm can never be reached because earlier arms cover all its cases.
    RedundantArm {
        /// Span of the unreachable arm.
        arm_span: Span,
        /// Span of the enclosing match expression.
        match_span: Span,
        /// Zero-based index of the redundant arm.
        arm_index: usize,
    },
}
