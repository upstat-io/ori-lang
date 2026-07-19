//! Shared state and primitive operations for canonical-expression lowering.

use ori_ir::canon::{CanArena, CanId, CanonResult, MonoConstBinding};
use ori_ir::{Name, StringInterner};
use ori_types::{Idx, Pool, TypeFlags};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::ir::{ArcFunction, ArcValue, ArcVarId, LitValue};

use super::super::scope::ArcScope;
use super::super::{ArcIrBuilder, ArcProblem, VariantCtors};

// Loop context

/// Context for the enclosing loop (used by `break`/`continue`).
#[derive(Debug)]
pub(crate) struct LoopContext {
    /// Target label of this loop (`Name::EMPTY` = unlabeled). A labeled
    /// break/continue walks the loop-context stack for the matching label.
    /// Spec: Clause 16.3.3.
    pub label: Name,
    /// Block to jump to on `break`.
    pub exit_block: crate::ir::ArcBlockId,
    /// Block to jump to on `continue`.
    pub continue_block: crate::ir::ArcBlockId,
    /// Mutable variables in block-parameter order for SSA merge.
    /// MUST be `Vec` (not `HashMap`) — order must match `add_block_param` order.
    /// Each entry is `(name, header_param)` where `header_param` is the SSA
    /// value at loop header entry — used as infallible fallback when
    /// `scope.lookup(name)` fails during break/continue lowering.
    pub mutable_vars: Vec<(Name, crate::ir::ArcVarId)>,
    /// Iterator handle that must be dropped when a labeled transfer abandons
    /// this loop for an outer target. A break targeting this loop does not run
    /// the obligation because its exit block performs the normal drop.
    pub abandon_iter: Option<crate::ir::ArcVarId>,
    /// For-yield specific: when set, break/continue handle list accumulation
    /// and thread the collection phantom parameter.
    pub yield_ctx: Option<ForYieldContext>,
}

/// The comprehension shape lowered by the for-yield strategies.
///
/// Bundles the binding pattern, guard, body, and result type — the four
/// comprehension inputs that travel together through `lower_for_yield` and
/// both its `_option` / `_iterator` strategy methods.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ForYieldShape {
    pub pattern: ori_ir::canon::CanBindingPatternId,
    pub guard: CanId,
    pub body: CanId,
    pub result_ty: Idx,
}

/// The full `CanExpr::For` shape lowered by `lower_for`.
///
/// Bundles the binding pattern, iterable, guard, body, result type, yield
/// flag, and label — the seven for-loop inputs destructured together from the
/// `CanExpr::For` node and threaded into the variant-specific lowering paths.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ForLoop {
    pub pattern: ori_ir::canon::CanBindingPatternId,
    pub iter: CanId,
    pub guard: CanId,
    pub body: CanId,
    pub ty: Idx,
    pub is_yield: bool,
    pub label: Name,
}

/// Context for break/continue inside a for-yield loop.
///
/// Enables `lower_break`/`lower_continue` to push values to the
/// accumulating list and prepend the collection phantom to jump args.
#[derive(Debug)]
pub(crate) struct ForYieldContext {
    /// The `ori_list_new` result pointer — used with `ori_list_push`.
    pub list_ptr: crate::ir::ArcVarId,
    /// Element size literal — passed to `ori_list_push`.
    pub elem_size: crate::ir::ArcVarId,
    /// Interned `"ori_list_push"` name.
    pub list_push_name: Name,
}

// ArcLowerer

/// Expression lowerer that walks the canonical IR and emits ARC IR.
///
/// Borrows the `ArcIrBuilder` and contextual data (arena, canon result,
/// interner, pool) needed to lower each expression variant.
pub(crate) struct ArcLowerer<'a> {
    pub(crate) builder: &'a mut ArcIrBuilder,
    pub(crate) arena: &'a CanArena,
    pub(crate) canon: &'a CanonResult,
    pub(crate) interner: &'a StringInterner,
    pub(crate) pool: &'a Pool,
    pub(crate) scope: ArcScope,
    /// Stack of enclosing loop contexts (innermost at top). A labeled
    /// break/continue walks this top-down for the matching `label`; an
    /// unlabeled signal targets the top (innermost). Spec: Clause 16.3.3.
    pub(crate) loop_ctx_stack: Vec<LoopContext>,
    pub(crate) problems: &'a mut Vec<ArcProblem>,
    pub(crate) lambdas: &'a mut Vec<ArcFunction>,
    /// Resolved `#` (hash length) value for the current index expression.
    ///
    /// Set by `lower_index` before lowering the index sub-expression,
    /// so that `CanExpr::HashLength` resolves to the collection's length.
    /// Mirrors the interpreter's `eval_can_with_hash_length`.
    pub(crate) hash_length: Option<ArcVarId>,
    /// Names freshly `let`-bound in the current block.
    ///
    /// Tracks names introduced by `let` (via `bind_pattern`) to distinguish
    /// shadows from reassignments during block-exit propagation. A `let x = 20`
    /// in an inner block creates a shadow that must NOT propagate to the parent
    /// scope, whereas `x = 20` (reassignment) MUST propagate.
    ///
    /// Saved/restored around each `lower_block` so nesting works correctly.
    pub(crate) block_let_names: FxHashSet<Name>,
    /// The return type of the function under lowering.
    ///
    /// Used by `lower_try()` to construct the early-return `None`/`Err`
    /// with the correct type (must match the function signature, not the
    /// scrutinee's type).
    pub(crate) return_type: Idx,
    /// The name of the function under lowering.
    ///
    /// Used by `lower_exp_recurse()` to emit `Apply @func_name(...)` instead
    /// of a sentinel. This enables TCO detection (which checks
    /// `Apply.func == arc_func.name`) and fixes AOT compilation of
    /// `recurse()` patterns.
    pub(crate) func_name: Name,
    /// Reverse lookup from variant name to enum constructor info.
    ///
    /// Shared by reference from [`lower_function_can`](super::super::lower_function_can).
    /// Used to intercept variant constructor calls and emit `Construct`
    /// instructions instead of function calls.
    pub(crate) variant_ctors: &'a VariantCtors,
    /// Optional type substitution map for monomorphized generic functions.
    ///
    /// Maps generic `Idx` → concrete `Idx`. When `Some`, `expr_type()` applies
    /// the substitution so the ARC lowerer emits type-specific RC operations.
    /// `None` for non-generic functions (zero overhead).
    pub(crate) type_subst: Option<&'a FxHashMap<Idx, Idx>>,
    /// Exact named const substitutions for this monomorphic body.
    pub(crate) const_bindings: Option<&'a [MonoConstBinding]>,
}

// Builder and scope internals have independent diagnostic owners. Report the
// lowerer's active control-flow and accumulated-output coordinates.
impl std::fmt::Debug for ArcLowerer<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ArcLowerer")
            .field("loop_depth", &self.loop_ctx_stack.len())
            .field("problem_count", &self.problems.len())
            .field("lambda_count", &self.lambdas.len())
            .field("hash_length", &self.hash_length)
            .finish_non_exhaustive()
    }
}

impl ArcLowerer<'_> {
    /// Resolve an interned `Name` to its string for diagnostic/tracing output.
    #[inline]
    pub(crate) fn name_str(&self, name: Name) -> &str {
        self.interner.lookup(name)
    }

    /// Get the type of a canonical expression by its ID.
    ///
    /// When `type_subst` is `Some` (monomorphized function), applies the
    /// substitution to return the concrete type instead of the generic one.
    #[inline]
    pub(crate) fn expr_type(&self, id: CanId) -> Idx {
        if !id.is_valid() {
            return Idx::ERROR;
        }
        let ty = self.arena.ty(id);
        let idx = Idx::from_raw(ty.raw());
        self.resolve_body_type(idx)
    }

    /// Apply the type substitution map if present, returning the concrete type.
    ///
    /// For non-generic functions (`type_subst` is `None`), returns `ty` unchanged.
    #[inline]
    pub(crate) fn resolve_body_type(&self, ty: Idx) -> Idx {
        match self.type_subst {
            Some(map) => {
                if let Some(resolved) = map.get(&ty).copied() {
                    resolved
                } else {
                    // A generic body composite interned after call-site
                    // recording can miss the exact-Idx map and leave a rigid
                    // leaf for codegen; diagnose that residual here.
                    if self
                        .pool
                        .flags(ty)
                        .intersects(TypeFlags::HAS_RIGID_VAR | TypeFlags::HAS_VAR)
                    {
                        tracing::debug!(
                            ty = ?ty,
                            tag = ?self.pool.tag(ty),
                            map_len = map.len(),
                            "resolve_body_type miss on generic body type"
                        );
                    }
                    ty
                }
            }
            None => ty,
        }
    }

    /// Emit a unit literal.
    pub(crate) fn emit_unit(&mut self) -> ArcVarId {
        self.builder
            .emit_let(Idx::UNIT, ArcValue::Literal(LitValue::Unit), None)
    }
}
