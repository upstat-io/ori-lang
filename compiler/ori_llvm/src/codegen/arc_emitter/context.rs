//! Shared types for the ARC IR emitter.
//!
//! Contains helper types that bridge ARC IR value representations to LLVM:
//! - [`EmittedValue`] — tagged LLVM value carrying its memory representation
//! - [`InvokeMode`] — call vs invoke dispatch control
//! - [`CodegenContext`] — shared function-resolution lookup tables
//! - [`is_boxed_enum_field`] — recursive enum field detection

use ori_ir::Name;
use ori_types::{Idx, Pool, Tag};
use rustc_hash::{FxHashMap, FxHashSet};

use super::super::abi::FunctionAbi;
use super::super::value_id::{BlockId, FunctionId, ValueId};
use crate::codegen::arc_emitter::ArcIrEmitter;

use ori_arc::ir::ValueRepr;

// Recursive enum detection

/// Check if a field type creates a direct recursive reference within an enum.
///
/// Returns `true` when the resolved field type is the same Pool index as the
/// resolved enum type — meaning the field must be RC-boxed when stored in the
/// enum payload. Layout: stored as an 8-byte RC pointer instead of inline.
///
/// Only handles direct self-recursion (e.g., `type Tree = Node(Tree, Tree)`).
/// Mutual recursion (`type A = X(B)`, `type B = Y(A)`) is not yet supported.
pub(super) fn is_boxed_enum_field(pool: &Pool, enum_type: Idx, field_type: Idx) -> bool {
    let enum_resolved = pool.resolve_fully(enum_type);
    let field_resolved = pool.resolve_fully(field_type);
    pool.tag(enum_resolved) == Tag::Enum && enum_resolved == field_resolved
}

/// Tagged LLVM value carrying its memory representation.
///
/// Wraps [`ValueId`] with variant information derived from the ARC IR's
/// [`ValueRepr`]. This prevents the "did I load this already?" and
/// "is this a pointer or a scalar?" class of bugs by making the value's
/// representation explicit at the type level.
///
/// Inspired by Rust's `OperandValue` in `rustc_codegen_llvm`.
#[derive(Clone, Copy, Debug)]
pub(super) enum EmittedValue {
    /// Register scalar: i64, f64, i1, i8, i32.
    Immediate(ValueId),
    /// Pointer to heap-allocated RC'd memory (list, map, set, etc.).
    RcPointer(ValueId),
    /// Stack aggregate: struct, tuple, enum by value, fat value (str, closure).
    Aggregate(ValueId),
    /// Two-word split: {first, second} — str={len,ptr}, closure={fn,env}.
    /// The `second` component is typically the RC-managed pointer.
    /// Used by Section 01.3 when RC operations need direct component access.
    #[expect(dead_code, reason = "reserved for Section 01.3 RcStrategy split")]
    Pair { first: ValueId, second: ValueId },
    /// No runtime representation (unit, never).
    /// Used when ZST values are tracked through the pipeline.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "reserved for Section 01.3 ZST propagation")
    )]
    ZeroSized,
}

impl EmittedValue {
    /// Extract the single underlying [`ValueId`].
    ///
    /// # Panics
    /// Panics on `Pair` (two values) and `ZeroSized` (no value).
    /// For those variants, destructure the enum directly.
    pub(super) fn into_raw(self) -> ValueId {
        match self {
            Self::Immediate(v) | Self::RcPointer(v) | Self::Aggregate(v) => v,
            Self::Pair { .. } => {
                panic!("EmittedValue::Pair has no single ValueId — destructure instead")
            }
            Self::ZeroSized => panic!("EmittedValue::ZeroSized has no ValueId"),
        }
    }

    /// Get the RC-trackable data pointer, if this value is reference-counted.
    ///
    /// - `RcPointer` → the pointer itself
    /// - `Pair` → the second component (typically the RC-managed pointer)
    /// - Others → `None`
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "reserved for Section 01.3 RC strategy dispatch")
    )]
    pub(super) fn rc_data_ptr(self) -> Option<ValueId> {
        match self {
            Self::RcPointer(v) => Some(v),
            Self::Pair { second, .. } => Some(second),
            _ => None,
        }
    }

    /// True if this value contains a reference-counted component.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "reserved for Section 01.3 RC strategy dispatch")
    )]
    pub(super) fn is_rc_managed(self) -> bool {
        matches!(self, Self::RcPointer(_) | Self::Pair { .. })
    }

    /// Bridge from an ARC IR [`ValueRepr`] to an emitted value.
    ///
    /// Maps single-valued representations directly. `FatValue` is stored
    /// as `Aggregate` (the two components remain packed in a single LLVM
    /// struct value); use `Pair` only when the components are split.
    pub(super) fn from_repr(repr: ValueRepr, value: ValueId) -> Self {
        match repr {
            ValueRepr::Scalar => Self::Immediate(value),
            ValueRepr::RcPointer => Self::RcPointer(value),
            ValueRepr::Aggregate | ValueRepr::FatValue => Self::Aggregate(value),
        }
    }
}

/// Controls whether a function call emits LLVM `invoke` (with unwind) or `call` + `br`.
///
/// Eliminates the boolean flag + dead `unwind_block` parameter pattern:
/// - `Invoke { unwind }` carries the unwind block only when needed
/// - `Call { normal }` makes it clear no unwind handling is needed
#[derive(Clone, Copy, Debug)]
pub(super) enum InvokeMode {
    /// Emit LLVM `invoke` with both normal and unwind continuations.
    Invoke { normal: BlockId, unwind: BlockId },
    /// Emit LLVM `call` + unconditional `br` to normal block (nounwind callee).
    Call { normal: BlockId },
}

impl InvokeMode {
    /// The normal continuation block (used by both variants).
    pub(super) fn normal_block(self) -> BlockId {
        match self {
            Self::Invoke { normal, .. } | Self::Call { normal } => normal,
        }
    }
}

/// Shared lookup tables for function resolution during ARC IR → LLVM IR emission.
///
/// Bundles the five name-resolution maps that travel together from
/// [`FunctionCompiler`] to [`ArcIrEmitter`]. Extracting these reduces the
/// emitter constructor from 12 parameters to 7 semantically distinct ones.
#[derive(Default)]
pub struct CodegenContext {
    /// Declared functions: `Name` → (`FunctionId`, ABI).
    pub functions: FxHashMap<Name, (FunctionId, FunctionAbi)>,
    /// Type-qualified method lookup: `(type_name, method_name)` → (`FunctionId`, ABI).
    pub method_functions: FxHashMap<(Name, Name), (FunctionId, FunctionAbi)>,
    /// Maps receiver type `Idx` → type `Name` for operator trait dispatch.
    pub type_idx_to_name: FxHashMap<Idx, Name>,
    /// Monomorphized generic dispatch: original name → `[(concrete_param_types, mangled_name)]`.
    ///
    /// When a non-generic function calls a generic one (e.g., `identity(42)`), the ARC IR
    /// uses the original name (`"identity"`), but the LLVM function is declared under the
    /// mangled name (`"identity$m$int"`). This index resolves the call by matching arg types.
    pub mono_dispatch: FxHashMap<Name, Vec<(Vec<Idx>, Name)>>,
    /// Known-nounwind user function names: `Invoke` terminators calling these
    /// emit `call` + `br` instead of LLVM `invoke`, eliminating landing pads.
    pub nounwind_functions: FxHashSet<Name>,
    /// Non-capturing lambda names: these are declared with closure-compatible
    /// ABI (`ccc` + phantom `ptr` env param) so `PartialApply` can point
    /// directly at them without generating a `_ori_partial_N` trampoline.
    pub non_capturing_lambdas: FxHashSet<Name>,
}

// Re-export convenience method on ArcIrEmitter for use by submodules
// that need to check boxed enum fields.
impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Check if a field type creates a direct recursive reference within an enum.
    ///
    /// Convenience wrapper around [`is_boxed_enum_field`] using the emitter's pool.
    #[allow(
        dead_code,
        reason = "convenience for submodules that don't have direct pool access"
    )]
    pub(super) fn is_boxed_enum_field(&self, enum_type: Idx, field_type: Idx) -> bool {
        is_boxed_enum_field(self.pool, enum_type, field_type)
    }
}
