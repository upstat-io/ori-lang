//! Canonical IR — sugar-free, type-annotated intermediate representation.
//!
//! The canonical IR (`CanExpr`) sits between the type checker and both backends.
//! It is a **distinct type** from `ExprKind` — sugar variants cannot be represented,
//! enforced at the type level. Both `ori_eval` (interpreter) and `ori_arc` (ARC/LLVM
//! codegen) consume `CanExpr` exclusively after migration.
//!
//! # Architecture
//!
//! ```text
//! Source → Lex → Parse → Type Check → Canonicalize ─┬─→ ori_eval  (interprets CanExpr)
//!                                       (ori_canon)   └─→ ori_arc   (lowers CanExpr → ARC IR)
//! ```
//!
//! # Prior Art
//!
//! - **Roc**: `ast::Expr` → `can::Expr` → `mono::Expr` — both dev and LLVM backends
//!   consume the same mono IR. Zero parse-AST dispatch in codegen.
//! - **Elm**: `Source` → `Canonical` → `Optimized` → JS — decision trees baked into
//!   the `Optimized` form, backends never see raw patterns.
//!
//! # What's Different from `ExprKind`
//!
//! - No `CallNamed` / `MethodCallNamed` — desugared to positional `Call` / `MethodCall`
//! - No `TemplateLiteral` / `TemplateFull` — desugared to string concatenation chains
//! - No `ListWithSpread` / `MapWithSpread` / `StructWithSpread` — desugared to method calls
//! - Added `Constant(ConstantId)` — compile-time-folded values
//! - Added `DecisionTreeId` on `Match` — patterns pre-compiled to decision trees
//! - Uses `CanId` / `CanRange` (not `ExprId` / `ExprRange`) — distinct index space

pub mod hash;
pub mod tree;

use std::fmt;
use std::hash::{Hash, Hasher};

use crate::arena::{to_u16, to_u32};
use crate::{BinaryOp, DurationUnit, FunctionExpKind, Name, SizeUnit, Span, TypeId, UnaryOp};

pub use tree::{
    DecisionTree, FlatPattern, PathInstruction, PatternMatrix, PatternRow, ScrutineePath, TestKind,
    TestValue,
};

/// Index into a [`CanArena`]. Distinct from [`ExprId`](crate::ExprId) —
/// these reference canonical expressions in a separate index space.
///
/// # Salsa Compatibility
/// Implements `Copy`, `Clone`, `Eq`, `PartialEq`, `Hash`, `Debug`.
#[derive(Copy, Clone, Eq, PartialEq)]
#[repr(transparent)]
pub struct CanId(u32);

impl CanId {
    /// Sentinel value indicating "no expression" (analogous to `ExprId::INVALID`).
    /// Used for optional child expressions (e.g., no else branch, no guard).
    pub const INVALID: CanId = CanId(u32::MAX);

    /// Create a new `CanId` from a raw index.
    #[inline]
    pub const fn new(index: u32) -> Self {
        Self(index)
    }

    /// Get the raw index into the arena.
    #[inline]
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    /// Get the raw `u32` value.
    #[inline]
    pub const fn raw(self) -> u32 {
        self.0
    }

    /// Bridge: create a `CanId` from an `ExprId`'s raw index.
    ///
    /// Used by backends that haven't migrated to `CanonResult` yet (`ori_arc`).
    /// The resulting `CanId` carries the same raw index as the `ExprId`, which
    /// the backend interprets in its own context.
    ///
    /// This will be removed once the ARC backend migrates to `CanonResult` (07.2).
    #[inline]
    pub const fn from_expr_id(id: crate::ExprId) -> Self {
        Self(id.raw())
    }

    /// Bridge: convert back to an `ExprId` raw index.
    ///
    /// Used by backends that haven't migrated to `CanonResult` yet.
    /// Will be removed once all backends use `CanonResult` (07.2).
    #[inline]
    pub const fn to_expr_id(self) -> crate::ExprId {
        crate::ExprId::new(self.0)
    }

    /// Returns `true` if this is a valid (non-sentinel) ID.
    #[inline]
    pub const fn is_valid(self) -> bool {
        self.0 != u32::MAX
    }
}

impl Hash for CanId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl fmt::Debug for CanId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if *self == Self::INVALID {
            write!(f, "CanId::INVALID")
        } else {
            write!(f, "CanId({})", self.0)
        }
    }
}

impl Default for CanId {
    fn default() -> Self {
        Self::INVALID
    }
}

/// A contiguous range of canonical expression IDs in a [`CanArena`].
///
/// Used for expression lists: function arguments, list elements, block
/// statements, tuple elements, etc. Indexes into the arena's `expr_lists`
/// storage.
///
/// Layout matches [`ExprRange`](crate::ExprRange): `start: u32, len: u16` = 8 bytes.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Default)]
#[repr(C)]
pub struct CanRange {
    pub start: u32,
    pub len: u16,
}

impl CanRange {
    /// Empty range constant.
    pub const EMPTY: Self = Self { start: 0, len: 0 };

    /// Create a new range.
    #[inline]
    pub const fn new(start: u32, len: u16) -> Self {
        Self { start, len }
    }

    /// Returns `true` if the range contains no elements.
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Number of elements in the range.
    #[inline]
    pub const fn len(&self) -> usize {
        self.len as usize
    }
}

impl fmt::Debug for CanRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "CanRange({}..{})",
            self.start,
            self.start + u32::from(self.len)
        )
    }
}

/// Range of map entries in a [`CanArena`]. Each entry is a key-value pair.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Default)]
#[repr(C)]
pub struct CanMapEntryRange {
    pub start: u32,
    pub len: u16,
}

impl CanMapEntryRange {
    pub const EMPTY: Self = Self { start: 0, len: 0 };

    #[inline]
    pub const fn new(start: u32, len: u16) -> Self {
        Self { start, len }
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

impl fmt::Debug for CanMapEntryRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "CanMapEntryRange({}..{})",
            self.start,
            self.start + u32::from(self.len)
        )
    }
}

/// Range of struct field initializers in a [`CanArena`]. Each field is name + value.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Default)]
#[repr(C)]
pub struct CanFieldRange {
    pub start: u32,
    pub len: u16,
}

impl CanFieldRange {
    pub const EMPTY: Self = Self { start: 0, len: 0 };

    #[inline]
    pub const fn new(start: u32, len: u16) -> Self {
        Self { start, len }
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

impl fmt::Debug for CanFieldRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "CanFieldRange({}..{})",
            self.start,
            self.start + u32::from(self.len)
        )
    }
}

/// Index into a [`CanArena`]'s binding pattern storage.
///
/// Replaces `BindingPatternId` (which indexes `ExprArena.binding_patterns`)
/// with a canonical equivalent that keeps the IR self-contained.
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
#[repr(transparent)]
pub struct CanBindingPatternId(u32);

impl CanBindingPatternId {
    #[inline]
    pub const fn new(index: u32) -> Self {
        Self(index)
    }

    #[inline]
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    #[inline]
    pub const fn raw(self) -> u32 {
        self.0
    }
}

impl fmt::Debug for CanBindingPatternId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CanBindingPatternId({})", self.0)
    }
}

/// Range of binding pattern IDs in `CanArena.binding_pattern_lists`.
///
/// Used for `Tuple` and `List` sub-patterns which contain multiple children.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Default)]
#[repr(C)]
pub struct CanBindingPatternRange {
    pub start: u32,
    pub len: u16,
}

impl CanBindingPatternRange {
    pub const EMPTY: Self = Self { start: 0, len: 0 };

    #[inline]
    pub const fn new(start: u32, len: u16) -> Self {
        Self { start, len }
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

impl fmt::Debug for CanBindingPatternRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "CanBindingPatternRange({}..{})",
            self.start,
            self.start + u32::from(self.len)
        )
    }
}

/// Range of field bindings in `CanArena.field_bindings`.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Default)]
#[repr(C)]
pub struct CanFieldBindingRange {
    pub start: u32,
    pub len: u16,
}

impl CanFieldBindingRange {
    pub const EMPTY: Self = Self { start: 0, len: 0 };

    #[inline]
    pub const fn new(start: u32, len: u16) -> Self {
        Self { start, len }
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

impl fmt::Debug for CanFieldBindingRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "CanFieldBindingRange({}..{})",
            self.start,
            self.start + u32::from(self.len)
        )
    }
}

/// Canonical binding pattern — self-contained, no `ExprArena` references.
///
/// Mirrors `BindingPattern` from `ori_ir::ast` but stores sub-patterns
/// in `CanArena` via `CanBindingPatternId` instead of `Vec<BindingPattern>`.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub enum CanBindingPattern {
    /// Simple name binding: `let x = ...`
    Name(Name),
    /// Tuple destructuring: `let (a, b) = ...`
    Tuple(CanBindingPatternRange),
    /// Struct destructuring: `let { x, y } = ...`
    Struct { fields: CanFieldBindingRange },
    /// List destructuring: `let [head, ..tail] = ...`
    List {
        elements: CanBindingPatternRange,
        rest: Option<Name>,
    },
    /// Wildcard: `let _ = ...`
    Wildcard,
}

/// A struct field binding in canonical form: field name + sub-pattern.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct CanFieldBinding {
    pub name: Name,
    pub pattern: CanBindingPatternId,
}

/// Canonical function parameter — only what evaluation/codegen needs.
///
/// Replaces `Param` (which contains `MatchPattern`, `ParsedType`, `ExprId`)
/// with a minimal representation: just the name and an optional default.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct CanParam {
    /// Parameter name.
    pub name: Name,
    /// Default value expression. `CanId::INVALID` if no default.
    pub default: CanId,
}

/// Range of canonical parameters in `CanArena.params`.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Default)]
#[repr(C)]
pub struct CanParamRange {
    pub start: u32,
    pub len: u16,
}

impl CanParamRange {
    pub const EMPTY: Self = Self { start: 0, len: 0 };

    #[inline]
    pub const fn new(start: u32, len: u16) -> Self {
        Self { start, len }
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

impl fmt::Debug for CanParamRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "CanParamRange({}..{})",
            self.start,
            self.start + u32::from(self.len)
        )
    }
}

/// A named expression in canonical form (for `FunctionExp` props).
///
/// Replaces `NamedExpr` which contains `ExprId` (an `ExprArena` reference)
/// with a canonical version that uses `CanId`.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct CanNamedExpr {
    pub name: Name,
    pub value: CanId,
}

/// Range of named expressions in `CanArena.named_exprs`.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Default)]
#[repr(C)]
pub struct CanNamedExprRange {
    pub start: u32,
    pub len: u16,
}

impl CanNamedExprRange {
    pub const EMPTY: Self = Self { start: 0, len: 0 };

    #[inline]
    pub const fn new(start: u32, len: u16) -> Self {
        Self { start, len }
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

impl fmt::Debug for CanNamedExprRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "CanNamedExprRange({}..{})",
            self.start,
            self.start + u32::from(self.len)
        )
    }
}

/// Index into a [`ConstantPool`]. References a compile-time-folded value.
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
#[repr(transparent)]
pub struct ConstantId(u32);

impl ConstantId {
    #[inline]
    pub const fn new(index: u32) -> Self {
        Self(index)
    }

    #[inline]
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    #[inline]
    pub const fn raw(self) -> u32 {
        self.0
    }
}

impl fmt::Debug for ConstantId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ConstantId({})", self.0)
    }
}

/// Index into a [`DecisionTreePool`]. References a pre-compiled decision tree.
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
#[repr(transparent)]
pub struct DecisionTreeId(u32);

impl DecisionTreeId {
    #[inline]
    pub const fn new(index: u32) -> Self {
        Self(index)
    }

    #[inline]
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    #[inline]
    pub const fn raw(self) -> u32 {
        self.0
    }
}

impl fmt::Debug for DecisionTreeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DecisionTreeId({})", self.0)
    }
}

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
/// | `TemplateLiteral` | `Str` + `.to_str()` + `.concat()` chain |
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
    /// A value folded at compile time. Index into [`ConstantPool`].
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

    // Error Recovery
    /// Parse/type error placeholder. Propagates silently through lowering.
    Error,
}

// CanExpr: 24 bytes on 64-bit (same as ExprKind).
// Largest variants are Duration/Size (u64 forces 8-byte alignment).
static_assert_size!(CanExpr, 24);

impl fmt::Debug for CanExpr {
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

/// A compile-time constant value stored in a [`ConstantPool`].
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

/// Pool of compile-time constant values, indexed by [`ConstantId`].
///
/// Constants are interned: duplicate values share the same ID.
/// Pre-interns common sentinels (unit, true, false, 0, 1, empty string)
/// for O(1) access.
#[derive(Clone, Debug)]
pub struct ConstantPool {
    values: Vec<ConstValue>,
    /// Content-hash dedup: maps value hash to index for O(1) lookup.
    dedup: rustc_hash::FxHashMap<ConstValue, ConstantId>,
}

impl ConstantPool {
    // Pre-interned sentinel IDs.
    pub const UNIT: ConstantId = ConstantId(0);
    pub const TRUE: ConstantId = ConstantId(1);
    pub const FALSE: ConstantId = ConstantId(2);
    pub const ZERO: ConstantId = ConstantId(3);
    pub const ONE: ConstantId = ConstantId(4);
    pub const EMPTY_STR: ConstantId = ConstantId(5);

    /// Create a new constant pool with pre-interned sentinels.
    pub fn new() -> Self {
        let sentinels = vec![
            ConstValue::Unit,
            ConstValue::Bool(true),
            ConstValue::Bool(false),
            ConstValue::Int(0),
            ConstValue::Int(1),
            ConstValue::Str(Name::EMPTY),
        ];

        let mut dedup = rustc_hash::FxHashMap::default();
        for (i, v) in sentinels.iter().enumerate() {
            dedup.insert(v.clone(), ConstantId::new(to_u32(i, "constant sentinels")));
        }

        Self {
            values: sentinels,
            dedup,
        }
    }

    /// Intern a constant value. Returns the existing ID if already interned.
    pub fn intern(&mut self, value: ConstValue) -> ConstantId {
        if let Some(&id) = self.dedup.get(&value) {
            return id;
        }
        let id = ConstantId::new(to_u32(self.values.len(), "constants"));
        self.dedup.insert(value.clone(), id);
        self.values.push(value);
        id
    }

    /// Get a constant value by ID.
    pub fn get(&self, id: ConstantId) -> &ConstValue {
        &self.values[id.index()]
    }

    /// Number of interned constants.
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Returns `true` if only sentinels are present.
    pub fn is_empty(&self) -> bool {
        self.values.len() <= 6 // sentinels only
    }
}

impl Default for ConstantPool {
    fn default() -> Self {
        Self::new()
    }
}

impl PartialEq for ConstantPool {
    fn eq(&self, other: &Self) -> bool {
        self.values == other.values
    }
}

impl Eq for ConstantPool {}

/// Pool of compiled decision trees, indexed by [`DecisionTreeId`].
///
/// Decision trees are produced during pattern compilation (Section 03)
/// and consumed by both `ori_eval` and `ori_arc`. Trees are wrapped in
/// `Arc` so consumers can cheaply clone a reference (O(1)) instead of
/// deep-cloning the recursive tree structure.
/// Shared decision tree — `Arc<DecisionTree>` for O(1) cloning.
///
/// Decision trees are immutable after construction and may be cloned
/// by both `ori_eval` (to release a borrow on `self`) and `ori_arc`
/// (same pattern). Arc sharing avoids deep-copying the recursive structure.
#[expect(
    clippy::disallowed_types,
    reason = "Arc enables O(1) clone for immutable decision trees shared across eval/codegen"
)]
pub type SharedDecisionTree = std::sync::Arc<DecisionTree>;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DecisionTreePool {
    trees: Vec<SharedDecisionTree>,
}

impl DecisionTreePool {
    /// Create an empty pool.
    pub fn new() -> Self {
        Self { trees: Vec::new() }
    }

    /// Store a decision tree and return its ID.
    pub fn push(&mut self, tree: DecisionTree) -> DecisionTreeId {
        let id = DecisionTreeId::new(to_u32(self.trees.len(), "decision trees"));
        self.trees.push(SharedDecisionTree::new(tree));
        id
    }

    /// Get a decision tree by ID.
    pub fn get(&self, id: DecisionTreeId) -> &DecisionTree {
        &self.trees[id.index()]
    }

    /// Get a shared reference to a decision tree for O(1) cloning.
    ///
    /// Use this instead of `get().clone()` when you need to own a copy
    /// of the tree (e.g., to release a borrow on `self`). The Arc clone
    /// is O(1) vs O(n) for deep-cloning the recursive tree structure.
    pub fn get_shared(&self, id: DecisionTreeId) -> SharedDecisionTree {
        SharedDecisionTree::clone(&self.trees[id.index()])
    }

    /// Number of stored trees.
    pub fn len(&self) -> usize {
        self.trees.len()
    }

    /// Returns `true` if no trees are stored.
    pub fn is_empty(&self) -> bool {
        self.trees.is_empty()
    }
}

/// Arena for canonical expressions.
///
/// Uses struct-of-arrays layout for cache locality, following the same
/// pattern as [`ExprArena`](crate::ExprArena).
///
/// # Index Spaces
///
/// - `kinds`/`spans`/`types`: parallel arrays indexed by [`CanId`]
/// - `expr_lists`: flat `Vec<CanId>` indexed by [`CanRange`]
/// - `map_entries`: indexed by [`CanMapEntryRange`]
/// - `fields`: indexed by [`CanFieldRange`]
#[derive(Clone, Debug)]
pub struct CanArena {
    /// Canonical expression kinds (parallel with spans and types).
    kinds: Vec<CanExpr>,
    /// Source spans for error reporting (parallel with kinds).
    spans: Vec<Span>,
    /// Resolved types from the type checker (parallel with kinds).
    types: Vec<TypeId>,
    /// Flattened expression ID lists for ranges (args, elements, stmts).
    expr_lists: Vec<CanId>,
    /// Map entries (key-value pairs).
    map_entries: Vec<CanMapEntry>,
    /// Struct field initializers (name-value pairs).
    fields: Vec<CanField>,
    /// Canonical binding patterns (indexed by `CanBindingPatternId`).
    binding_patterns: Vec<CanBindingPattern>,
    /// Flattened binding pattern ID lists (for Tuple/List sub-patterns).
    binding_pattern_lists: Vec<CanBindingPatternId>,
    /// Struct field bindings (indexed by `CanFieldBindingRange`).
    field_bindings: Vec<CanFieldBinding>,
    /// Canonical function parameters (indexed by `CanParamRange`).
    params: Vec<CanParam>,
    /// Named expressions for `FunctionExp` props (indexed by `CanNamedExprRange`).
    named_exprs: Vec<CanNamedExpr>,
}

impl CanArena {
    /// Create an empty arena.
    pub fn new() -> Self {
        Self {
            kinds: Vec::new(),
            spans: Vec::new(),
            types: Vec::new(),
            expr_lists: Vec::new(),
            map_entries: Vec::new(),
            fields: Vec::new(),
            binding_patterns: Vec::new(),
            binding_pattern_lists: Vec::new(),
            field_bindings: Vec::new(),
            params: Vec::new(),
            named_exprs: Vec::new(),
        }
    }

    /// Create an arena pre-allocated based on source length.
    ///
    /// Uses the same heuristic as `ExprArena`: ~1 expression per 20 bytes of source.
    pub fn with_capacity(source_len: usize) -> Self {
        let estimated = source_len / 20;
        Self {
            kinds: Vec::with_capacity(estimated),
            spans: Vec::with_capacity(estimated),
            types: Vec::with_capacity(estimated),
            expr_lists: Vec::with_capacity(estimated),
            map_entries: Vec::new(),
            fields: Vec::new(),
            binding_patterns: Vec::new(),
            binding_pattern_lists: Vec::new(),
            field_bindings: Vec::new(),
            params: Vec::new(),
            named_exprs: Vec::new(),
        }
    }

    /// Allocate a canonical node, returning its ID.
    pub fn push(&mut self, node: CanNode) -> CanId {
        let id = CanId::new(to_u32(self.kinds.len(), "canonical expressions"));
        self.kinds.push(node.kind);
        self.spans.push(node.span);
        self.types.push(node.ty);
        id
    }

    /// Get the expression kind for a node.
    #[inline]
    pub fn kind(&self, id: CanId) -> &CanExpr {
        &self.kinds[id.index()]
    }

    /// Get the source span for a node.
    #[inline]
    pub fn span(&self, id: CanId) -> Span {
        self.spans[id.index()]
    }

    /// Get the resolved type for a node.
    #[inline]
    pub fn ty(&self, id: CanId) -> TypeId {
        self.types[id.index()]
    }

    /// Reconstruct a full `CanNode` from parallel arrays.
    pub fn get(&self, id: CanId) -> CanNode {
        CanNode {
            kind: self.kinds[id.index()],
            span: self.spans[id.index()],
            ty: self.types[id.index()],
        }
    }

    /// Number of allocated nodes.
    pub fn len(&self) -> usize {
        self.kinds.len()
    }

    /// Returns `true` if no nodes have been allocated.
    pub fn is_empty(&self) -> bool {
        self.kinds.is_empty()
    }

    /// Allocate a contiguous range of expression IDs (for args, elements, stmts).
    pub fn push_expr_list(&mut self, ids: &[CanId]) -> CanRange {
        if ids.is_empty() {
            return CanRange::EMPTY;
        }
        let start = to_u32(self.expr_lists.len(), "expression lists");
        self.expr_lists.extend_from_slice(ids);
        CanRange::new(start, to_u16(ids.len(), "expression list"))
    }

    /// Get expression IDs from a range.
    pub fn get_expr_list(&self, range: CanRange) -> &[CanId] {
        if range.is_empty() {
            return &[];
        }
        let start = range.start as usize;
        let end = start + range.len();
        &self.expr_lists[start..end]
    }

    /// Begin building an expression list incrementally.
    pub fn start_expr_list(&self) -> u32 {
        to_u32(self.expr_lists.len(), "expression lists")
    }

    /// Push one ID to the list being built.
    pub fn push_expr_list_item(&mut self, id: CanId) {
        self.expr_lists.push(id);
    }

    /// Append all items from an existing expression list into the list being built.
    ///
    /// Use between `start_expr_list()` and `finish_expr_list()` to splice in
    /// items from an existing range without an intermediate `Vec` allocation.
    pub fn extend_expr_list(&mut self, src: CanRange) {
        if src.is_empty() {
            return;
        }
        let start = src.start as usize;
        self.expr_lists.extend_from_within(start..start + src.len());
    }

    /// Finish building an expression list.
    pub fn finish_expr_list(&self, start: u32) -> CanRange {
        let len = to_u16(self.expr_lists.len() - start as usize, "expression list");
        if len == 0 {
            CanRange::EMPTY
        } else {
            CanRange::new(start, len)
        }
    }

    /// Allocate a contiguous range of map entries.
    pub fn push_map_entries(&mut self, entries: &[CanMapEntry]) -> CanMapEntryRange {
        if entries.is_empty() {
            return CanMapEntryRange::EMPTY;
        }
        let start = to_u32(self.map_entries.len(), "map entries");
        self.map_entries.extend_from_slice(entries);
        CanMapEntryRange::new(start, to_u16(entries.len(), "map entry list"))
    }

    /// Get map entries from a range.
    pub fn get_map_entries(&self, range: CanMapEntryRange) -> &[CanMapEntry] {
        if range.is_empty() {
            return &[];
        }
        let start = range.start as usize;
        let end = start + range.len();
        &self.map_entries[start..end]
    }

    /// Allocate a contiguous range of struct field initializers.
    pub fn push_fields(&mut self, fields: &[CanField]) -> CanFieldRange {
        if fields.is_empty() {
            return CanFieldRange::EMPTY;
        }
        let start = to_u32(self.fields.len(), "struct fields");
        self.fields.extend_from_slice(fields);
        CanFieldRange::new(start, to_u16(fields.len(), "struct field list"))
    }

    /// Get struct fields from a range.
    pub fn get_fields(&self, range: CanFieldRange) -> &[CanField] {
        if range.is_empty() {
            return &[];
        }
        let start = range.start as usize;
        let end = start + range.len();
        &self.fields[start..end]
    }

    /// Allocate a canonical binding pattern, returning its ID.
    pub fn push_binding_pattern(&mut self, pattern: CanBindingPattern) -> CanBindingPatternId {
        let id = CanBindingPatternId::new(to_u32(self.binding_patterns.len(), "binding patterns"));
        self.binding_patterns.push(pattern);
        id
    }

    /// Get a canonical binding pattern by ID.
    pub fn get_binding_pattern(&self, id: CanBindingPatternId) -> &CanBindingPattern {
        &self.binding_patterns[id.index()]
    }

    /// Allocate a range of binding pattern IDs (for Tuple/List sub-patterns).
    pub fn push_binding_pattern_list(
        &mut self,
        ids: &[CanBindingPatternId],
    ) -> CanBindingPatternRange {
        if ids.is_empty() {
            return CanBindingPatternRange::EMPTY;
        }
        let start = to_u32(self.binding_pattern_lists.len(), "binding pattern lists");
        self.binding_pattern_lists.extend_from_slice(ids);
        CanBindingPatternRange::new(start, to_u16(ids.len(), "binding pattern list"))
    }

    /// Get binding pattern IDs from a range.
    pub fn get_binding_pattern_list(
        &self,
        range: CanBindingPatternRange,
    ) -> &[CanBindingPatternId] {
        if range.is_empty() {
            return &[];
        }
        let start = range.start as usize;
        let end = start + range.len();
        &self.binding_pattern_lists[start..end]
    }

    /// Allocate a range of field bindings.
    pub fn push_field_bindings(&mut self, bindings: &[CanFieldBinding]) -> CanFieldBindingRange {
        if bindings.is_empty() {
            return CanFieldBindingRange::EMPTY;
        }
        let start = to_u32(self.field_bindings.len(), "field bindings");
        self.field_bindings.extend_from_slice(bindings);
        CanFieldBindingRange::new(start, to_u16(bindings.len(), "field binding list"))
    }

    /// Get field bindings from a range.
    pub fn get_field_bindings(&self, range: CanFieldBindingRange) -> &[CanFieldBinding] {
        if range.is_empty() {
            return &[];
        }
        let start = range.start as usize;
        let end = start + range.len();
        &self.field_bindings[start..end]
    }

    /// Allocate a range of canonical parameters.
    pub fn push_params(&mut self, params: &[CanParam]) -> CanParamRange {
        if params.is_empty() {
            return CanParamRange::EMPTY;
        }
        let start = to_u32(self.params.len(), "params");
        self.params.extend_from_slice(params);
        CanParamRange::new(start, to_u16(params.len(), "param list"))
    }

    /// Get canonical parameters from a range.
    pub fn get_params(&self, range: CanParamRange) -> &[CanParam] {
        if range.is_empty() {
            return &[];
        }
        let start = range.start as usize;
        let end = start + range.len();
        &self.params[start..end]
    }

    /// Allocate a range of named expressions.
    pub fn push_named_exprs(&mut self, exprs: &[CanNamedExpr]) -> CanNamedExprRange {
        if exprs.is_empty() {
            return CanNamedExprRange::EMPTY;
        }
        let start = to_u32(self.named_exprs.len(), "named exprs");
        self.named_exprs.extend_from_slice(exprs);
        CanNamedExprRange::new(start, to_u16(exprs.len(), "named expr list"))
    }

    /// Get named expressions from a range.
    pub fn get_named_exprs(&self, range: CanNamedExprRange) -> &[CanNamedExpr] {
        if range.is_empty() {
            return &[];
        }
        let start = range.start as usize;
        let end = start + range.len();
        &self.named_exprs[start..end]
    }
}

impl Default for CanArena {
    fn default() -> Self {
        Self::new()
    }
}

impl PartialEq for CanArena {
    fn eq(&self, other: &Self) -> bool {
        self.kinds == other.kinds
            && self.spans == other.spans
            && self.types == other.types
            && self.expr_lists == other.expr_lists
            && self.map_entries == other.map_entries
            && self.fields == other.fields
            && self.binding_patterns == other.binding_patterns
            && self.binding_pattern_lists == other.binding_pattern_lists
            && self.field_bindings == other.field_bindings
            && self.params == other.params
            && self.named_exprs == other.named_exprs
    }
}

impl Eq for CanArena {}

/// A canonicalized function root — body + defaults in canonical IR.
///
/// Replaces the previous `(Name, CanId)` tuple in `CanonResult.roots`,
/// adding canonical default expressions so that the evaluator can use
/// `eval_can(CanId)` instead of `eval(ExprId)` for default parameter values.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanonRoot {
    /// Function or test name.
    pub name: Name,
    /// Canonical body expression.
    pub body: CanId,
    /// Canonical default expressions, parallel to the function's parameter list.
    /// `defaults[i]` is `Some(can_id)` if parameter `i` has a default value,
    /// `None` if the parameter is required.
    pub defaults: Vec<Option<CanId>>,
}

/// A canonicalized method root — body in canonical IR.
///
/// Replaces the previous `(Name, Name, CanId)` tuple in `CanonResult.method_roots`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MethodRoot {
    /// Type that owns the method (e.g., `Point`, `list`).
    pub type_name: Name,
    /// Method name.
    pub method_name: Name,
    /// Canonical body expression.
    pub body: CanId,
}

/// Output of the canonicalization pass.
///
/// Contains everything needed by both backends: the canonical expression
/// arena, constant pool, decision trees, and the root expression.
///
/// # Salsa Compatibility
///
/// Implements Clone, Debug for Salsa query return types.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanonResult {
    /// The canonical expression arena.
    pub arena: CanArena,
    /// Pool of compile-time constant values.
    pub constants: ConstantPool,
    /// Pool of compiled decision trees.
    pub decision_trees: DecisionTreePool,
    /// The root expression (entry point for single-expression lowering).
    pub root: CanId,
    /// Named roots for module-level lowering (one per function/test).
    pub roots: Vec<CanonRoot>,
    /// Method roots for `impl`/`extend`/`def_impl` blocks.
    pub method_roots: Vec<MethodRoot>,
    /// Pattern problems detected during exhaustiveness checking.
    pub problems: Vec<PatternProblem>,
}

impl CanonResult {
    /// Create an empty result (for error recovery).
    pub fn empty() -> Self {
        Self {
            arena: CanArena::new(),
            constants: ConstantPool::new(),
            decision_trees: DecisionTreePool::new(),
            root: CanId::INVALID,
            roots: Vec::new(),
            method_roots: Vec::new(),
            problems: Vec::new(),
        }
    }

    /// Look up a named root by function name.
    pub fn root_for(&self, name: Name) -> Option<CanId> {
        self.roots.iter().find(|r| r.name == name).map(|r| r.body)
    }

    /// Look up a canon root by function name (includes defaults).
    pub fn canon_root_for(&self, name: Name) -> Option<&CanonRoot> {
        self.roots.iter().find(|r| r.name == name)
    }

    /// Look up a method root by type name and method name.
    pub fn method_root_for(&self, type_name: Name, method_name: Name) -> Option<CanId> {
        self.method_roots
            .iter()
            .find(|r| r.type_name == type_name && r.method_name == method_name)
            .map(|r| r.body)
    }
}

/// Thread-safe shared reference to a `CanonResult`.
///
/// Analogous to `SharedArena` but for canonical IR. Functions carry this
/// to resolve `CanId` values in their body during evaluation.
#[derive(Clone, Debug)]
#[expect(
    clippy::disallowed_types,
    reason = "Arc is the implementation of SharedCanonResult"
)]
pub struct SharedCanonResult(std::sync::Arc<CanonResult>);

#[expect(
    clippy::disallowed_types,
    reason = "Arc is the implementation of SharedCanonResult"
)]
impl SharedCanonResult {
    /// Create a new shared canon result.
    pub fn new(result: CanonResult) -> Self {
        Self(std::sync::Arc::new(result))
    }
}

impl std::ops::Deref for SharedCanonResult {
    type Target = CanonResult;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use std::mem;

    use crate::Span;

    use super::*;

    // ── Size Assertions ─────────────────────────────────────────

    #[test]
    fn can_expr_size() {
        assert_eq!(mem::size_of::<CanExpr>(), 24);
    }

    #[test]
    fn can_id_size() {
        assert_eq!(mem::size_of::<CanId>(), 4);
    }

    #[test]
    fn can_range_size() {
        assert_eq!(mem::size_of::<CanRange>(), 8);
    }

    #[test]
    fn can_map_entry_range_size() {
        assert_eq!(mem::size_of::<CanMapEntryRange>(), 8);
    }

    #[test]
    fn can_field_range_size() {
        assert_eq!(mem::size_of::<CanFieldRange>(), 8);
    }

    #[test]
    fn constant_id_size() {
        assert_eq!(mem::size_of::<ConstantId>(), 4);
    }

    #[test]
    fn decision_tree_id_size() {
        assert_eq!(mem::size_of::<DecisionTreeId>(), 4);
    }

    // ── CanId ───────────────────────────────────────────────────

    #[test]
    fn can_id_invalid() {
        assert!(!CanId::INVALID.is_valid());
        assert!(CanId::new(0).is_valid());
        assert!(CanId::new(42).is_valid());
    }

    #[test]
    fn can_id_default_is_invalid() {
        let id: CanId = CanId::default();
        assert!(!id.is_valid());
    }

    #[test]
    fn can_id_debug() {
        assert_eq!(format!("{:?}", CanId::INVALID), "CanId::INVALID");
        assert_eq!(format!("{:?}", CanId::new(5)), "CanId(5)");
    }

    // ── CanRange ────────────────────────────────────────────────

    #[test]
    fn can_range_empty() {
        assert!(CanRange::EMPTY.is_empty());
        assert_eq!(CanRange::EMPTY.len(), 0);
    }

    #[test]
    fn can_range_new() {
        let r = CanRange::new(10, 5);
        assert_eq!(r.start, 10);
        assert_eq!(r.len(), 5);
        assert!(!r.is_empty());
    }

    #[test]
    fn can_range_debug() {
        let r = CanRange::new(5, 3);
        assert_eq!(format!("{r:?}"), "CanRange(5..8)");
    }

    // ── CanArena ────────────────────────────────────────────────

    #[test]
    fn arena_push_and_get() {
        let mut arena = CanArena::new();
        let node = CanNode::new(CanExpr::Int(42), Span::DUMMY, TypeId::INT);
        let id = arena.push(node);

        assert_eq!(*arena.kind(id), CanExpr::Int(42));
        assert_eq!(arena.span(id), Span::DUMMY);
        assert_eq!(arena.ty(id), TypeId::INT);
        assert_eq!(arena.len(), 1);
    }

    #[test]
    fn arena_multiple_nodes() {
        let mut arena = CanArena::new();
        let id1 = arena.push(CanNode::new(CanExpr::Int(1), Span::DUMMY, TypeId::INT));
        let id2 = arena.push(CanNode::new(CanExpr::Bool(true), Span::DUMMY, TypeId::BOOL));
        let id3 = arena.push(CanNode::new(CanExpr::Unit, Span::DUMMY, TypeId::UNIT));

        assert_eq!(id1.raw(), 0);
        assert_eq!(id2.raw(), 1);
        assert_eq!(id3.raw(), 2);
        assert_eq!(arena.len(), 3);

        assert_eq!(*arena.kind(id1), CanExpr::Int(1));
        assert_eq!(*arena.kind(id2), CanExpr::Bool(true));
        assert_eq!(*arena.kind(id3), CanExpr::Unit);
    }

    #[test]
    fn arena_expr_list() {
        let mut arena = CanArena::new();
        let id1 = arena.push(CanNode::new(CanExpr::Int(1), Span::DUMMY, TypeId::INT));
        let id2 = arena.push(CanNode::new(CanExpr::Int(2), Span::DUMMY, TypeId::INT));
        let id3 = arena.push(CanNode::new(CanExpr::Int(3), Span::DUMMY, TypeId::INT));

        let range = arena.push_expr_list(&[id1, id2, id3]);
        assert_eq!(range.len(), 3);

        let ids = arena.get_expr_list(range);
        assert_eq!(ids, &[id1, id2, id3]);
    }

    #[test]
    fn arena_empty_expr_list() {
        let mut arena = CanArena::new();
        let range = arena.push_expr_list(&[]);
        assert!(range.is_empty());
        assert_eq!(arena.get_expr_list(range), &[]);
    }

    #[test]
    fn arena_incremental_expr_list() {
        let mut arena = CanArena::new();
        let id1 = arena.push(CanNode::new(CanExpr::Int(1), Span::DUMMY, TypeId::INT));
        let id2 = arena.push(CanNode::new(CanExpr::Int(2), Span::DUMMY, TypeId::INT));

        let start = arena.start_expr_list();
        arena.push_expr_list_item(id1);
        arena.push_expr_list_item(id2);
        let range = arena.finish_expr_list(start);

        assert_eq!(range.len(), 2);
        assert_eq!(arena.get_expr_list(range), &[id1, id2]);
    }

    #[test]
    fn arena_map_entries() {
        let mut arena = CanArena::new();
        let k = arena.push(CanNode::new(
            CanExpr::Str(Name::EMPTY),
            Span::DUMMY,
            TypeId::STR,
        ));
        let v = arena.push(CanNode::new(CanExpr::Int(42), Span::DUMMY, TypeId::INT));

        let range = arena.push_map_entries(&[CanMapEntry { key: k, value: v }]);
        assert_eq!(range.len(), 1);

        let entries = arena.get_map_entries(range);
        assert_eq!(entries[0].key, k);
        assert_eq!(entries[0].value, v);
    }

    #[test]
    fn arena_fields() {
        let mut arena = CanArena::new();
        let v = arena.push(CanNode::new(CanExpr::Int(0), Span::DUMMY, TypeId::INT));

        let range = arena.push_fields(&[CanField {
            name: Name::from_raw(1),
            value: v,
        }]);
        assert_eq!(range.len(), 1);

        let fields = arena.get_fields(range);
        assert_eq!(fields[0].name, Name::from_raw(1));
        assert_eq!(fields[0].value, v);
    }

    // ── ConstantPool ────────────────────────────────────────────

    #[test]
    fn constant_pool_sentinels() {
        let pool = ConstantPool::new();
        assert_eq!(*pool.get(ConstantPool::UNIT), ConstValue::Unit);
        assert_eq!(*pool.get(ConstantPool::TRUE), ConstValue::Bool(true));
        assert_eq!(*pool.get(ConstantPool::FALSE), ConstValue::Bool(false));
        assert_eq!(*pool.get(ConstantPool::ZERO), ConstValue::Int(0));
        assert_eq!(*pool.get(ConstantPool::ONE), ConstValue::Int(1));
        assert_eq!(
            *pool.get(ConstantPool::EMPTY_STR),
            ConstValue::Str(Name::EMPTY)
        );
    }

    #[test]
    fn constant_pool_intern_dedup() {
        let mut pool = ConstantPool::new();
        let id1 = pool.intern(ConstValue::Int(42));
        let id2 = pool.intern(ConstValue::Int(42));
        assert_eq!(id1, id2); // same constant → same ID
    }

    #[test]
    fn constant_pool_intern_distinct() {
        let mut pool = ConstantPool::new();
        let id1 = pool.intern(ConstValue::Int(42));
        let id2 = pool.intern(ConstValue::Int(43));
        assert_ne!(id1, id2); // different constants → different IDs
    }

    #[test]
    fn constant_pool_sentinel_dedup() {
        let mut pool = ConstantPool::new();
        // Interning a sentinel value should return the pre-interned ID.
        let id = pool.intern(ConstValue::Bool(true));
        assert_eq!(id, ConstantPool::TRUE);
    }

    // ── DecisionTreePool ────────────────────────────────────────

    #[test]
    fn decision_tree_pool_push_and_get() {
        let mut pool = DecisionTreePool::new();
        let tree = DecisionTree::Leaf {
            arm_index: 0,
            bindings: vec![],
        };
        let id = pool.push(tree.clone());
        assert_eq!(*pool.get(id), tree);
        assert_eq!(pool.len(), 1);
    }

    // ── CanonResult ─────────────────────────────────────────────

    #[test]
    fn canon_result_empty() {
        let result = CanonResult::empty();
        assert!(!result.root.is_valid());
        assert!(result.arena.is_empty());
    }

    // ── CanExpr equality / hashing ──────────────────────────────

    #[test]
    fn can_expr_eq() {
        assert_eq!(CanExpr::Int(42), CanExpr::Int(42));
        assert_ne!(CanExpr::Int(42), CanExpr::Int(43));
        assert_ne!(CanExpr::Int(42), CanExpr::Float(42));
    }

    #[test]
    fn can_expr_hash_consistency() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(CanExpr::Int(42));
        set.insert(CanExpr::Int(42)); // duplicate
        set.insert(CanExpr::Bool(true));
        assert_eq!(set.len(), 2);
    }
}
