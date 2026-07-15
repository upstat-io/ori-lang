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
//! CanExpr → ori_arc::lower → ArcFunction
//!          → ori_arc pipeline (borrow, RC, reuse, eliminate)
//!          → ArcIrEmitter → LLVM IR (with RC lifecycle)
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
//! - [`narrowing_codegen`] — integer and float narrowing at struct/collection/local boundaries
//! - [`operators`] — binary and unary operator emission (primitive + trait dispatch)
//! - [`rc_helpers`] — RC data pointer extraction and inline enum cleanup
//! - [`rc_ops`] — `ori_rc_inc`/`ori_rc_dec` emission with closure-aware `env_ptr` handling
//! - [`terminators`] — `ArcTerminator` → LLVM control flow emission

mod apply;
mod apply_casts;
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
mod emit_function_setup;
mod emitter_utils;
mod field_scan;
mod field_walk;
mod instr_dispatch;
mod narrowing_codegen;
mod narrowing_local;
mod operators;
mod rc_buffer_ops;
mod rc_helpers;
mod rc_ops;
mod rc_value_traversal;
mod rpo;
pub(super) mod tag_access;
mod tagless_enum;
mod terminators;
mod value_emission;
mod variant_construction;

pub use context::CodegenContext;
use context::EmittedValue;
pub(crate) use narrowing_codegen::narrowed_collection_element_width;

use crate::aot::debug::DebugContext;
use ori_arc::ir::ArcVarId;
use ori_arc::ArcClassification;
use ori_arc::MemoryContract;
use ori_ir::{Name, StringInterner};
use ori_types::{Idx, Pool, Tag};
use rustc_hash::{FxHashMap, FxHashSet};

use super::ir_builder::IrBuilder;
use super::type_info::{TypeInfoStore, TypeLayoutResolver};
use super::value_id::{BlockId, FunctionId, TokenId, ValueId};

/// Pre-interned `Name`s for the list runtime symbols the emitter
/// special-cases at call-emission sites (per interning discipline: identity
/// comparison goes through `Name`, pre-interned once — never per-call
/// string comparison).
#[derive(Clone, Copy, Debug)]
pub(super) struct ListRtNames {
    /// `ori_list_push` — element arg coerced to ptr; for-yield elem-size override.
    pub push: Name,
    /// `ori_list_new` — for-yield elem-size override.
    pub new: Name,
    /// `ori_list_take` — real runtime fn with special sret handling (non-protocol).
    pub take: Name,
    /// `ori_list_slice_drop` — list rest-pattern expansion (non-protocol).
    pub slice_drop: Name,
}

impl ListRtNames {
    fn from_interner(interner: &StringInterner) -> Self {
        Self {
            push: interner.intern("ori_list_push"),
            new: interner.intern("ori_list_new"),
            take: interner.intern("ori_list_take"),
            slice_drop: interner.intern("ori_list_slice_drop"),
        }
    }
}

/// Pre-interned `Name`s for the `ori_format_*` intercepts (per interning
/// discipline: identity comparison goes through `Name`, pre-interned once).
/// [`FormatRtNames::lookup`] is the single dispatch point — it carries the
/// runtime symbol AND whether the formatted value itself is a string struct
/// needing ptr coercion, so adding a format runtime function extends this
/// registry rather than a free-floating literal match.
#[derive(Clone, Copy, Debug)]
pub(super) struct FormatRtNames {
    int: Name,
    float: Name,
    str: Name,
    bool: Name,
    char: Name,
}

impl FormatRtNames {
    fn from_interner(interner: &StringInterner) -> Self {
        Self {
            int: interner.intern("ori_format_int"),
            float: interner.intern("ori_format_float"),
            str: interner.intern("ori_format_str"),
            bool: interner.intern("ori_format_bool"),
            char: interner.intern("ori_format_char"),
        }
    }

    /// Resolve a callee `Name` to `(runtime_symbol, value_is_str)`.
    pub(super) fn lookup(&self, callee: Name) -> Option<(&'static str, bool)> {
        if callee == self.int {
            Some(("ori_format_int", false))
        } else if callee == self.float {
            Some(("ori_format_float", false))
        } else if callee == self.str {
            Some(("ori_format_str", true))
        } else if callee == self.bool {
            Some(("ori_format_bool", false))
        } else if callee == self.char {
            Some(("ori_format_char", false))
        } else {
            None
        }
    }
}

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
    type_resolver: &'a TypeLayoutResolver<'a, 'ctx, 'tcx>,
    /// String interner for `Name` → `&str`.
    interner: &'a StringInterner,
    /// Pre-interned list-runtime-symbol names for emission-site identity checks.
    list_rt_names: ListRtNames,
    format_rt_names: FormatRtNames,
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
    /// Debug info context (None for JIT or when debug info is disabled).
    debug_context: Option<&'a DebugContext<'ctx>>,
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
    /// Live ARC unwind block for the in-flight INTERCEPTED may-unwind builtin
    /// emission (e.g. list `updated` — `ori_list_updated_cow` panics on OOB).
    /// When `Some`, `emit_rt_call` routes calls to non-nounwind runtime
    /// functions through `invoke` with this block as the unwind edge so the
    /// ARC cleanup decs run on the panic path. Set/cleared by `emit_invoke`
    /// around `try_emit_builtin_method`; Itanium-only (`None` inside SEH
    /// funclet pads).
    pub(super) intercepted_unwind: Option<crate::codegen::value_id::BlockId>,
    /// Maps each same-frame catch-scoped inline checked-op result `ArcVarId`
    /// (from `ArcFunction::catch_scoped_checked_ops`) to the LLVM landing-pad
    /// `BlockId` materialized for its catch handler. Populated in
    /// `emit_function` after the handler blocks are created; consulted in
    /// `emit_instr` to thread `IrBuilder::catch_unwind_target` PER-DISPATCH
    /// around each catch-scoped checked op. Empty when the function
    /// has no same-frame checked-op catch.
    pub(super) same_frame_catch_landing_pads:
        FxHashMap<ArcVarId, crate::codegen::value_id::BlockId>,
    /// Variables rooted at borrowed parameters (or Let-aliases thereof).
    /// When storing an inline enum value to a boxed field, sub-pointers
    /// must be incremented if the source is borrowed-rooted (the caller
    /// retains a reference, so the boxed store creates an additional one).
    borrowed_rooted_vars: FxHashSet<ArcVarId>,
    /// The current function's interprocedural `MemoryContract`, when available.
    /// Read by `compute_borrowed_rooted_vars` to seed `iter_consume_owns_rooted_vars`.
    func_contract: Option<&'a MemoryContract>,
    /// Variables rooted at a param whose `ParamContract` proves it is iter-consume
    /// transferred AND NOT transferred through the function's own Return
    /// (`iter_consumes && !transfers_through_return`). For such a `.iter()`
    /// receiver the iterator OWNS the backing buffer (the RL-2 inward transfer
    /// gave it ownership, the caller relinquished its dec, and the param is not
    /// also returned) → `owns_data=true` even though it is borrowed-rooted.
    iter_consume_owns_rooted_vars: FxHashSet<ArcVarId>,
    /// Borrowed parameter pointer forwarding: maps `ArcVarId` → original LLVM
    /// parameter pointer for variables received as `Reference`/`Indirect` params.
    /// When passing such a variable to another function that also expects a
    /// pointer, we forward the original pointer directly instead of creating
    /// an alloca+store round-trip.
    borrowed_param_ptrs: FxHashMap<ArcVarId, ValueId>,
    /// Borrowed `Reference`/`Indirect` parameters whose entry-block aggregate
    /// load was elided (bound to a zero placeholder) because every use forwards
    /// the source pointer. A `Direct` (by-value) call argument cannot forward a
    /// pointer — it must materialize the aggregate by loading from
    /// `borrowed_param_ptrs`, so the Direct passing path consults this set to
    /// reload only the elided params.
    pointer_only_params: FxHashSet<ArcVarId>,
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
    /// `take`-semantics: consumed on first use to prevent multiple calls
    /// from writing to the same sret pointer.
    current_sret_ptr: Option<ValueId>,

    /// The `ValueId` of the result loaded from a forwarded sret pointer.
    /// When set, the `Return + Sret` terminator can skip the identity store
    /// (the value is already at the sret destination).
    sret_forwarded_result: Option<ValueId>,

    /// Per-function narrowed local variable widths.
    ///
    /// Maps `ArcVarId` → `IntWidth` for local `int` variables whose value
    /// range fits in a narrower integer type. Function parameters are excluded
    /// (no visibility info in ARC IR — conservative safety).
    ///
    /// When a variable is in this map:
    /// - `def_var_repr` inserts `trunc i64 %val to i<width>` at definition
    /// - `var` inserts `sext i<width> %val to i64` at each use
    /// - Phi nodes use the narrow type
    narrowed_vars: FxHashMap<ArcVarId, ori_repr::IntWidth>,

    /// Repr plan for collection element narrowing.
    ///
    /// When present, collection construction and element access paths consult
    /// this plan for narrowed element types (e.g., `[int]` with elements in
    /// `[-128, 127]` uses `i8` element storage instead of `i64`).
    repr_plan: Option<&'a ori_repr::ReprPlan>,

    /// `elem_size` `ArcVarId`s for int-element for-yield loops.
    ///
    /// Pre-scanned from `ori_list_push` calls: when a push's element arg
    /// has type `Tag::Int`, the `elem_size` arg (shared between `ori_list_new`
    /// and `ori_list_push` in the same for-yield) is added to this set.
    /// The for-yield narrowing override only fires for variables in this set,
    /// preventing non-int accumulators (e.g., `[str]`) from being corrupted.
    for_yield_int_elem_sizes: FxHashSet<ArcVarId>,

    /// Elem-size `ArcVarId` → element `Idx` for all for-yield loops.
    ///
    /// Used to override ARC-emitted `pool_type_store_size` values with
    /// the LLVM struct store size (which accounts for field reordering).
    /// Without this, reordered structs/tuples get a size mismatch between
    /// the runtime list stride and LLVM GEP stride.
    for_yield_elem_size_types: FxHashMap<ArcVarId, Idx>,

    /// Niche-encoded enum tag tracking.
    ///
    /// When `Project { field: 0 }` extracts a tag from a niche-encoded enum,
    /// the destination variable holds the raw niche field value (not a logical
    /// variant index). This map records `dst_var → source_enum_type_idx` so
    /// that `Switch` can emit niche-aware comparisons instead of a standard
    /// LLVM switch instruction.
    niche_scrutinees: FxHashMap<ArcVarId, Idx>,

    /// Whether ARC/LLVM IR verification is enabled (`ORI_VERIFY_ARC=1`).
    /// Set via [`set_verify_arc`] after construction; defaults to `false`.
    /// SSOT: plumbed from `FunctionCompiler::verify_arc` — do NOT re-read env var.
    verify_arc: bool,
}

impl<'a, 'scx: 'ctx, 'ctx, 'tcx> ArcIrEmitter<'a, 'scx, 'ctx, 'tcx> {
    /// Create a new ARC IR emitter.
    pub fn new(
        builder: &'a mut IrBuilder<'scx, 'ctx>,
        type_info: &'a TypeInfoStore<'tcx>,
        type_resolver: &'a TypeLayoutResolver<'a, 'ctx, 'tcx>,
        interner: &'a StringInterner,
        pool: &'a Pool,
        classifier: &'a dyn ArcClassification,
        current_function: FunctionId,
        ctx: &'a CodegenContext,
        debug_context: Option<&'a DebugContext<'ctx>>,
    ) -> Self {
        Self {
            builder,
            type_info,
            type_resolver,
            list_rt_names: ListRtNames::from_interner(interner),
            format_rt_names: FormatRtNames::from_interner(interner),
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
            debug_context,
            partial_apply_counter: 0,
            catch_thunk_counter: 0,
            var_map: Vec::new(),
            block_map: Vec::new(),
            phi_incoming: Vec::new(),
            current_block_idx: 0,
            current_instr_idx: 0,
            current_funclet_pad: None,
            intercepted_unwind: None,
            same_frame_catch_landing_pads: FxHashMap::default(),
            borrowed_rooted_vars: FxHashSet::default(),
            func_contract: None,
            iter_consume_owns_rooted_vars: FxHashSet::default(),
            borrowed_param_ptrs: FxHashMap::default(),
            pointer_only_params: FxHashSet::default(),
            iter_next_decomposed: FxHashMap::default(),
            current_sret_ptr: None,
            sret_forwarded_result: None,
            narrowed_vars: FxHashMap::default(),
            repr_plan: type_resolver.repr_plan(),
            for_yield_int_elem_sizes: FxHashSet::default(),
            for_yield_elem_size_types: FxHashMap::default(),
            niche_scrutinees: FxHashMap::default(),
            verify_arc: false,
        }
    }

    /// Set whether ARC/LLVM IR verification is enabled.
    ///
    /// Called by `FunctionCompiler` after construction to propagate its
    /// `verify_arc` flag — eliminates env var re-reads in submodules.
    pub fn set_verify_arc(&mut self, enable: bool) {
        self.verify_arc = enable;
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

    /// Supply the current function's interprocedural `MemoryContract` before
    /// `emit_function`, so `compute_borrowed_rooted_vars` can seed the
    /// iter-consume-owns set from its `ParamContract`s. Defaults to `None`
    /// (no flips) when not set — lambdas and tests leave it unset.
    pub(super) fn set_func_contract(&mut self, contract: Option<&'a MemoryContract>) {
        self.func_contract = contract;
    }

    /// Whether the `.iter()` receiver `var` roots to a param proven
    /// iter-consume-transferred AND NOT transferred through the function's own
    /// Return (`iter_consumes && !transfers_through_return`) — the RL-2 inward
    /// transfer that makes the iterator the buffer's owner (`owns_data=true`)
    /// even though the receiver is borrowed-rooted. The genuinely-borrowed
    /// receiver (flatten inner `sub.iter()`) and the iter-consume+return overlap
    /// (`@f(x) = { x.iter().count(); x }`, where the buffer escapes via Return)
    /// are both absent here → `false`.
    pub(super) fn iter_receiver_owns_via_contract(&self, var: ArcVarId) -> bool {
        self.iter_consume_owns_rooted_vars.contains(&var)
    }

    // Struct layout remapping

    /// Translate a declaration-order field index to the memory-order index
    /// for the LLVM struct type.
    ///
    /// After field reordering, `StructRepr.fields` are sorted by alignment
    /// (memory order), but ARC IR uses declaration-order indices. This helper
    /// bridges the two: if the type has a `StructRepr` in the `ReprPlan`, look
    /// up the memory position; otherwise return the original index unchanged.
    ///
    /// Compare a niche field value against the niche sentinel.
    ///
    /// Returns an `i1` that is `true` when the value IS the niche (e.g., None).
    /// Handles both integer niche fields (`icmp eq`) and pointer niche fields
    /// (`ptrtoint` + `icmp eq`, using the established null-check pattern).
    pub(super) fn niche_is_sentinel(
        &mut self,
        field_val: super::value_id::ValueId,
        niche_value: u64,
        name: &str,
    ) -> super::value_id::ValueId {
        if self.builder.is_pointer_value(field_val) {
            let i64_ty = self.builder.i64_type();
            let as_int = self.builder.ptr_to_int(field_val, i64_ty, "niche.p2i");
            let niche_const = self.builder.const_i64(niche_value as i64);
            self.builder.icmp_eq(as_int, niche_const, name)
        } else {
            let niche_const = self.builder.const_int_matching(field_val, niche_value);
            self.builder.icmp_eq(field_val, niche_const, name)
        }
    }

    // Tagged-pointer encoder/decoder helpers.
    //
    // A tagged-pointer enum value is a single `i64` slot containing
    // `(payload_ptr | variant_tag)`. The low 3 bits hold the variant
    // discriminant (0..7); the high 61 bits hold either an 8-byte-aligned
    // heap pointer or zero (for unit variants).
    //
    // These helpers mirror `niche_is_sentinel`'s pattern: pure LLVM IR
    // emission, no `EnumRepr`/`TagEncoding` dependency. Callers (Construct,
    // Project, Switch, RcInc/Dec, Drop) decide WHEN to use them based on
    // `get_tagged_ptr_encoding`.
    //
    // All consumers flow through these three primitives — never re-derive
    // the masks at call sites.

    /// Encode a payload pointer with a variant tag into the i64 slot.
    ///
    /// `result = (ptr_as_int & TAGGED_PTR_PTR_MASK) | tag`
    ///
    /// In debug builds, asserts that the low 3 bits of `payload_ptr` are
    /// zero (i.e., the pointer is 8-byte aligned). The mask is applied
    /// unconditionally so the encoding is robust to slightly misaligned
    /// inputs in release builds, but the assert catches alignment bugs
    /// at the source.
    ///
    /// `payload_ptr` may be either an LLVM pointer or an integer that
    /// already represents a pointer (e.g., from a prior `ptrtoint`).
    /// Unit variants pass an integer zero (no payload to encode).
    #[allow(
        dead_code,
        reason = "tagged-pointer encoding primitive — not yet wired into codegen consumers"
    )]
    pub(super) fn tagged_ptr_encode(
        &mut self,
        payload_ptr: super::value_id::ValueId,
        variant_tag: u32,
        name: &str,
    ) -> super::value_id::ValueId {
        let i64_ty = self.builder.i64_type();
        let as_int = if self.builder.is_pointer_value(payload_ptr) {
            self.builder
                .ptr_to_int(payload_ptr, i64_ty, &format!("{name}.p2i"))
        } else {
            payload_ptr
        };
        let ptr_mask = self
            .builder
            .const_i64(tag_access::TagEncoding::TAGGED_PTR_PTR_MASK as i64);
        let cleared = self
            .builder
            .and(as_int, ptr_mask, &format!("{name}.cleared"));
        let tag_const = self.builder.const_i64(i64::from(variant_tag));
        self.builder.or(cleared, tag_const, name)
    }

    /// Extract the variant discriminant (low 3 bits) from a tagged-pointer
    /// encoded value.
    ///
    /// `result = encoded & TAGGED_PTR_TAG_MASK`
    ///
    /// The result is an `i64` in `[0, 7]`. Switch terminators that need a
    /// narrower type (e.g., `i8`) should `trunc` the result themselves —
    /// keeping the helper width-agnostic avoids hardcoding tag widths here.
    ///
    /// Accepts either an i64 value or an LLVM pointer. Pointers are
    /// converted via `ptrtoint` so the helper is robust to either form
    /// (e.g., the encoded value loaded from an `i64` slot vs. carried in
    /// an `RcPointer` value through ARC IR).
    #[allow(
        dead_code,
        reason = "tagged-pointer encoding primitive — not yet wired into codegen consumers"
    )]
    pub(super) fn tagged_ptr_decode_tag(
        &mut self,
        encoded: super::value_id::ValueId,
        name: &str,
    ) -> super::value_id::ValueId {
        let i64_ty = self.builder.i64_type();
        let as_int = if self.builder.is_pointer_value(encoded) {
            self.builder
                .ptr_to_int(encoded, i64_ty, &format!("{name}.p2i"))
        } else {
            encoded
        };
        let tag_mask = self
            .builder
            .const_i64(tag_access::TagEncoding::TAGGED_PTR_TAG_MASK as i64);
        self.builder.and(as_int, tag_mask, name)
    }

    /// Extract the payload pointer (high 61 bits) from a tagged-pointer
    /// encoded value.
    ///
    /// `result = (encoded & TAGGED_PTR_PTR_MASK) as ptr`
    ///
    /// Returns an LLVM pointer (via `inttoptr`). For unit variants the
    /// resulting pointer is null — callers MUST guard pointer dereferences
    /// behind a tag check (`tagged_ptr_decode_tag` + variant predicate).
    ///
    /// Accepts either an i64 value or an LLVM pointer (converted via
    /// `ptrtoint` first), matching `tagged_ptr_decode_tag`.
    #[allow(
        dead_code,
        reason = "tagged-pointer encoding primitive — not yet wired into codegen consumers"
    )]
    pub(super) fn tagged_ptr_decode_ptr(
        &mut self,
        encoded: super::value_id::ValueId,
        name: &str,
    ) -> super::value_id::ValueId {
        let i64_ty = self.builder.i64_type();
        let as_int = if self.builder.is_pointer_value(encoded) {
            self.builder
                .ptr_to_int(encoded, i64_ty, &format!("{name}.p2i"))
        } else {
            encoded
        };
        let ptr_mask = self
            .builder
            .const_i64(tag_access::TagEncoding::TAGGED_PTR_PTR_MASK as i64);
        let cleared = self
            .builder
            .and(as_int, ptr_mask, &format!("{name}.cleared"));
        self.builder.int_to_ptr(cleared, name)
    }

    /// Get the `TagEncoding` for an enum type, if it uses niche encoding.
    ///
    /// Returns `Some(encoding)` ONLY for `EnumTag::Niche`. Tagless
    /// (`EnumTag::None`) is dispatched separately via [`get_tagless_encoding`]
    /// (struct-like, no niche field) and tagged-pointer via
    /// [`get_tagged_ptr_encoding`]; explicit tags (the common case) return
    /// `None` so callers fall through to the standard tag-switch path.
    ///
    /// Falls back to on-the-fly canonical computation for types with variable
    /// residue (e.g., `Option<Var(T→str)>`) that weren't in the `ReprPlan`.
    pub(super) fn get_niche_encoding(&self, ty: Idx) -> Option<tag_access::TagEncoding> {
        let enum_repr = self.enum_repr_for(ty)?;
        match &enum_repr.tag {
            ori_repr::EnumTag::Niche { .. } => {
                Some(tag_access::TagEncoding::from_enum_repr(&enum_repr))
            }
            // Niche-only. `None` (tagless), `TaggedPtr`, and `Explicit`
            // are each dispatched by their own query — niche-specific
            // consumers (which `.unwrap` the niche field) must not see
            // a tagless or tagged-ptr encoding.
            ori_repr::EnumTag::Explicit { .. }
            | ori_repr::EnumTag::TaggedPtr
            | ori_repr::EnumTag::None => None,
        }
    }

    /// Resolve the `EnumRepr` for `ty` via the `ReprPlan` SSOT ladder
    /// (`ReprPlan::enum_repr_with_fallback` — plan-first, canonical
    /// recomputation for variable-residue enum shapes).
    ///
    /// Shared by [`Self::get_niche_encoding`], [`Self::is_tagless_enum`], and
    /// [`Self::get_tagged_ptr_encoding`] — never re-derive it at call sites.
    fn enum_repr_for(&self, ty: Idx) -> Option<std::borrow::Cow<'_, ori_repr::EnumRepr>> {
        self.repr_plan?.enum_repr_with_fallback(self.pool, ty)
    }

    /// Whether `ty` is a tagless single-variant enum (`EnumTag::None`).
    ///
    /// A tagless enum has no discriminant and no niche field — its LLVM type is
    /// a plain struct of the single variant's non-void payload fields (see
    /// `resolve_enum_tagless`). Construct / Project / drop / RC consumers route
    /// the tagless case through their struct-shaped paths (direct field GEP,
    /// recursive-field boxing) rather than the niche or explicit-tag paths.
    pub(super) fn is_tagless_enum(&self, ty: Idx) -> bool {
        self.enum_repr_for(ty)
            .is_some_and(|enum_repr| matches!(enum_repr.tag, ori_repr::EnumTag::None))
    }

    /// Look up the tagged-pointer encoding for an enum type, if present.
    ///
    /// Mirrors [`get_niche_encoding`] but returns `Some` only for
    /// `EnumTag::TaggedPtr`. Used by codegen consumers to dispatch
    /// to tagged-pointer encode/decode paths instead of struct-based GEP.
    ///
    /// Returns `None` when:
    /// - The type has no `ReprPlan` entry (variable residue).
    /// - The type is not an enum.
    /// - The enum uses `Explicit`, `Niche`, or `None` tagging.
    #[allow(
        dead_code,
        reason = "tagged-pointer encoding primitive — not yet wired into codegen consumers"
    )]
    pub(super) fn get_tagged_ptr_encoding(&self, ty: Idx) -> Option<tag_access::TagEncoding> {
        let enum_repr = self.enum_repr_for(ty)?;
        match &enum_repr.tag {
            ori_repr::EnumTag::TaggedPtr => {
                Some(tag_access::TagEncoding::from_enum_repr(&enum_repr))
            }
            ori_repr::EnumTag::Explicit { .. }
            | ori_repr::EnumTag::Niche { .. }
            | ori_repr::EnumTag::None => None,
        }
    }

    /// Only applies to user structs/tuples — enum payloads, closure envs,
    /// and collection internals are not subject to reordering.
    pub(super) fn remap_struct_field(&self, ty: Idx, decl_field: u32) -> u32 {
        let Some(plan) = self.repr_plan else {
            return decl_field;
        };
        let resolved = self.pool.resolve_fully(ty);
        let tag = self.pool.tag(resolved);
        if tag != Tag::Struct && tag != Tag::Tuple {
            return decl_field;
        }
        let Some(repr) = plan.get_repr(resolved) else {
            return decl_field;
        };
        match repr {
            ori_repr::MachineRepr::Struct(s) => {
                s.memory_index(decl_field).map_or(decl_field, |i| {
                    #[expect(
                        clippy::cast_possible_truncation,
                        reason = "struct fields always < u32::MAX"
                    )]
                    let idx = i as u32;
                    idx
                })
            }
            ori_repr::MachineRepr::Tuple(t) => t.memory_index(decl_field).map_or(decl_field, |i| {
                #[expect(
                    clippy::cast_possible_truncation,
                    reason = "tuple elements always < u32::MAX"
                )]
                let idx = i as u32;
                idx
            }),
            _ => decl_field,
        }
    }

    /// Reorder args from declaration order to memory order for struct
    /// construction. Returns a new vec with args in LLVM struct field order.
    ///
    /// If the type has no `StructRepr` in the `ReprPlan` (or no reordering),
    /// returns the args unchanged.
    pub(super) fn reorder_args_to_memory_order(&self, args: &[ValueId], ty: Idx) -> Vec<ValueId> {
        let Some(plan) = self.repr_plan else {
            return args.to_vec();
        };
        let resolved = self.pool.resolve_fully(ty);
        let tag = self.pool.tag(resolved);
        if tag != Tag::Struct && tag != Tag::Tuple {
            return args.to_vec();
        }
        let Some(repr) = plan.get_repr(resolved) else {
            return args.to_vec();
        };
        let fields = match repr {
            ori_repr::MachineRepr::Struct(s) => &s.fields[..],
            ori_repr::MachineRepr::Tuple(t) => &t.elements[..],
            _ => return args.to_vec(),
        };
        // fields is in memory order; fields[mem_pos].original_index is the
        // declaration-order index. Build a vec where result[mem_pos] =
        // args[original_index].
        fields
            .iter()
            .map(|f| {
                args.get(f.original_index as usize)
                    .copied()
                    .unwrap_or(args[0]) // defensive — should never happen
            })
            .collect()
    }
}

#[cfg(test)]
mod test_utils;

#[cfg(test)]
mod tests;
