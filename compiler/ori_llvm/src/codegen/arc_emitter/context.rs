//! Shared types for the ARC IR emitter.
//!
//! Contains helper types that bridge ARC IR value representations to LLVM:
//! - [`EmittedValue`] — tagged LLVM value carrying its memory representation
//! - [`InvokeMode`] — call vs invoke dispatch control
//! - [`CodegenContext`] — shared function-resolution lookup tables
//! - [`is_boxed_enum_field`] — recursive enum field detection
//! - [`is_callee_intercepted`] — callee interception shared by analysis and emission

use ori_arc::ir::ValueRepr;
use ori_arc::ownership::Ownership;
use ori_arc::{ClosureAdapterPlan, RetainPlanTable};
use ori_ir::canon::MonoInstanceId;
use ori_ir::Name;
use ori_types::{Idx, Pool};
use rustc_hash::{FxHashMap, FxHashSet};

use super::super::abi::FunctionAbi;
use super::super::value_id::{BlockId, FunctionId, ValueId};
use crate::codegen::arc_emitter::ArcIrEmitter;
use crate::codegen::type_info::TypeInfoStore;

// Recursive field / payload detection

/// Returns whether an aggregate position is heap-boxed as an RC pointer.
///
/// Delegates to the representation boxing oracle used by layout resolution.
pub(super) fn is_boxed_enum_field(pool: &Pool, owner_type: Idx, field_type: Idx) -> bool {
    crate::codegen::type_info::repr_box_oracle::position_is_rc_boxed(pool, owner_type, field_type)
}

// Callee interception detection

/// Builtin callee names whose intercepted emission may unwind for at least
/// one concrete input/result type pair.
///
/// The set includes inline panic paths, scaled unit factories, and iterator
/// operations that may execute a stored user closure. Names are unqualified;
/// [`intercepted_emission_invokes_unwind`] supplies the type-directed verdict.
pub(crate) const MAY_UNWIND_INTERCEPTED_METHODS: &[&str] = &[
    "__cast",
    "abs",
    "byte",
    "int",
    "to_int",
    "unwrap",
    "unwrap_err",
    "expect",
    "expect_err",
    "updated",
    "__index",
    "from_microseconds",
    "from_micros",
    "from_milliseconds",
    "from_millis",
    "from_seconds",
    "from_minutes",
    "from_hours",
    "from_bytes",
    "from_kilobytes",
    "from_kb",
    "from_megabytes",
    "from_mb",
    "from_gigabytes",
    "from_gb",
    "from_terabytes",
    "from_tb",
    "__iter_next",
    "__collect_set",
    "next",
    "next_back",
    "rev",
    "collect",
    "count",
    "any",
    "all",
    "find",
    "for_each",
    "fold",
    "last",
    "rfind",
    "rfold",
    "join",
];

/// Determine whether builtin handlers intercept a callee during emission.
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
/// Declared user functions, monomorphized functions, and runtime functions
/// remain on normal dispatch. Other builtin receivers use interception when
/// no type-qualified method target exists.
pub(crate) fn is_callee_intercepted(
    callee_name: &str,
    callee: Name,
    args: &[ori_arc::ir::ArcVarId],
    func: &ori_arc::ArcFunction,
    ctx: &CodegenContext,
    type_info: &TypeInfoStore<'_>,
) -> bool {
    use super::builtins::prelude::HANDLED_PRELUDE_NAMES;

    if callee_name.starts_with("ori_format_") {
        return true;
    }
    if HANDLED_PRELUDE_NAMES.contains(&callee_name) {
        return true;
    }
    // INVARIANT: Intercepted protocols derive unwind behavior from stored-callback risk.
    if ori_ir::builtin_constants::protocol::ProtocolBuiltin::from_name(callee_name).is_some() {
        return true;
    }
    if ctx.functions.contains_key(&callee) {
        return false;
    }
    if callee_name.starts_with("ori_") || callee_name.starts_with("__") {
        return false;
    }
    // INVARIANT: Generic dispatch precedes builtin receiver heuristics.
    if ctx.mono_dispatch.contains_key(&callee) {
        return false;
    }
    if let Some(&first_arg) = args.first() {
        let receiver_ty = func.var_type(first_arg);
        let info = type_info.get(receiver_ty);
        if info.builtin_type_name().is_some() {
            if let Some(type_name) = ctx.type_idx_to_name.get(&receiver_ty) {
                if !ctx.method_functions.contains_key(&(*type_name, callee)) {
                    return true;
                }
            } else {
                // A missing builtin type name routes the call to its handler.
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
/// The result is true only when the selected receiver-specific emission
/// necessarily calls an unwind-capable runtime function. This prevents both
/// orphan landing pads and invokes targeting omitted cleanup blocks.
pub(crate) fn intercepted_emission_invokes_unwind(
    method_name: &str,
    receiver_tag: Option<ori_types::Tag>,
    result_tag: Option<ori_types::Tag>,
) -> bool {
    use ori_types::Tag;
    if !MAY_UNWIND_INTERCEPTED_METHODS.contains(&method_name) {
        return false;
    }
    match method_name {
        // Inline checked conversions call `ori_panic_cstr` only for these
        // concrete source/result pairs. Keeping the effect type-directed
        // avoids pessimizing lossless scalar conversions.
        "__cast" => {
            matches!(receiver_tag, Some(Tag::Int))
                && matches!(result_tag, Some(Tag::Byte | Tag::Char))
        }
        "int" | "to_int" => matches!(receiver_tag, Some(Tag::Float)),
        "byte" => matches!(receiver_tag, Some(Tag::Int | Tag::Char)),
        "abs" => matches!(receiver_tag, Some(Tag::Int)),
        "updated" => matches!(receiver_tag, Some(Tag::List)),
        "__index" => matches!(receiver_tag, Some(Tag::List | Tag::Str)),
        "unwrap" | "expect" => matches!(receiver_tag, Some(Tag::Option | Tag::Result)),
        "unwrap_err" | "expect_err" => matches!(receiver_tag, Some(Tag::Result)),
        "__iter_next" | "__collect_set" | "next" | "next_back" | "rev" | "collect" | "count"
        | "any" | "all" | "find" | "for_each" | "fold" | "last" | "rfind" | "rfold" | "join" => {
            matches!(receiver_tag, Some(Tag::Iterator | Tag::DoubleEndedIterator))
        }
        // New inventory entries fail closed until they receive a narrower
        // type-directed arm above.
        _ => true,
    }
}

/// Check whether an intercepted callee is guaranteed to be nounwind for the
/// concrete receiver/result types at this call site.
///
/// Called by the nounwind analyzer when [`is_callee_intercepted`] returned
/// `true` to decide whether the intercept may unwind. The default is
/// `true` (nounwind). Candidate names in
/// [`MAY_UNWIND_INTERCEPTED_METHODS`] are then classified by the same
/// type-directed predicate used to retain their LLVM unwind edges.
///
/// Matching is by unqualified method name (the callee `Name` already
/// strips the type prefix, e.g., `expect` not `Option.expect`), because
/// the may-unwind set is keyed on the method surface, not the receiver
/// type — `Option.expect` and `Result.expect` both panic via the same
/// emission helper.
#[must_use]
pub(crate) fn intercepted_is_nounwind(
    callee_name: &str,
    receiver_tag: Option<ori_types::Tag>,
    result_tag: Option<ori_types::Tag>,
) -> bool {
    !intercepted_emission_invokes_unwind(callee_name, receiver_tag, result_tag)
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
}

impl EmittedValue {
    /// Extract the single underlying [`ValueId`].
    ///
    pub(super) fn into_raw(self) -> ValueId {
        match self {
            Self::Immediate(v) | Self::RcPointer(v) | Self::Aggregate(v) => v,
        }
    }

    /// Bridge from an ARC IR [`ValueRepr`] to an emitted value.
    ///
    /// Maps scalar and aggregate representations to their emitted form.
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
/// [`FunctionCompiler`](crate::codegen::function_compiler::FunctionCompiler) to
/// [`ArcIrEmitter`]. Extracting these reduces the
/// emitter constructor from 12 parameters to 7 semantically distinct ones.
#[derive(Debug, Default)]
pub struct CodegenContext {
    /// Declared functions: `Name` → (`FunctionId`, ABI).
    pub functions: FxHashMap<Name, (FunctionId, FunctionAbi)>,
    /// Exact post-AIMS direct-call targets keyed by artifact function name and
    /// stable result register. Populated only from `ExecutableProgram`.
    pub executable_call_targets:
        FxHashMap<(Name, ori_arc::ArcVarId), ori_repr::executable::CallableTarget>,
    /// Artifact function names in stable `FunctionId` order.
    pub executable_function_names: Vec<Name>,
    /// Import-local names in stable artifact `ExternalFunctionId` order.
    pub executable_external_names: Vec<Name>,
    /// Type-qualified method lookup: `(type_name, method_name)` → (`FunctionId`, ABI).
    pub method_functions: FxHashMap<(Name, Name), (FunctionId, FunctionAbi)>,
    /// Closed semantic receiver/method lookup projected from `ExecutableProgram`.
    ///
    /// This table is authoritative whenever executable facts are bound. It also
    /// covers backend-synthesized calls inside compound builtins, which have no
    /// ARC result register through which to consult `executable_call_targets`.
    pub exact_method_functions: FxHashMap<(Idx, Name), (FunctionId, FunctionAbi)>,
    /// Artifact-bound user-drop operations keyed by their exact semantic type.
    /// Production drop emission and nounwind analysis consult only this table;
    /// the general method map is retained for unbound unit-test fixtures.
    pub user_drop_functions: FxHashMap<Idx, (FunctionId, FunctionAbi)>,
    /// Maps receiver type `Idx` → type `Name` for operator trait dispatch.
    pub type_idx_to_name: FxHashMap<Idx, Name>,
    /// Per-instantiation derived-method dispatch: `(concrete_resolved_idx, method_name)` →
    /// (`FunctionId`, ABI). A generic composite (`P3Pair<int,str>`, `Box<Box<int>>`)
    /// emits one derived method per concrete instantiation, each keyed here by the
    /// materialized concrete `Struct`/`Enum` `Idx` (`pool.resolve_fully(Applied)`), so
    /// nested and multi-instantiation dispatch resolves the layout-correct body.
    /// Resolution prefers this map; non-generic types fall back to the
    /// type-name-keyed `method_functions`.
    pub mono_derive_functions: FxHashMap<(Idx, Name), (FunctionId, FunctionAbi)>,
    /// Monomorphized generic dispatch: original name → `[(concrete_param_types, mangled_name)]`.
    ///
    /// When a non-generic function calls a generic one (e.g., `identity(42)`), the ARC IR
    /// Call sites without a `MonoInstanceId` use this index to match argument
    /// types against the mangled LLVM declaration.
    pub mono_dispatch: FxHashMap<Name, Vec<(Vec<Idx>, Name)>>,
    /// Monomorphized generic dispatch keyed by abstract instance id.
    ///
    /// Populated alongside `mono_dispatch` from each mono function identity in
    /// `declare_mono_functions`. When an `ArcInstr::Apply` /
    /// `ArcTerminator::Invoke` carries `mono_instance_id: Some(id)`,
    /// `lookup_mono_dispatch` resolves the call directly via this map — no
    /// argument-type matching, no generic-name lookup. The mangled string
    /// remains owned exclusively by `ori_llvm` (computed in
    /// `mangle_mono_name`); frontend phases produce only the abstract
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
    /// Frozen closure-call adapters keyed by their concrete target.
    /// Populated only from the validated executable artifact.
    pub closure_adapters: FxHashMap<Name, ClosureAdapterPlan>,
    /// Closed backend-neutral ownership topology used by frozen adapter actions.
    pub retain_plans: RetainPlanTable,
    /// Whether this context is bound to a closed executable artifact.
    /// A bound context fails closed on missing closure facts; it never consults
    /// the per-lambda ownership fallback.
    pub executable_facts_bound: bool,
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
#[path = "context_tests.rs"]
mod tests;
