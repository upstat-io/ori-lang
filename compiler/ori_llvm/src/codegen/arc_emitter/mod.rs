//! ARC IR → LLVM IR emitter.
//!
//! Translates [`ArcFunction`] basic blocks and instructions directly to LLVM IR,
//! including RC operations (`ori_rc_inc`, `ori_rc_dec`) and structured cleanup
//! via `invoke`/`landingpad`.
//!
//! This is the **sole codegen path** for all Ori functions (JIT and AOT).
//! Every function goes through: `CanExpr → ARC IR → ArcIrEmitter → LLVM IR`.
//!
//! # Architecture
//!
//! ```text
//! CanExpr  →  ori_arc::lower  →  ArcFunction
//!          →  ori_arc pipeline (borrow, RC, reuse, eliminate)
//!          →  ArcIrEmitter    →  LLVM IR  (with RC lifecycle)
//! ```
//!
//! # Submodules
//!
//! - [`builtins`] — builtin method emission (string, list, map, iterator ops)
//! - [`closure_wrappers`] — closure wrapper function generation (`_ori_partial_N` trampolines)
//! - [`closures`] — closure (partial application) emission and environment management
//! - [`construction`] — value construction: structs, enums, lists, maps, sets
//! - [`context`] — shared types: `CodegenContext`, `EmittedValue`, `InvokeMode`, `is_boxed_enum_field`
//! - [`dead_unwind`] — dead unwind block detection (nounwind invoke targets)
//! - [`drop_gen`] — per-type LLVM drop function generation (cached by mangled name)
//! - [`emit_function`] — function-level emission orchestration
//! - [`field_scan`] — field usage scanning for surgical struct loading
//! - [`instr_dispatch`] — per-instruction dispatch (`emit_instr`, `emit_project`)
//! - [`operators`] — binary and unary operator emission (primitive + trait dispatch)
//! - [`rc_helpers`] — RC data pointer extraction and inline enum cleanup
//! - [`rc_ops`] — `ori_rc_inc`/`ori_rc_dec` emission with closure-aware `env_ptr` handling
//! - [`terminators`] — `ArcTerminator` → LLVM control flow emission

mod apply;
mod apply_helpers;
mod apply_protocols;
pub(crate) mod builtins;
mod catch_thunk;
mod catch_thunk_gen;
mod closure_wrappers;
mod closures;
mod construction;
pub(crate) mod context;
mod dead_unwind;
mod drop_enum;
mod drop_gen;
mod element_fn_gen;
mod emit_function;
mod emitter_utils;
mod field_scan;
mod instr_dispatch;
mod operators;
mod rc_buffer_ops;
mod rc_helpers;
mod rc_ops;
mod rc_value_traversal;
mod rpo;
mod terminators;
mod value_emission;
mod variant_construction;

pub use context::CodegenContext;
use context::EmittedValue;

use ori_arc::ir::ArcVarId;
use ori_arc::ArcClassification;
use ori_ir::StringInterner;
use ori_types::{Idx, Pool};
use rustc_hash::{FxHashMap, FxHashSet};

use super::ir_builder::IrBuilder;
use super::type_info::{TypeInfoStore, TypeLayoutResolver};
use super::value_id::{BlockId, FunctionId, TokenId, ValueId};

/// Whether the current funclet is a catch handler or a cleanup block.
///
/// SEH funclets exit differently: catch pads use `catchret` (can branch to
/// normal code), cleanup pads use `cleanupret` (re-raises the exception).
/// Encoding this in the type prevents `br_exiting_catchpad` from accidentally
/// emitting `catchret` for a cleanup pad.
#[derive(Clone, Copy, Debug)]
pub(super) enum FuncletPadKind {
    /// `catchpad` — exits via `catchret` to a trampoline, then branches normally.
    ///
    /// Currently unused: SEH catch blocks use the `ori_try_call` trampoline
    /// instead of LLVM `catchpad`. Retained for match exhaustiveness in
    /// `br_exiting_catchpad` and Jump terminator handlers.
    #[expect(dead_code, reason = "retained for defensive match exhaustiveness")]
    Catch,
    /// `cleanuppad` — exits via `cleanupret` (re-raises exception).
    Cleanup,
}

/// Emits LLVM IR from ARC IR basic blocks.
///
/// Maps `ArcVarId` → `ValueId` and `ArcBlockId` → `BlockId`, walking
/// each block's instructions and terminator to produce LLVM IR.
pub struct ArcIrEmitter<'a, 'scx, 'ctx, 'tcx> {
    /// ID-based LLVM instruction builder.
    builder: &'a mut IrBuilder<'scx, 'ctx>,
    /// Type info cache (`Idx` → `TypeInfo`).
    type_info: &'a TypeInfoStore<'tcx>,
    /// Recursive type layout resolver.
    type_resolver: &'a TypeLayoutResolver<'a, 'scx, 'ctx>,
    /// String interner for `Name` → `&str`.
    interner: &'a StringInterner,
    /// Type pool for structural queries (used by drop function generation).
    pool: &'a Pool,
    /// ARC type classifier for drop function generation.
    classifier: &'a dyn ArcClassification,
    /// Cache: type `Idx` → already-generated drop function `FunctionId`.
    /// Avoids regenerating drop functions for the same type and handles
    /// recursive types (entry inserted before body generation).
    drop_fn_cache: FxHashMap<Idx, FunctionId>,
    /// Cache: element type `Idx` → already-generated element-dec function.
    /// Element-dec functions receive a pointer to an element within a buffer
    /// and decrement that element's RC children (without freeing the element).
    elem_dec_fn_cache: FxHashMap<Idx, FunctionId>,
    /// Cache: element type `Idx` → already-generated element-inc function.
    /// Element-inc functions receive a pointer to an element within a buffer
    /// and increment that element's RC children. Used by COW slow paths.
    elem_inc_fn_cache: FxHashMap<Idx, FunctionId>,
    /// Cache: element type `Idx` → already-generated comparison thunk `FunctionId`.
    /// Compare thunks have signature `fn(*const u8, *const u8) -> i32` (-1/0/1)
    /// and are used by `ori_list_sort_cow`.
    compare_thunk_cache: FxHashMap<Idx, FunctionId>,
    /// Cache: element type `Idx` → already-generated equality thunk `FunctionId`.
    /// Equality thunks have signature `fn(*const u8, *const u8) -> bool` (i1)
    /// and are used by map/set COW operations for key/element comparison.
    eq_thunk_cache: FxHashMap<Idx, FunctionId>,
    /// Cache: element type `Idx` → already-generated hash thunk `FunctionId`.
    /// Hash thunks have signature `fn(*const u8) -> i64` and are used by
    /// hash table map/set operations for key/element hashing.
    hash_thunk_cache: FxHashMap<Idx, FunctionId>,
    /// The LLVM function being compiled.
    current_function: FunctionId,
    /// Shared function-resolution lookup tables.
    ctx: &'a CodegenContext,
    /// Counter for unique `PartialApply` wrapper/drop function names.
    partial_apply_counter: u32,
    /// Counter for unique catch thunk function names (SEH `catch(expr:)`).
    catch_thunk_counter: u32,
    /// ARC variable → typed LLVM value mapping.
    var_map: Vec<Option<EmittedValue>>,
    /// ARC block → LLVM block mapping (`None` for dead unwind blocks).
    block_map: Vec<Option<BlockId>>,
    /// Deferred phi incoming values: `block_index` → `[(param_index, value, source_block)]`.
    /// Collected during terminator emission, applied after all blocks are emitted.
    phi_incoming: Vec<(usize, usize, ValueId, BlockId)>,
    /// Current block index in the ARC IR function (set during emission).
    /// Used by COW emitters to query `CowAnnotations` for the current instruction.
    pub(super) current_block_idx: usize,
    /// Current instruction index within the current block (set during emission).
    pub(super) current_instr_idx: usize,
    /// Active SEH funclet pad token and kind, if inside a `catchpad` or `cleanuppad`.
    /// When `Some`, all runtime calls are emitted with a `"funclet"` operand
    /// bundle so that LLVM's verifier accepts them inside SEH pads.
    /// The `FuncletPadKind` distinguishes catch (exits via `catchret`) from
    /// cleanup (exits via `cleanupret`).
    pub(super) current_funclet_pad: Option<(TokenId, FuncletPadKind)>,
    /// Variables rooted at borrowed parameters (or Let-aliases thereof).
    /// When storing an inline enum value to a boxed field, sub-pointers
    /// must be incremented if the source is borrowed-rooted (the caller
    /// retains a reference, so the boxed store creates an additional one).
    borrowed_rooted_vars: FxHashSet<ArcVarId>,
    /// Borrowed parameter pointer forwarding: maps `ArcVarId` → original LLVM
    /// parameter pointer for variables received as `Reference`/`Indirect` params.
    /// When passing such a variable to another function that also expects a
    /// pointer, we forward the original pointer directly instead of creating
    /// an alloca+store round-trip.
    borrowed_param_ptrs: FxHashMap<ArcVarId, ValueId>,
    /// Decomposed iterator-next results: maps the `Apply(__iter_next)` dst
    /// `ArcVarId` → `(tag: ValueId, scratch_ptr: ValueId, elem_ty: LLVMTypeId)`.
    ///
    /// When `emit_project` encounters a `Project(value, field)` where `value`
    /// is in this map, it returns the tag (field 0) or loads from scratch
    /// (field 1) directly — avoiding the `{i64, T}` wrapper struct that the
    /// legacy `emit_iter_next` built via `insertvalue`.
    iter_next_decomposed: FxHashMap<ArcVarId, (ValueId, ValueId, super::value_id::LLVMTypeId)>,
    /// The function's sret pointer (parameter 0 when return uses `Sret`).
    /// Used by `call_with_sret` to forward the destination directly to a
    /// runtime function, avoiding an intermediate alloca+load+store.
    /// `take()`-semantics: consumed on first use to prevent multiple calls
    /// from writing to the same sret pointer.
    current_sret_ptr: Option<ValueId>,

    /// The `ValueId` of the result loaded from a forwarded sret pointer.
    /// When set, the `Return + Sret` terminator can skip the identity store
    /// (the value is already at the sret destination).
    sret_forwarded_result: Option<ValueId>,
}

impl<'a, 'scx: 'ctx, 'ctx, 'tcx> ArcIrEmitter<'a, 'scx, 'ctx, 'tcx> {
    /// Create a new ARC IR emitter.
    pub fn new(
        builder: &'a mut IrBuilder<'scx, 'ctx>,
        type_info: &'a TypeInfoStore<'tcx>,
        type_resolver: &'a TypeLayoutResolver<'a, 'scx, 'ctx>,
        interner: &'a StringInterner,
        pool: &'a Pool,
        classifier: &'a dyn ArcClassification,
        current_function: FunctionId,
        ctx: &'a CodegenContext,
    ) -> Self {
        Self {
            builder,
            type_info,
            type_resolver,
            interner,
            pool,
            classifier,
            drop_fn_cache: FxHashMap::default(),
            elem_dec_fn_cache: FxHashMap::default(),
            elem_inc_fn_cache: FxHashMap::default(),
            compare_thunk_cache: FxHashMap::default(),
            eq_thunk_cache: FxHashMap::default(),
            hash_thunk_cache: FxHashMap::default(),
            current_function,
            ctx,
            partial_apply_counter: 0,
            catch_thunk_counter: 0,
            var_map: Vec::new(),
            block_map: Vec::new(),
            phi_incoming: Vec::new(),
            current_block_idx: 0,
            current_instr_idx: 0,
            current_funclet_pad: None,
            borrowed_rooted_vars: FxHashSet::default(),
            borrowed_param_ptrs: FxHashMap::default(),
            iter_next_decomposed: FxHashMap::default(),
            current_sret_ptr: None,
            sret_forwarded_result: None,
        }
    }

    /// Check if a variable is rooted at a borrowed parameter.
    ///
    /// Returns `true` if the variable is a borrowed function parameter or
    /// a Let-alias chain leading to one. Used by `emit_variant_via_alloca`
    /// to decide whether storing an inline enum to a boxed field needs a
    /// sub-pointer increment (borrowed → yes, consumed → no).
    pub(super) fn is_var_borrowed_rooted(&self, var: ArcVarId) -> bool {
        self.borrowed_rooted_vars.contains(&var)
    }
}

#[cfg(test)]
mod tests;
