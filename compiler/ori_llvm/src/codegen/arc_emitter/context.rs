//! Shared types for the ARC IR emitter.
//!
//! Contains helper types that bridge ARC IR value representations to LLVM:
//! - [`EmittedValue`] — tagged LLVM value carrying its memory representation
//! - [`InvokeMode`] — call vs invoke dispatch control
//! - [`CodegenContext`] — shared function-resolution lookup tables
//! - [`is_boxed_enum_field`] — recursive enum field detection
//! - [`is_callee_intercepted`] — callee interception check shared by nounwind analysis and emission

use ori_arc::ir::ValueRepr;
use ori_arc::ownership::Ownership;
use ori_ir::canon::MonoInstanceId;
use ori_ir::Name;
use ori_types::{Idx, Pool};
use rustc_hash::{FxHashMap, FxHashSet};

use super::super::abi::FunctionAbi;
use super::super::value_id::{BlockId, FunctionId, ValueId};
use crate::codegen::arc_emitter::ArcIrEmitter;
use crate::codegen::type_info::TypeInfoStore;

// Recursive field / payload detection

/// Whether the field/payload position holding `field_type` inside `owner_type`
/// is heap-boxed as an 8-byte RC pointer. Thin wrapper over the boxing SSOT
/// `repr_box_oracle::position_is_rc_boxed`, which the layout resolver also
/// consumes so box-vs-inline LLVM layout and Construct/Project/drop never
/// disagree. The name is retained for the enum call sites; the predicate
/// applies equally to struct fields and tuple elements.
pub(super) fn is_boxed_enum_field(pool: &Pool, owner_type: Idx, field_type: Idx) -> bool {
    crate::codegen::type_info::repr_box_oracle::position_is_rc_boxed(pool, owner_type, field_type)
}

// Callee interception detection

/// Builtin method names whose intercepted emission may call `ori_panic`
/// (e.g., `Option.expect`, `Result.unwrap`). These emit an inline
/// `call ori_panic` on the failing variant and therefore may unwind
/// through the caller — they are NOT nounwind even though they are
/// intercepted. Used by the nounwind analyzer to avoid marking callers
/// of these methods as nounwind.
///
/// The emission sites live in
/// `codegen/arc_emitter/builtins/option_result_helpers.rs`
/// (`emit_expect_branch`, `emit_unwrap_branch`). Keep this list in sync
/// with the dispatch table in `option_result.rs`.
///
/// `updated` (`IndexSet`) is included because the list lowering calls
/// `ori_list_updated_cow`, which panics on out-of-bounds keys (matching
/// `list[key]`). `__index` is included because the list lowering calls
/// `ori_list_get`, which panics on out-of-bounds access. Matching is by
/// unqualified method name, so map `updated` / map `__index` (never
/// panic) are conservatively included too.
pub(crate) const MAY_UNWIND_INTERCEPTED_METHODS: &[&str] = &[
    "unwrap",
    "unwrap_err",
    "expect",
    "expect_err",
    "updated",
    "__index",
];

/// Check if a callee will be intercepted by builtin handlers during emission.
///
/// Intercepted calls always emit `call` (never `invoke`), so they skip the
/// ARC-IR `Invoke` path. Most intercepts are nounwind — but a small set of
/// builtin methods (`unwrap`, `expect`, etc.) emit an inline `call ori_panic`
/// in their failing-variant branch and therefore MAY unwind. See
/// [`MAY_UNWIND_INTERCEPTED_METHODS`] for the canonical list; nounwind
/// analysis uses that list via [`intercepted_is_nounwind`]. Shared by
/// [`FunctionCompiler::is_arc_function_nounwind`] (nounwind analysis) and
/// [`ArcIrEmitter::callee_will_be_intercepted`] (emission).
///
/// The six checks, in order:
/// 1. Format call interceptor (`ori_format_*` prefix)
/// 2. Prelude function interceptor (exact name match)
/// 3. Protocol builtins (`__iter_next`, `__collect_set`, `__index`)
/// 4. Declared user functions — NOT intercepted (normal dispatch)
/// 5. Runtime functions (`ori_*`, `__*`) — NOT intercepted
/// 6. Builtin method heuristic: receiver is a builtin type and callee is not
///    in the method dispatch chain
pub(crate) fn is_callee_intercepted(
    callee_name: &str,
    callee: Name,
    args: &[ori_arc::ir::ArcVarId],
    func: &ori_arc::ArcFunction,
    ctx: &CodegenContext,
    type_info: &TypeInfoStore<'_>,
) -> bool {
    use super::builtins::prelude::HANDLED_PRELUDE_NAMES;

    // Format call interceptor
    if callee_name.starts_with("ori_format_") {
        return true;
    }
    // Prelude function interceptor
    if HANDLED_PRELUDE_NAMES.contains(&callee_name) {
        return true;
    }
    // All protocol builtins are nounwind (iterator creation/cleanup don't
    // panic), so they're always safe to emit as `call` rather than `invoke`.
    // Some (Index, IterNext, CollectSet, Cast) are also intercepted by
    // try_emit_protocol(); others (Iter, IterDrop) go through normal
    // function dispatch but are still nounwind.
    if ori_ir::builtin_constants::protocol::ProtocolBuiltin::from_name(callee_name).is_some() {
        return true;
    }
    // Declared user functions use normal dispatch — NOT intercepted
    if ctx.functions.contains_key(&callee) {
        return false;
    }
    // Runtime functions have their own emission paths — NOT intercepted
    if callee_name.starts_with("ori_") || callee_name.starts_with("__") {
        return false;
    }
    // Monomorphized generic dispatch: callee name resolves to a generic
    // function via lookup_mono_dispatch() during emission — NOT intercepted.
    // Without this check, a generic call like `identity(s)` where `s: str`
    // would fall through to the builtin method heuristic (str receiver →
    // true), incorrectly treating a may-unwind user function as intercepted.
    if ctx.mono_dispatch.contains_key(&callee) {
        return false;
    }
    // Builtin method: receiver is a builtin type and not in method_functions
    if let Some(&first_arg) = args.first() {
        let receiver_ty = func.var_type(first_arg);
        let info = type_info.get(receiver_ty);
        if info.builtin_type_name().is_some() {
            if let Some(type_name) = ctx.type_idx_to_name.get(&receiver_ty) {
                if !ctx.method_functions.contains_key(&(*type_name, callee)) {
                    return true;
                }
            } else {
                // Builtin type but no type_idx_to_name entry — method
                // dispatch chain can't resolve it, will be intercepted.
                return true;
            }
        }
    }
    false
}

/// Whether an intercepted may-unwind builtin emission routes its panicking
/// runtime call through the ARC unwind block (`invoke`), so the unwind
/// block's cleanup decs run on the panic path AND the panic lands in an
/// enclosing `catch(expr:)` handler (Spec: Clause 17.4 — implicit panics
/// are catchable).
///
/// Keep in sync with the emission sites; the predicate MUST be true only
/// when the emission is guaranteed to emit at least one call to a
/// non-nounwind runtime function — listing a never-panicking emission
/// would create an orphan landingpad (no `invoke` ever targets it):
/// - list `updated` (`emit_list_updated_cow`) calls `ori_list_updated_cow`,
///   which panics on out-of-bounds keys. Map `updated` never panics
///   (`ori_map_updated_cow` carries `Nounwind`).
/// - Option/Result `unwrap` / `expect` (+ Result `unwrap_err` /
///   `expect_err`) always emit a panic branch calling `ori_panic` /
///   `ori_panic_cstr` (`emit_unwrap_branch` / `emit_expect_branch`).
/// - list `__index` (`emit_list_index`) calls `ori_list_get`, which panics
///   on out-of-bounds access. Map `__index` returns Option (never panics).
///
/// Consumed by `detect_dead_unwind_blocks` (keep the unwind block live) and
/// `emit_invoke` (arm `intercepted_unwind` around the protocol + builtin
/// dispatch) — both sites MUST agree or the landingpad is orphaned / the
/// invoke targets a dead block.
pub(crate) fn intercepted_emission_invokes_unwind(
    method_name: &str,
    receiver_tag: ori_types::Tag,
) -> bool {
    use ori_types::Tag;
    match method_name {
        "updated" => matches!(receiver_tag, Tag::List),
        "__index" => matches!(receiver_tag, Tag::List | Tag::Str),
        "unwrap" | "expect" => matches!(receiver_tag, Tag::Option | Tag::Result),
        "unwrap_err" | "expect_err" => matches!(receiver_tag, Tag::Result),
        _ => false,
    }
}

/// Check whether an intercepted callee is guaranteed to be nounwind.
///
/// Called by the nounwind analyzer when [`is_callee_intercepted`] returned
/// `true` to decide whether the intercept may unwind. The default is
/// `true` (nounwind) — the exceptional cases in
/// [`MAY_UNWIND_INTERCEPTED_METHODS`] return `false` because they emit
/// an inline `call ori_panic` on a failing variant.
///
/// Matching is by unqualified method name (the callee `Name` already
/// strips the type prefix, e.g., `expect` not `Option.expect`), because
/// the may-unwind set is keyed on the method surface, not the receiver
/// type — `Option.expect` and `Result.expect` both panic via the same
/// emission helper.
#[must_use]
pub(crate) fn intercepted_is_nounwind(callee_name: &str) -> bool {
    !MAY_UNWIND_INTERCEPTED_METHODS.contains(&callee_name)
}

/// Tagged LLVM value carrying its memory representation.
///
/// Wraps [`ValueId`] with variant information derived from the ARC IR's
/// [`ValueRepr`]. This prevents the "did I load this already?" and
/// "is this a pointer or a scalar?" class of bugs by making the value's
/// representation explicit at the type level.
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
    /// Used when RC operations need direct component access.
    #[expect(dead_code, reason = "reserved for the RcStrategy split")]
    Pair { first: ValueId, second: ValueId },
    /// No runtime representation (unit, never).
    /// Used when ZST values are tracked through the pipeline.
    #[cfg_attr(not(test), expect(dead_code, reason = "reserved for ZST propagation"))]
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
        expect(dead_code, reason = "reserved for RC strategy dispatch")
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
        expect(dead_code, reason = "reserved for RC strategy dispatch")
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
    /// Per-instantiation derived-method dispatch: `(concrete_resolved_idx, method_name)` →
    /// (`FunctionId`, ABI). A generic composite (`P3Pair<int,str>`, `Box<Box<int>>`)
    /// emits one derived method per concrete instantiation, each keyed here by the
    /// materialized concrete `Struct`/`Enum` `Idx` (`pool.resolve_fully(Applied)`), so
    /// nested + multi-instantiation dispatch resolves the layout-correct body
    /// Resolution prefers this map; non-generic types fall back to the
    /// type-name-keyed `method_functions`.
    pub mono_derive_functions: FxHashMap<(Idx, Name), (FunctionId, FunctionAbi)>,
    /// Monomorphized generic dispatch: original name → `[(concrete_param_types, mangled_name)]`.
    ///
    /// When a non-generic function calls a generic one (e.g., `identity(42)`), the ARC IR
    /// uses the original name (`"identity"`), but the LLVM function is declared under the
    /// mangled name (`"identity$m$int"`). This index resolves the call by matching arg types
    /// — the legacy fallback used when the call site does not carry a `MonoInstanceId`
    /// (e.g., deferred-resolution mono instances awaiting sub-step 1b-deferred wiring).
    pub mono_dispatch: FxHashMap<Name, Vec<(Vec<Idx>, Name)>>,
    /// Monomorphized generic dispatch keyed by abstract instance id.
    ///
    /// Populated alongside `mono_dispatch` from each `MonoFunction.instance_ids`
    /// in `declare_mono_functions`. When an `ArcInstr::Apply` /
    /// `ArcTerminator::Invoke` carries `mono_instance_id: Some(id)`,
    /// `lookup_mono_dispatch` resolves the call directly via this map — no
    /// argument-type matching, no generic-name lookup. The mangled string
    /// remains owned exclusively by `ori_llvm` (computed in
    /// `mangle_mono_name`); upstream phases only ever produce the abstract
    /// index, satisfying the phase-purity contract for LLVM-specific names.
    pub mono_dispatch_by_id: FxHashMap<MonoInstanceId, Name>,
    /// Known-nounwind user function names: `Invoke` terminators calling these
    /// emit `call` + `br` instead of LLVM `invoke`, eliminating landing pads.
    pub nounwind_functions: FxHashSet<Name>,
    /// Non-capturing lambda names: these are declared with closure-compatible
    /// ABI (`ccc` + phantom `ptr` env param) so `PartialApply` can point
    /// directly at them without generating a `_ori_partial_N` trampoline.
    pub non_capturing_lambdas: FxHashSet<Name>,
    /// Per-lambda capture ownership: which captures are borrowed vs owned.
    ///
    /// When a lambda borrows a capture parameter, the closure's wrapper
    /// function skips `RcInc` for borrowed captures — the lambda body
    /// borrows from the env rather than getting its own reference.
    /// Maps lambda `Name` → ownership of each capture param (indexed by
    /// position in the `PartialApply` args list).
    pub lambda_capture_ownership: FxHashMap<Name, Vec<Ownership>>,
    /// Known-pure function names (no memory effects). These get the LLVM
    /// `memory(none)` attribute, enabling aggressive optimization.
    pub pure_functions: FxHashSet<Name>,
    /// Known read-only function names (reads memory, no writes). These get
    /// the LLVM `memory(read)` attribute. Strictly weaker than `pure_functions`.
    pub readonly_functions: FxHashSet<Name>,
    /// Memoized `ori_arc::type_drop_may_unwind` results keyed by type `Idx`.
    ///
    /// Interior-mutable so the nounwind analysis (`is_arc_function_nounwind`,
    /// `&self`) and the `RcDec` emitter share one cache. A `RcDec` of a type
    /// whose drop transitively runs a user `@drop` may raise a foreign Itanium
    /// exception, so its dec site is may-unwind (needs an `invoke` + cleanup
    /// pad). Stable per compilation — the type pool is frozen.
    pub drop_unwind_memo: std::cell::RefCell<FxHashMap<Idx, bool>>,
}

// Re-export convenience method on ArcIrEmitter for use by submodules
// that need to check boxed enum fields.
impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Whether the position holding `field_type` inside `enum_type` is a boxed
    /// recursive back-edge. Convenience wrapper over [`is_boxed_enum_field`]
    /// using the emitter's pool.
    pub(super) fn is_boxed_enum_field(&self, enum_type: Idx, field_type: Idx) -> bool {
        is_boxed_enum_field(self.pool, enum_type, field_type)
    }
}

#[cfg(test)]
mod tests {
    use super::{intercepted_is_nounwind, MAY_UNWIND_INTERCEPTED_METHODS};

    #[test]
    fn intercepted_is_nounwind_defaults_true_for_unknown_methods() {
        // A random builtin method name not in the may-unwind list should
        // be treated as nounwind (the default behavior for intercepted
        // methods like `map`, `filter`, `len`, `is_empty`).
        assert!(intercepted_is_nounwind("map"));
        assert!(intercepted_is_nounwind("filter"));
        assert!(intercepted_is_nounwind("len"));
        assert!(intercepted_is_nounwind("is_empty"));
        assert!(intercepted_is_nounwind(""));
    }

    #[test]
    fn intercepted_is_nounwind_rejects_may_unwind_methods() {
        // Each entry in MAY_UNWIND_INTERCEPTED_METHODS must be recognized
        // as may-unwind — their builtin emission includes `call ori_panic`
        // on the failing variant, so callers must keep their invoke edges
        // and not be marked nounwind.
        for &name in MAY_UNWIND_INTERCEPTED_METHODS {
            assert!(
                !intercepted_is_nounwind(name),
                "method {name:?} must be classified may-unwind"
            );
        }
    }

    #[test]
    fn may_unwind_list_covers_option_result_panic_methods() {
        // Regression pin: these are the exact method names that the
        // builtins in `option_result_helpers.rs` route through
        // `emit_expect_branch` / `emit_unwrap_branch`. If a new method
        // is added to the dispatch in `option_result.rs`, it must also
        // be added to `MAY_UNWIND_INTERCEPTED_METHODS`.
        for expected in ["unwrap", "unwrap_err", "expect", "expect_err"] {
            assert!(
                MAY_UNWIND_INTERCEPTED_METHODS.contains(&expected),
                "expected method {expected:?} missing from may-unwind list"
            );
        }
    }
}
