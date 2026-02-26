//! `TypeInfo` enum — LLVM-specific type representation for codegen.
//!
//! Every Ori type category gets a [`TypeInfo`] variant that encapsulates its
//! LLVM representation, memory layout, and calling convention. Adding a new
//! type means adding one enum variant — not modifying match arms across the
//! codebase.

use inkwell::types::BasicTypeEnum;

use ori_ir::Name;
use ori_types::Idx;

use crate::context::SimpleCx;

/// Variant info for a single enum variant, stored in `TypeInfo::Enum`.
#[derive(Clone, Debug)]
pub struct EnumVariantInfo {
    /// Variant name (interned).
    pub name: Name,
    /// Field types (empty for unit variants, one element for tuple variants).
    pub fields: Vec<Idx>,
}

/// LLVM-specific type information for code generation.
///
/// Every Ori type category gets a variant. The enum encapsulates all
/// information needed to generate LLVM IR for values of this type:
/// representation, size, ABI, copy/destroy emission.
///
/// ARC classification is NOT here — it lives in `ori_arc::ArcClassification`
/// (no LLVM dependency). This enum is purely about LLVM code generation.
#[derive(Clone, Debug)]
pub enum TypeInfo {
    /// `int` -> i64
    Int,
    /// `float` -> f64
    Float,
    /// `bool` -> i1
    Bool,
    /// `char` -> i32 (Unicode scalar value)
    Char,
    /// `byte` -> i8
    Byte,
    /// `unit` -> i64 (LLVM void cannot be stored/passed/phi'd)
    Unit,
    /// `never` -> i64 (LLVM void cannot be stored/passed/phi'd)
    Never,
    /// `str` -> {i64 len, ptr data}
    Str,
    /// `duration` -> i64 (nanoseconds)
    Duration,
    /// `size` -> i64 (bytes)
    Size,
    /// `ordering` -> i8 (Less=0, Equal=1, Greater=2)
    Ordering,
    /// `[T]` -> {i64 len, i64 cap, ptr data}
    List { element: Idx },
    /// `{K: V}` -> {i64 len, i64 cap, ptr keys, ptr vals}
    Map { key: Idx, value: Idx },
    /// `set[T]` -> {i64 len, i64 cap, ptr data}
    Set { element: Idx },
    /// `(A, B, ...)` -> {A, B, ...}
    Tuple { elements: Vec<Idx> },
    /// `option[T]` -> {i8 tag, T payload}
    Option { inner: Idx },
    /// `result[T, E]` -> {i8 tag, max(T, E) payload}
    Result { ok: Idx, err: Idx },
    /// `range` -> {i64 start, i64 end, i64 step, i64 inclusive}
    Range,
    /// User-defined struct -> {field1, field2, ...}
    Struct { fields: Vec<(Name, Idx)> },
    /// User-defined enum -> {tag, max(variant payloads)}
    Enum { variants: Vec<EnumVariantInfo> },
    /// `Iterator<T>` -> ptr (opaque heap-allocated iterator handle)
    Iterator { element: Idx },
    /// `chan<T>` -> ptr (opaque heap-allocated channel)
    Channel { element: Idx },
    /// `(P1, ...) -> R` -> ptr (function pointer or closure pointer)
    Function { params: Vec<Idx>, ret: Idx },
    /// Error/unknown type fallback.
    ///
    /// Used for types that should never reach codegen:
    /// `Var`, `BoundVar`, `RigidVar`, `Scheme`, `Projection`,
    /// `ModuleNs`, `Infer`, `SelfType`.
    Error,
}

impl TypeInfo {
    /// The LLVM type used to represent values of this type in memory.
    ///
    /// This is the canonical type mapping. Every `TypeInfo` variant knows
    /// exactly how it maps to LLVM IR, making extension straightforward.
    pub fn storage_type<'ll>(&self, scx: &SimpleCx<'ll>) -> BasicTypeEnum<'ll> {
        match self {
            // Primitives
            Self::Int | Self::Duration | Self::Size | Self::Unit | Self::Never => {
                scx.type_i64().into()
            }
            Self::Float => scx.type_f64().into(),
            Self::Bool => scx.type_i1().into(),
            Self::Char => scx.type_i32().into(),
            Self::Byte | Self::Ordering => scx.type_i8().into(),

            // Str: {i64 len, ptr data}
            Self::Str => scx
                .type_struct(&[scx.type_i64().into(), scx.type_ptr().into()], false)
                .into(),

            // Collections
            Self::List { .. } | Self::Set { .. } => scx
                .type_struct(
                    &[
                        scx.type_i64().into(),
                        scx.type_i64().into(),
                        scx.type_ptr().into(),
                    ],
                    false,
                )
                .into(),

            Self::Map { .. } => scx
                .type_struct(
                    &[
                        scx.type_i64().into(),
                        scx.type_i64().into(),
                        scx.type_ptr().into(),
                        scx.type_ptr().into(),
                    ],
                    false,
                )
                .into(),

            // Range layout: {i64 start, i64 end, i64 step, i64 inclusive}
            // ARC IR constructs ranges as 4-element tuples with i64 fields.
            // The inclusive flag is stored as i64 (0/1) and truncated to i1
            // only when calling ori_iter_from_range.
            Self::Range => scx
                .type_struct(
                    &[
                        scx.type_i64().into(),
                        scx.type_i64().into(),
                        scx.type_i64().into(),
                        scx.type_i64().into(),
                    ],
                    false,
                )
                .into(),

            // Tagged unions: {i8 tag, payload}
            // Option uses the inner type directly as payload.
            // Result uses the larger of ok/err — for now, uses ok type.
            // TODO: Result should use max(ok, err) size for correct layout.
            Self::Option { inner } => {
                // Payload type depends on inner type. Since we don't have
                // the store here, use i64 as a uniform payload representation.
                // The actual payload coercion happens at emit time.
                let _ = inner;
                scx.type_struct(&[scx.type_i64().into(), scx.type_i64().into()], false)
                    .into()
            }
            Self::Result { ok, err } => {
                let _ = (ok, err);
                scx.type_struct(&[scx.type_i64().into(), scx.type_i64().into()], false)
                    .into()
            }

            // Tuple: struct of element types. Without the store, we can't
            // resolve element types here. This returns an empty struct as
            // placeholder — actual tuple lowering uses TypeInfoStore which
            // has access to resolve element types via the Pool.
            Self::Tuple { elements } => {
                // Placeholder: tuple of N i64s. Real lowering via store.
                let fields: Vec<BasicTypeEnum<'ll>> =
                    elements.iter().map(|_| scx.type_i64().into()).collect();
                scx.type_struct(&fields, false).into()
            }

            // Iterator / Channel: opaque heap-allocated handles
            Self::Iterator { .. } | Self::Channel { .. } => scx.type_ptr().into(),

            // Function: fat-pointer closure { fn_ptr: ptr, env_ptr: ptr }
            // All function-typed values use this two-pointer representation,
            // even non-closures (which have env_ptr = null). This uniform
            // representation avoids branching at call sites.
            Self::Function { .. } => scx
                .type_struct(&[scx.type_ptr().into(), scx.type_ptr().into()], false)
                .into(),

            // User-defined types (placeholder — resolved via TypeInfoStore)
            Self::Struct { fields } => {
                let field_types: Vec<BasicTypeEnum<'ll>> =
                    fields.iter().map(|_| scx.type_i64().into()).collect();
                scx.type_struct(&field_types, false).into()
            }
            Self::Enum { .. } => {
                // Default: {i64 tag, i64 payload} — real layout computed by store
                scx.type_struct(&[scx.type_i64().into(), scx.type_i64().into()], false)
                    .into()
            }

            // Error fallback
            Self::Error => scx.type_i64().into(),
        }
    }

    /// Size in bytes (ABI size).
    ///
    /// Returns `None` for types whose size depends on element types and
    /// can only be computed with a `TypeInfoStore` (which has Pool access).
    ///
    /// Used by Section 04's `compute_param_passing()` and `compute_return_passing()`.
    pub fn size(&self) -> Option<u64> {
        match self {
            // 8-byte types: scalars, handles, error fallback
            Self::Int
            | Self::Float
            | Self::Duration
            | Self::Size
            | Self::Unit
            | Self::Never
            | Self::Iterator { .. }
            | Self::Channel { .. }
            | Self::Error => Some(8),

            // 1-byte types
            Self::Bool | Self::Byte | Self::Ordering => Some(1),

            // 4-byte types
            Self::Char => Some(4),

            // 16-byte types:
            // Function: fat-pointer closure { ptr, ptr }
            // Str: {i64, ptr}
            // Option/Result: {i64, T} — uniform i64 tag + payload = 16 bytes
            Self::Function { .. } | Self::Str | Self::Option { .. } | Self::Result { .. } => {
                Some(16)
            }

            // List/Set: {i64, i64, ptr} = 24 bytes
            Self::List { .. } | Self::Set { .. } => Some(24),

            // Range: {i64 start, i64 end, i64 step, i64 inclusive} = 32 bytes
            // Map: {i64, i64, ptr, ptr} = 32 bytes
            Self::Range | Self::Map { .. } => Some(32),

            // Dynamic-size types: depend on element/field types
            Self::Tuple { .. } | Self::Struct { .. } | Self::Enum { .. } => None,
        }
    }

    /// The type name matching `TYPECK_BUILTIN_METHODS` convention.
    ///
    /// Returns `Some("int")` for `TypeInfo::Int`, `Some("Option")` for
    /// `TypeInfo::Option { .. }`, etc. Returns `None` for types without
    /// builtin methods (Unit, Never, user-defined structs/enums).
    ///
    /// Naming convention follows `TYPECK_BUILTIN_METHODS`: lowercase for
    /// primitive syntax types (`int`, `str`, `list`, `map`, `range`, `tuple`),
    /// `PascalCase` for named/standard types (`Option`, `Result`, `Set`,
    /// `Iterator`, `Channel`, `Duration`, `Size`, `Ordering`).
    pub fn builtin_type_name(&self) -> Option<&'static str> {
        match self {
            Self::Int => Some("int"),
            Self::Float => Some("float"),
            Self::Bool => Some("bool"),
            Self::Char => Some("char"),
            Self::Byte => Some("byte"),
            Self::Str => Some("str"),
            Self::Duration => Some("Duration"),
            Self::Size => Some("Size"),
            Self::Ordering => Some("Ordering"),
            Self::List { .. } => Some("list"),
            Self::Map { .. } => Some("map"),
            Self::Set { .. } => Some("Set"),
            Self::Tuple { .. } => Some("tuple"),
            Self::Option { .. } => Some("Option"),
            Self::Result { .. } => Some("Result"),
            Self::Range => Some("range"),
            Self::Iterator { .. } => Some("Iterator"),
            Self::Channel { .. } => Some("Channel"),
            Self::Error => Some("error"),
            // No builtin methods for these types
            Self::Unit
            | Self::Never
            | Self::Struct { .. }
            | Self::Enum { .. }
            | Self::Function { .. } => None,
        }
    }

    /// Alignment in bytes.
    ///
    /// Returns the required alignment for this type. On x86-64, all types
    /// align to at most 8 bytes.
    pub fn alignment(&self) -> u32 {
        match self {
            Self::Bool | Self::Byte | Self::Ordering => 1,
            Self::Char => 4,
            // Everything else aligns to 8 on x86-64
            _ => 8,
        }
    }

    /// True if this type has no ARC semantics (no retain/release needed).
    ///
    /// Trivial types are passed by value and don't participate in
    /// reference counting. This is the codegen-level triviality check;
    /// the ARC-level classification lives in `ori_arc`.
    pub fn is_trivial(&self) -> bool {
        match self {
            // Scalar primitives and error fallback are trivial
            Self::Int
            | Self::Float
            | Self::Bool
            | Self::Char
            | Self::Byte
            | Self::Unit
            | Self::Never
            | Self::Duration
            | Self::Size
            | Self::Ordering
            | Self::Range
            | Self::Error => true,

            // Everything else has heap data or may contain heap data.
            // Tagged unions (Option/Result) and composites (Tuple/Struct/Enum)
            // are conservatively non-trivial — precise classification requires
            // transitive field analysis (future: ori_arc ArcClassification).
            Self::Str
            | Self::List { .. }
            | Self::Map { .. }
            | Self::Set { .. }
            | Self::Iterator { .. }
            | Self::Channel { .. }
            | Self::Function { .. }
            | Self::Option { .. }
            | Self::Result { .. }
            | Self::Tuple { .. }
            | Self::Struct { .. }
            | Self::Enum { .. } => false,
        }
    }

    /// True if values fit in registers and can be loaded/stored directly.
    ///
    /// Non-loadable types must be passed by reference (sret ABI).
    pub fn is_loadable(&self) -> bool {
        match self.size() {
            Some(size) => size <= 16,
            // Unknown size — conservatively not loadable
            None => false,
        }
    }
}
