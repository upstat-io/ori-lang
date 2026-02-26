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
//! - [`drop_gen`] — per-type LLVM drop function generation (cached by mangled name)
//! - [`rc_ops`] — `ori_rc_inc`/`ori_rc_dec` emission with closure-aware `env_ptr` handling

mod builtins;
mod drop_gen;
mod rc_ops;

pub use builtins::borrowing_builtin_names;

use ori_arc::ir::{
    ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, CtorKind, LitValue, PrimOp, ValueRepr,
};
use ori_arc::ArcClassification;
use ori_ir::{BinaryOp, Name, StringInterner, UnaryOp};
use ori_types::{Idx, Pool, Tag};
use rustc_hash::{FxHashMap, FxHashSet};

use super::abi::{FunctionAbi, ParamPassing, ReturnPassing};
use super::ir_builder::IrBuilder;
use super::type_info::{TypeInfoStore, TypeLayoutResolver};
use super::value_id::{BlockId, FunctionId, LLVMTypeId, ValueId};

// ---------------------------------------------------------------------------
// Recursive enum detection
// ---------------------------------------------------------------------------

/// Check if a field type creates a direct recursive reference within an enum.
///
/// Returns `true` when the resolved field type is the same Pool index as the
/// resolved enum type — meaning the field must be RC-boxed when stored in the
/// enum payload. Layout: stored as an 8-byte RC pointer instead of inline.
///
/// Only handles direct self-recursion (e.g., `type Tree = Node(Tree, Tree)`).
/// Mutual recursion (`type A = X(B)`, `type B = Y(A)`) is not yet supported.
fn is_boxed_enum_field(pool: &Pool, enum_type: Idx, field_type: Idx) -> bool {
    let enum_resolved = pool.resolve_fully(enum_type);
    let field_resolved = pool.resolve_fully(field_type);
    pool.tag(enum_resolved) == Tag::Enum && enum_resolved == field_resolved
}

// ---------------------------------------------------------------------------
// EmittedValue
// ---------------------------------------------------------------------------

/// Tagged LLVM value carrying its memory representation.
///
/// Wraps [`ValueId`] with variant information derived from the ARC IR's
/// [`ValueRepr`]. This prevents the "did I load this already?" and
/// "is this a pointer or a scalar?" class of bugs by making the value's
/// representation explicit at the type level.
///
/// Inspired by Rust's `OperandValue` in `rustc_codegen_llvm`.
#[derive(Clone, Copy, Debug)]
enum EmittedValue {
    /// Register scalar: i64, f64, i1, i8, i32.
    Immediate(ValueId),
    /// Pointer to heap-allocated RC'd memory (list, map, set, etc.).
    RcPointer(ValueId),
    /// Stack aggregate: struct, tuple, enum by value, fat value (str, closure).
    Aggregate(ValueId),
    /// Two-word split: {first, second} — str={len,ptr}, closure={fn,env}.
    /// The `second` component is typically the RC-managed pointer.
    /// Used by Section 01.3 when RC operations need direct component access.
    #[allow(dead_code, reason = "reserved for Section 01.3 RcStrategy split")]
    Pair { first: ValueId, second: ValueId },
    /// No runtime representation (unit, never).
    /// Used when ZST values are tracked through the pipeline.
    #[allow(dead_code, reason = "reserved for Section 01.3 ZST propagation")]
    ZeroSized,
}

impl EmittedValue {
    /// Extract the single underlying [`ValueId`].
    ///
    /// # Panics
    /// Panics on `Pair` (two values) and `ZeroSized` (no value).
    /// For those variants, destructure the enum directly.
    fn into_raw(self) -> ValueId {
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
    #[allow(dead_code, reason = "used by Section 01.3 RC strategy dispatch")]
    fn rc_data_ptr(self) -> Option<ValueId> {
        match self {
            Self::RcPointer(v) => Some(v),
            Self::Pair { second, .. } => Some(second),
            _ => None,
        }
    }

    /// True if this value contains a reference-counted component.
    #[allow(dead_code, reason = "used by Section 01.3 RC strategy dispatch")]
    fn is_rc_managed(self) -> bool {
        matches!(self, Self::RcPointer(_) | Self::Pair { .. })
    }

    /// Bridge from an ARC IR [`ValueRepr`] to an emitted value.
    ///
    /// Maps single-valued representations directly. `FatValue` is stored
    /// as `Aggregate` (the two components remain packed in a single LLVM
    /// struct value); use `Pair` only when the components are split.
    fn from_repr(repr: ValueRepr, value: ValueId) -> Self {
        match repr {
            ValueRepr::Scalar => Self::Immediate(value),
            ValueRepr::RcPointer => Self::RcPointer(value),
            ValueRepr::Aggregate | ValueRepr::FatValue => Self::Aggregate(value),
        }
    }
}

// ---------------------------------------------------------------------------
// InvokeMode
// ---------------------------------------------------------------------------

/// Controls whether a function call emits LLVM `invoke` (with unwind) or `call` + `br`.
///
/// Eliminates the boolean flag + dead `unwind_block` parameter pattern:
/// - `Invoke { unwind }` carries the unwind block only when needed
/// - `Call { normal }` makes it clear no unwind handling is needed
#[derive(Clone, Copy, Debug)]
enum InvokeMode {
    /// Emit LLVM `invoke` with both normal and unwind continuations.
    Invoke { normal: BlockId, unwind: BlockId },
    /// Emit LLVM `call` + unconditional `br` to normal block (nounwind callee).
    Call { normal: BlockId },
}

impl InvokeMode {
    /// The normal continuation block (used by both variants).
    fn normal_block(self) -> BlockId {
        match self {
            Self::Invoke { normal, .. } | Self::Call { normal } => normal,
        }
    }
}

// ---------------------------------------------------------------------------
// CodegenContext
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// ArcIrEmitter
// ---------------------------------------------------------------------------

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
    /// The LLVM function being compiled.
    current_function: FunctionId,
    /// Shared function-resolution lookup tables.
    ctx: &'a CodegenContext,
    /// Counter for unique `PartialApply` wrapper/drop function names.
    partial_apply_counter: u32,
    /// ARC variable → typed LLVM value mapping.
    var_map: Vec<Option<EmittedValue>>,
    /// ARC block → LLVM block mapping (`None` for dead unwind blocks).
    block_map: Vec<Option<BlockId>>,
    /// Deferred phi incoming values: `block_index` → `[(param_index, value, source_block)]`.
    /// Collected during terminator emission, applied after all blocks are emitted.
    phi_incoming: Vec<(usize, usize, ValueId, BlockId)>,
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
            current_function,
            ctx,
            partial_apply_counter: 0,
            var_map: Vec::new(),
            block_map: Vec::new(),
            phi_incoming: Vec::new(),
        }
    }

    /// Resolve an `Idx` to an `LLVMTypeId`.
    fn resolve_type(&mut self, idx: Idx) -> LLVMTypeId {
        let llvm_ty = self.type_resolver.resolve(idx);
        self.builder.register_type(llvm_ty)
    }

    /// Allocate a heap cell via `ori_rc_alloc(size, align)`.
    ///
    /// Returns a `ptr` to the RC-managed allocation. Used for boxing
    /// recursive enum fields that must be stored as pointers in the payload.
    fn rc_alloc(&mut self, size: u64, align: u64) -> ValueId {
        let size_val = self.builder.const_i64(size as i64);
        let align_val = self.builder.const_i64(align as i64);
        let rc_alloc_func = self.builder.runtime_fn("ori_rc_alloc");
        self.builder
            .call(rc_alloc_func, &[size_val, align_val], "rc.alloc")
            .unwrap_or_else(|| self.builder.const_null_ptr())
    }

    /// Compute the store size in bytes for a type index.
    ///
    /// Uses `TypeInfo::size()` for well-known types (primitives, str=16, list=24, etc.).
    /// Falls back to `TypeLayoutResolver::type_store_size()` for compound types
    /// (struct, tuple, enum) where the size depends on field layout.
    pub(crate) fn element_store_size(&self, ty: Idx) -> u64 {
        self.type_info.get(ty).size().unwrap_or_else(|| {
            let llvm_ty = self.type_resolver.resolve(ty);
            TypeLayoutResolver::type_store_size(llvm_ty)
        })
    }

    /// Look up the raw LLVM value for an ARC variable.
    ///
    /// Returns the underlying `ValueId`, suitable for consumers that don't
    /// need representation info. For typed access, use [`var_emitted`](Self::var_emitted).
    ///
    /// # Panics
    /// Panics if the stored value is `Pair` or `ZeroSized`. Use `var_emitted()`
    /// for variables that may hold those variants.
    ///
    /// Returns `ValueId::NONE` and logs an error if the variable is not yet defined.
    fn var(&self, v: ArcVarId) -> ValueId {
        self.var_emitted(v).into_raw()
    }

    /// Look up the typed emitted value for an ARC variable.
    ///
    /// Returns the full [`EmittedValue`] including representation info.
    /// Prefer this over [`var`](Self::var) when the consumer needs to
    /// distinguish between value kinds (e.g., RC operations).
    fn var_emitted(&self, v: ArcVarId) -> EmittedValue {
        if let Some(Some(val)) = self.var_map.get(v.index()) {
            *val
        } else {
            tracing::error!(var = v.raw(), "ArcIrEmitter: variable not yet defined");
            EmittedValue::Immediate(ValueId::NONE)
        }
    }

    /// Bind an ARC variable to a typed LLVM value.
    fn def_var(&mut self, v: ArcVarId, val: EmittedValue) {
        let idx = v.index();
        if idx >= self.var_map.len() {
            self.var_map.resize(idx + 1, None);
        }
        self.var_map[idx] = Some(val);
    }

    /// Bind an ARC variable to a raw LLVM value, inferring its [`EmittedValue`]
    /// variant from the variable's [`ValueRepr`] in the ARC function.
    fn def_var_repr(&mut self, v: ArcVarId, val: ValueId, func: &ArcFunction) {
        let repr = func.var_repr(v).unwrap_or(ValueRepr::Scalar);
        self.def_var(v, EmittedValue::from_repr(repr, val));
    }

    /// Look up the LLVM block for an ARC block.
    ///
    /// Panics if the block is a dead unwind block (no LLVM block was created).
    fn block(&self, b: ori_arc::ir::ArcBlockId) -> BlockId {
        self.block_map[b.index()]
            .expect("block() called for dead unwind block — invariant violated")
    }

    /// Get or generate the drop function for a type.
    ///
    /// Returns a function pointer `ValueId` suitable for passing to
    /// `ori_rc_dec`. Returns null for scalar types or when no classifier
    /// is available (no drop needed).
    ///
    /// Drop functions are cached per type. For recursive types, the
    /// `FunctionId` is cached **before** body generation to break cycles.
    fn get_or_generate_drop_fn(&mut self, ty: Idx) -> ValueId {
        // Fast path: already generated
        if let Some(&func_id) = self.drop_fn_cache.get(&ty) {
            return self.builder.get_function_ptr(func_id);
        }

        // Compute what drop operations this type needs
        let Some(drop_info) = ori_arc::compute_drop_info(ty, self.classifier, self.pool) else {
            return self.builder.const_null_ptr();
        };

        // Save current builder position (we're about to create a new function)
        let saved_pos = self.builder.save_position();
        let saved_func = self.builder.current_function();

        // Generate the drop function (handles declaration, caching, and body)
        let func_id = drop_gen::generate_drop_fn(self, ty, &drop_info);

        // Restore builder position
        self.builder.restore_position(saved_pos);
        if let Some(f) = saved_func {
            self.builder.set_current_function(f);
        }

        self.builder.get_function_ptr(func_id)
    }

    // -----------------------------------------------------------------------
    // Top-level emission
    // -----------------------------------------------------------------------

    /// Emit an entire `ArcFunction` as LLVM IR.
    ///
    /// Pre-creates all LLVM blocks, binds function parameters, emits each
    /// block's instructions and terminator, then patches phi nodes.
    pub fn emit_function(&mut self, func: &ArcFunction, abi: &FunctionAbi) {
        // Pre-scan: find dead unwind blocks. With nounwind analysis,
        // Invoke terminators calling known-nounwind functions are downgraded
        // to `call` + `br`, so their unwind blocks become dead code.
        // This must happen before block pre-creation so we can skip creating
        // LLVM basic blocks for dead blocks entirely.
        let mut all_invoke_unwind = rustc_hash::FxHashSet::default();
        let mut unwind_blocks = rustc_hash::FxHashSet::default();
        for block in &func.blocks {
            if let ArcTerminator::Invoke {
                unwind,
                func: callee,
                ..
            } = &block.terminator
            {
                all_invoke_unwind.insert(unwind.index());
                if !self.ctx.nounwind_functions.contains(callee) {
                    unwind_blocks.insert(unwind.index());
                }
            }
        }

        // Dead unwind blocks: targets only of nounwind Invokes (downgraded to call).
        // These blocks have no predecessors and must not be emitted.
        let dead_unwind: rustc_hash::FxHashSet<usize> = all_invoke_unwind
            .difference(&unwind_blocks)
            .copied()
            .collect();

        // Invariant: dead unwind blocks must not be reachable via non-Invoke edges.
        // If a Jump/Branch/Switch targets a dead block, the detection is broken.
        debug_assert!(
            {
                let mut ok = true;
                for block in &func.blocks {
                    let non_invoke_targets: Vec<usize> = match &block.terminator {
                        ArcTerminator::Jump { target, .. } => vec![target.index()],
                        ArcTerminator::Branch {
                            then_block,
                            else_block,
                            ..
                        } => {
                            vec![then_block.index(), else_block.index()]
                        }
                        ArcTerminator::Switch { cases, default, .. } => {
                            let mut t: Vec<usize> = cases.iter().map(|(_, b)| b.index()).collect();
                            t.push(default.index());
                            t
                        }
                        ArcTerminator::Invoke { normal, .. } => vec![normal.index()],
                        _ => vec![],
                    };
                    for target in non_invoke_targets {
                        if dead_unwind.contains(&target) {
                            ok = false;
                        }
                    }
                }
                ok
            },
            "dead unwind block is reachable via non-Invoke terminator — \
             dead_unwind detection invariant violated"
        );

        // Pre-create LLVM blocks, skipping dead unwind blocks
        self.block_map = func
            .blocks
            .iter()
            .enumerate()
            .map(|(i, _)| {
                if dead_unwind.contains(&i) {
                    None
                } else {
                    let name = format!("bb{i}");
                    Some(self.builder.append_block(self.current_function, &name))
                }
            })
            .collect();

        // Resize var_map to hold all variables
        self.var_map.resize(func.var_types.len(), None);

        // Bind function parameters (respecting ABI passing modes).
        // Reference and Indirect params arrive as pointers — load the actual
        // value so ARC IR sees the struct, not the pointer.
        //
        // Non-capturing lambdas have a phantom `ptr %_env` prepended to their
        // LLVM param list (so they're directly callable as closures). Skip it
        // by adding 1 to the starting index.
        let sret_offset = u32::from(matches!(abi.return_abi.passing, ReturnPassing::Sret { .. }));
        let phantom_env_offset = u32::from(self.ctx.non_capturing_lambdas.contains(&func.name));
        let needs_loads = abi.params.iter().any(|p| {
            matches!(
                p.passing,
                ParamPassing::Indirect { .. } | ParamPassing::Reference
            )
        });
        if needs_loads {
            // Position at entry block for load instructions
            self.builder.position_at_end(self.block(func.entry));
        }
        let mut llvm_param_idx = sret_offset + phantom_env_offset;
        for (i, param) in func.params.iter().enumerate() {
            let passing = &abi.params[i].passing;
            match passing {
                ParamPassing::Direct => {
                    let llvm_param = self
                        .builder
                        .get_param(self.current_function, llvm_param_idx);
                    self.def_var_repr(param.var, llvm_param, func);
                    llvm_param_idx += 1;
                }
                ParamPassing::Indirect { .. } | ParamPassing::Reference => {
                    let ptr_param = self
                        .builder
                        .get_param(self.current_function, llvm_param_idx);
                    let ty = self.resolve_type(param.ty);
                    let loaded = self.builder.load(ty, ptr_param, "param.load");
                    self.def_var_repr(param.var, loaded, func);
                    llvm_param_idx += 1;
                }
                ParamPassing::Void => {
                    // No physical LLVM param — bind to a zero/unit constant
                    let zero = self.builder.const_i64(0);
                    self.def_var(param.var, EmittedValue::Immediate(zero));
                }
            }
        }

        // Set personality function on the LLVM function if any real invokes exist.
        // Required for any function containing `invoke`/`landingpad`.
        let personality_id = if unwind_blocks.is_empty() {
            None
        } else {
            let pid = self.builder.runtime_fn("rust_eh_personality");
            self.builder.set_personality(self.current_function, pid);
            Some(pid)
        };

        // Position at entry block
        let entry = self.block(func.entry);
        self.builder.position_at_end(entry);

        // Create phi nodes for blocks with parameters (skip dead unwind blocks)
        let mut phi_nodes: Vec<Vec<(ArcVarId, ValueId)>> = Vec::new();
        for block in &func.blocks {
            let mut block_phis = Vec::new();
            if !block.params.is_empty() && !dead_unwind.contains(&block.id.index()) {
                self.builder.position_at_end(self.block(block.id));
                for &(var, ty) in &block.params {
                    let llvm_ty = self.resolve_type(ty);
                    let phi_val = self.builder.phi(llvm_ty, &format!("v{}", var.raw()));
                    self.def_var_repr(var, phi_val, func);
                    block_phis.push((var, phi_val));
                }
            }
            phi_nodes.push(block_phis);
        }

        // Emit each block's body and terminator.
        // Dead unwind blocks are skipped entirely — no LLVM block was created.
        // Live unwind blocks start with a landing pad. Two flavors:
        // - **Cleanup** (terminator = Resume): `landingpad cleanup`, RC cleanup, resume
        // - **Catch** (terminator = Jump): `landingpad catch null`, free exception via
        //   `ori_catch_cleanup`, then jump to catch handler. Used by `catch(expr:)`.
        let mut landingpad_values: FxHashMap<usize, ValueId> = FxHashMap::default();
        for block in &func.blocks {
            // Dead unwind block: no LLVM block exists, skip entirely
            if dead_unwind.contains(&block.id.index()) {
                continue;
            }

            self.builder.position_at_end(self.block(block.id));

            // Live unwind blocks must start with a landingpad instruction
            if unwind_blocks.contains(&block.id.index()) {
                if let Some(pid) = personality_id {
                    let is_catch = !matches!(block.terminator, ArcTerminator::Resume);
                    if is_catch {
                        // Catch-all landing pad: catches the exception
                        let lp = self.builder.landingpad_catch_all(pid, "lp.catch");
                        landingpad_values.insert(block.id.index(), lp);

                        // Extract exception pointer and free via _Unwind_DeleteException
                        let exc_ptr = self.builder.extract_value(lp, 0, "exc.ptr");
                        if let Some(exc_ptr) = exc_ptr {
                            self.emit_catch_cleanup(exc_ptr);
                        }
                    } else {
                        // Cleanup landing pad: do RC cleanup, then re-raise
                        let lp = self.builder.landingpad(pid, true, "lp");
                        landingpad_values.insert(block.id.index(), lp);
                    }
                }
            }

            for instr in &block.body {
                self.emit_instr(instr, func);
            }
            self.emit_terminator(
                &block.terminator,
                block.id,
                &phi_nodes,
                abi,
                &landingpad_values,
                func,
            );
        }

        // Patch phi incoming values
        for &(block_idx, param_idx, value, source_block) in &self.phi_incoming {
            let (_, phi_val) = phi_nodes[block_idx][param_idx];
            self.builder
                .add_phi_incoming(phi_val, &[(value, source_block)]);
        }
    }

    // -----------------------------------------------------------------------
    // Terminator emission
    // -----------------------------------------------------------------------

    /// Emit an `ArcTerminator` as LLVM control flow.
    fn emit_terminator(
        &mut self,
        term: &ArcTerminator,
        current_block: ori_arc::ir::ArcBlockId,
        _phi_nodes: &[Vec<(ArcVarId, ValueId)>],
        abi: &FunctionAbi,
        landingpad_values: &FxHashMap<usize, ValueId>,
        arc_func: &ArcFunction,
    ) {
        tracing::trace!(?term, block = current_block.index(), "emit_terminator");
        match term {
            ArcTerminator::Return { value } => {
                let val = self.var(*value);
                match &abi.return_abi.passing {
                    ReturnPassing::Sret { .. } => {
                        let sret_ptr = self.builder.get_param(self.current_function, 0);
                        self.builder.store(val, sret_ptr);
                        self.builder.ret_void();
                    }
                    ReturnPassing::Direct => {
                        self.builder.ret(val);
                    }
                    ReturnPassing::Void => {
                        self.builder.ret_void();
                    }
                }
            }

            ArcTerminator::Jump { target, args } => {
                // Record phi incoming values for the target block's parameters
                let target_idx = target.index();
                debug_assert_eq!(
                    args.len(),
                    arc_func.blocks[target_idx].params.len(),
                    "Jump arg count must match target block param count (block {target_idx})"
                );
                if !args.is_empty() {
                    let Some(source_block) = self.builder.current_block() else {
                        tracing::error!("ARC jump: no current block — skipping phi incoming");
                        self.builder.record_codegen_error();
                        self.builder.br(self.block(*target));
                        return;
                    };
                    for (i, &arg) in args.iter().enumerate() {
                        let val = self.var(arg);
                        self.phi_incoming.push((target_idx, i, val, source_block));
                    }
                }
                self.builder.br(self.block(*target));
            }

            ArcTerminator::Branch {
                cond,
                then_block,
                else_block,
            } => {
                let cond_val = self.var(*cond);
                self.builder
                    .cond_br(cond_val, self.block(*then_block), self.block(*else_block));
            }

            ArcTerminator::Switch {
                scrutinee,
                cases,
                default,
            } => {
                let scrut_val = self.var(*scrutinee);
                let llvm_cases: Vec<(ValueId, BlockId)> = cases
                    .iter()
                    .map(|&(tag, block_id)| {
                        let tag_val = self.builder.const_int_matching(scrut_val, tag);
                        (tag_val, self.block(block_id))
                    })
                    .collect();
                self.builder
                    .switch(scrut_val, self.block(*default), &llvm_cases);
            }

            ArcTerminator::Invoke {
                dst,
                ty: _,
                func,
                args,
                arg_ownership: _,
                normal,
                unwind,
            } => self.emit_invoke(*dst, *func, args, *normal, *unwind, arc_func),

            ArcTerminator::Resume => {
                // Re-raise the caught exception using the landingpad token
                // captured at the start of this unwind block.
                if let Some(&lp_val) = landingpad_values.get(&current_block.index()) {
                    self.builder.resume(lp_val);
                } else {
                    // No landingpad for this block — should not happen if ARC IR
                    // is well-formed, but emit unreachable as a safety fallback.
                    tracing::warn!(
                        block = current_block.index(),
                        "ARC Resume without landingpad — emitting unreachable"
                    );
                    self.builder.unreachable();
                }
            }

            ArcTerminator::Unreachable => {
                self.builder.unreachable();
            }
        }
    }

    /// Emit an `Invoke` terminator (ABI-aware function call with unwind).
    ///
    /// When the callee is in [`nounwind_functions`], emits `call` + `br` instead
    /// of `invoke`, eliminating the unwind edge and its associated landing pad.
    fn emit_invoke(
        &mut self,
        dst: ArcVarId,
        callee: Name,
        arc_args: &[ArcVarId],
        normal: ori_arc::ir::ArcBlockId,
        unwind: ori_arc::ir::ArcBlockId,
        arc_func: &ArcFunction,
    ) {
        let func_name_str = self.interner.lookup(callee);
        let normal_block = self.block(normal);
        let is_nounwind = self.ctx.nounwind_functions.contains(&callee);
        let mode = if is_nounwind {
            InvokeMode::Call {
                normal: normal_block,
            }
        } else {
            // Only resolve unwind block when actually needed — dead unwind
            // blocks have no LLVM basic block and would panic in block().
            let unwind_block = self.block(unwind);
            InvokeMode::Invoke {
                normal: normal_block,
                unwind: unwind_block,
            }
        };

        // Intercept ori_format_* calls: decompose string struct arg into (ptr, len).
        if let Some(val) = self.try_emit_format_call(func_name_str, arc_args, arc_func) {
            self.builder.br(normal_block);
            self.builder.position_at_end(normal_block);
            self.def_var_repr(dst, val, arc_func);
            return;
        }

        // Prelude builtin functions (str, int, float, byte, hash_combine, etc.)
        if let Some(val) =
            builtins::prelude::try_emit_prelude_function(self, func_name_str, arc_args, arc_func)
        {
            self.builder.br(normal_block);
            self.builder.position_at_end(normal_block);
            self.def_var_repr(dst, val, arc_func);
            return;
        }

        let arg_vals: Vec<ValueId> = arc_args.iter().map(|a| self.var(*a)).collect();

        // Method dispatch chain:
        // 1. Receiver-based: use first arg's type (instance methods like eq/hash)
        // 2. Return-type-based: use dst's type (static methods like default)
        // 3. Unqualified: bare function name (free functions)
        // 4. Monomorphized generic: match arg types → mangled specialization
        // 5. Diagnostic fallback: logs warning, returns None
        let resolved = self
            .lookup_method_by_receiver(callee, arc_args, arc_func)
            .or_else(|| self.lookup_method_by_return_type(callee, dst, arc_func))
            .or_else(|| self.ctx.functions.get(&callee))
            .or_else(|| self.lookup_mono_dispatch(callee, arc_args, arc_func))
            .or_else(|| self.lookup_method_fallback(callee))
            .map(|(fid, abi)| (*fid, abi.params.clone(), abi.return_abi.clone()));

        if let Some((func_id, params, ret_abi)) = resolved {
            let passed_args = self.apply_param_passing(&arg_vals, &params);
            let result = match &ret_abi.passing {
                ReturnPassing::Sret { .. } => {
                    let ret_ty = self.resolve_type(ret_abi.ty);
                    let sret_alloca = self.builder.alloca(ret_ty, "sret.tmp");
                    let mut full_args = vec![sret_alloca];
                    full_args.extend_from_slice(&passed_args);
                    self.call_or_invoke_llvm(func_id, &full_args, mode, "call");
                    self.builder.position_at_end(mode.normal_block());
                    Some(self.builder.load(ret_ty, sret_alloca, "sret.load"))
                }
                ReturnPassing::Direct | ReturnPassing::Void => {
                    let result = self.call_or_invoke_llvm(func_id, &passed_args, mode, "call");
                    self.builder.position_at_end(mode.normal_block());
                    result
                }
            };
            if let Some(val) = result {
                self.def_var_repr(dst, val, arc_func);
            } else {
                // Void-returning call: ARC IR still expects dst to be defined
                // (uniform SSA — every Invoke produces a variable). Bind to a
                // unit constant so successor blocks can reference it.
                let unit = self.builder.const_i64(0);
                self.def_var(dst, EmittedValue::Immediate(unit));
            }
        } else if let Some(val) = self.try_emit_builtin_method(callee, arc_args, arc_func) {
            // Builtin method handled inline — branch to normal block
            // (the current block needs a terminator since we skipped invoke)
            self.builder.br(normal_block);
            self.builder.position_at_end(normal_block);
            self.def_var_repr(dst, val, arc_func);
        } else if let Some(func_id) = self.builder.try_runtime_fn(func_name_str) {
            // Runtime function fallback with aggregate coercion.
            // Runtime functions take ptr params, but ARC IR passes aggregate
            // structs (Str, List, etc.) by value — coerce as needed.
            let is_list_push = func_name_str == "ori_list_push";
            let coerced_args: Vec<ValueId> = arc_args
                .iter()
                .zip(arg_vals.iter())
                .enumerate()
                .map(|(i, (arc_var, &val))| {
                    let arg_ty = arc_func.var_type(*arc_var);
                    if is_list_push && i == 1 {
                        self.coerce_any_to_ptr(val, arg_ty)
                    } else {
                        self.coerce_aggregate_to_ptr(val, arg_ty)
                    }
                })
                .collect();
            if let Some(val) = self.call_or_invoke_llvm(func_id, &coerced_args, mode, "call") {
                self.builder.position_at_end(mode.normal_block());
                self.def_var_repr(dst, val, arc_func);
            } else {
                // Void-returning runtime function: bind dst to unit constant
                self.builder.position_at_end(mode.normal_block());
                let unit = self.builder.const_i64(0);
                self.def_var(dst, EmittedValue::Immediate(unit));
            }
        } else {
            let msg =
                format!("unresolved function `{func_name_str}` in invoke — missing mono instance?");
            tracing::warn!("{msg}");
            // Emit a branch to the normal block so the IR stays well-formed
            // (every block must have a terminator).
            self.builder.br(normal_block);
            self.builder.position_at_end(normal_block);
            // Bind dst to unit constant so successor blocks don't crash
            let unit = self.builder.const_i64(0);
            self.def_var(dst, EmittedValue::Immediate(unit));
            self.builder.record_codegen_error_with_msg(msg);
        }
    }

    /// Emit either LLVM `invoke` or `call` + `br` based on [`InvokeMode`].
    ///
    /// - `InvokeMode::Invoke`: emits `invoke` with normal + unwind continuations
    /// - `InvokeMode::Call`: emits `call` + unconditional `br` to normal block
    fn call_or_invoke_llvm(
        &mut self,
        func_id: FunctionId,
        args: &[ValueId],
        mode: InvokeMode,
        name: &str,
    ) -> Option<ValueId> {
        match mode {
            InvokeMode::Call { normal } => {
                let result = self.builder.call(func_id, args, name);
                self.builder.br(normal);
                result
            }
            InvokeMode::Invoke { normal, unwind } => {
                self.builder.invoke(func_id, args, normal, unwind, name)
            }
        }
    }

    /// Look up a method function using the first arg's type as a receiver.
    ///
    /// Derived methods (e.g., `compare`, `eq`, `clone`) in ARC IR use unqualified
    /// names. When two types derive the same trait, the unqualified lookup is
    /// ambiguous. This method uses the first arg's type index to resolve the
    /// correct type-qualified entry in `method_functions`.
    fn lookup_method_by_receiver(
        &self,
        name: Name,
        args: &[ArcVarId],
        func: &ArcFunction,
    ) -> Option<&(FunctionId, FunctionAbi)> {
        let &first_arg = args.first()?;
        let receiver_ty = func.var_type(first_arg);
        let type_name = self.ctx.type_idx_to_name.get(&receiver_ty)?;
        self.ctx.method_functions.get(&(*type_name, name))
    }

    /// Look up a static/associated method by its return type.
    ///
    /// Type-qualified calls with no receiver (e.g., `Point.default()`) have an
    /// empty `args` list in ARC IR, so `lookup_method_by_receiver` fails.
    /// For factory methods like `default()`, the return type IS the owning type,
    /// so we can use `func.var_type(dst)` to find the correct type-qualified
    /// entry in `method_functions`.
    fn lookup_method_by_return_type(
        &self,
        name: Name,
        dst: ArcVarId,
        func: &ArcFunction,
    ) -> Option<&(FunctionId, FunctionAbi)> {
        let return_ty = func.var_type(dst);
        let type_name = self.ctx.type_idx_to_name.get(&return_ty)?;
        self.ctx.method_functions.get(&(*type_name, name))
    }

    /// Diagnostic check for method lookup when all typed dispatches miss.
    ///
    /// Always returns `None` — this function only logs diagnostics.
    /// If a method exists in `method_functions` but wasn't found through
    /// normal dispatch, it means the receiver's type wasn't registered in
    /// `type_idx_to_name` (e.g., enum types whose derives aren't compiled yet).
    /// Returning `None` ensures the caller falls through to the "unresolved
    /// function" error path instead of silently calling the wrong method.
    fn lookup_method_fallback(&self, name: Name) -> Option<&(FunctionId, FunctionAbi)> {
        let exists = self
            .ctx
            .method_functions
            .iter()
            .any(|((_, method_name), _)| *method_name == name);
        if exists {
            tracing::warn!(
                method = %self.interner.lookup(name),
                "method exists for another type but receiver type not registered — \
                 likely missing enum derive codegen"
            );
        }
        None
    }

    /// Resolve a generic function call to its monomorphized variant.
    ///
    /// The ARC IR uses the **original** generic name (e.g., `"identity"`),
    /// but the LLVM function was declared under the **mangled** name
    /// (e.g., `"identity$m$int"`). This method matches the concrete argument
    /// types at the call site to find the correct monomorphization.
    fn lookup_mono_dispatch(
        &self,
        callee: Name,
        args: &[ArcVarId],
        func: &ArcFunction,
    ) -> Option<&(FunctionId, FunctionAbi)> {
        let entries = self.ctx.mono_dispatch.get(&callee)?;
        let arg_types: Vec<Idx> = args
            .iter()
            .map(|a| self.pool.resolve_fully(func.var_type(*a)))
            .collect();
        entries
            .iter()
            .find(|(params, _)| {
                params.len() == arg_types.len()
                    && params
                        .iter()
                        .zip(&arg_types)
                        .all(|(p, a)| self.pool.resolve_fully(*p) == *a)
            })
            .and_then(|(_, mangled)| self.ctx.functions.get(mangled))
    }

    /// Emit an `Apply` instruction (ABI-aware direct call).
    fn emit_apply(&mut self, dst: ArcVarId, callee: Name, args: &[ArcVarId], func: &ArcFunction) {
        let callee_name_str = self.interner.lookup(callee);

        // Internal protocol: __iter_next(iter, elem_ty_marker).
        // args[0] = iterator pointer, args[1] = zero marker carrying elem_ty.
        // Result type is INT (no RC management); actual element type comes
        // from the marker argument.
        if callee_name_str == "__iter_next" && args.len() >= 2 {
            let iter_ptr = self.var(args[0]);
            let elem_ty = func.var_type(args[1]);
            if let Some(val) = self.emit_iter_next(iter_ptr, elem_ty) {
                self.def_var_repr(dst, val, func);
            }
            return;
        }

        // ori_list_take uses explicit sret pattern: void(list_ptr, out_ptr).
        // The ARC IR emits Apply "ori_list_take"(list_ptr) expecting a struct return.
        // We handle the sret plumbing here: alloca result struct, call, load.
        if callee_name_str == "ori_list_take" && !args.is_empty() {
            if let Some(val) = self.emit_list_take(args[0], func) {
                self.def_var_repr(dst, val, func);
            }
            return;
        }

        // Intercept ori_format_* calls: decompose string struct arg into (ptr, len).
        if let Some(val) = self.try_emit_format_call(callee_name_str, args, func) {
            self.def_var_repr(dst, val, func);
            return;
        }

        // Prelude builtin functions (str, int, float, byte, hash_combine, etc.)
        if let Some(val) =
            builtins::prelude::try_emit_prelude_function(self, callee_name_str, args, func)
        {
            self.def_var_repr(dst, val, func);
            return;
        }

        let arg_vals: Vec<ValueId> = args.iter().map(|a| self.var(*a)).collect();

        // Method dispatch chain (same as emit_invoke):
        // 1. Receiver-based: use first arg's type (instance methods)
        // 2. Return-type-based: use dst's type (static methods like default)
        // 3. Unqualified: bare function name (free functions)
        // 4. Monomorphized generic: match arg types → mangled specialization
        // 5. Diagnostic fallback: logs warning, returns None
        let resolved = self
            .lookup_method_by_receiver(callee, args, func)
            .or_else(|| self.lookup_method_by_return_type(callee, dst, func))
            .or_else(|| self.ctx.functions.get(&callee))
            .or_else(|| self.lookup_mono_dispatch(callee, args, func))
            .or_else(|| self.lookup_method_fallback(callee))
            .map(|(fid, abi)| (*fid, abi.params.clone(), abi.return_abi.clone()));

        let result = if let Some((func_id, params, ret_abi)) = resolved {
            let passed_args = self.apply_param_passing(&arg_vals, &params);
            match &ret_abi.passing {
                ReturnPassing::Sret { .. } => {
                    let ret_ty = self.resolve_type(ret_abi.ty);
                    self.call_with_sret(func_id, &passed_args, ret_ty, "call")
                }
                ReturnPassing::Direct | ReturnPassing::Void => {
                    self.builder.call(func_id, &passed_args, "call")
                }
            }
        } else if let Some(val) = self.try_emit_builtin_method(callee, args, func) {
            Some(val)
        } else if let Some(func_id) = self.builder.try_runtime_fn(callee_name_str) {
            // Runtime function fallback: coerce aggregate args to pointers.
            // Runtime functions (ori_print, ori_str_*, etc.) take ptr params,
            // but ARC IR passes aggregate structs (Str, List, etc.) by value.
            let is_list_push = callee_name_str == "ori_list_push";
            let coerced_args: Vec<ValueId> = args
                .iter()
                .zip(arg_vals.iter())
                .enumerate()
                .map(|(i, (arc_var, &val))| {
                    let arg_ty = func.var_type(*arc_var);
                    if is_list_push && i == 1 {
                        // ori_list_push(list_ptr, elem_ptr, elem_size):
                        // arg[1] is the element value that must be coerced
                        // to a pointer regardless of its type (even scalars).
                        self.coerce_any_to_ptr(val, arg_ty)
                    } else {
                        self.coerce_aggregate_to_ptr(val, arg_ty)
                    }
                })
                .collect();
            self.builder.call(func_id, &coerced_args, "call")
        } else {
            let msg = format!(
                "unresolved function `{callee_name_str}` in apply — missing mono instance?"
            );
            tracing::warn!("{msg}");
            self.builder.record_codegen_error_with_msg(msg);
            None
        };

        if let Some(val) = result {
            self.def_var_repr(dst, val, func);
        }
    }

    /// Emit an `ApplyIndirect` instruction (indirect call through closure).
    fn emit_apply_indirect(
        &mut self,
        dst: ArcVarId,
        ty: Idx,
        closure: ArcVarId,
        args: &[ArcVarId],
        func: &ArcFunction,
    ) {
        let closure_val = self.var(closure);
        tracing::trace!(
            ?ty,
            tag = ?self.pool.tag(ty),
            closure_var = closure.raw(),
            args = args.len(),
            "emit_apply_indirect"
        );
        let fn_ptr = self.builder.extract_value(closure_val, 0, "closure.fn_ptr");
        let env_ptr = self
            .builder
            .extract_value(closure_val, 1, "closure.env_ptr");

        if let (Some(fn_ptr), Some(env_ptr)) = (fn_ptr, env_ptr) {
            let ptr_ty = self.builder.ptr_type();
            let mut arg_vals = Vec::with_capacity(1 + args.len());
            let mut param_types = Vec::with_capacity(1 + args.len());
            arg_vals.push(env_ptr);
            param_types.push(ptr_ty);

            for &a in args {
                let arg_ty = func.var_type(a);
                let passing = super::abi::compute_param_passing(arg_ty, self.type_info);
                match passing {
                    super::abi::ParamPassing::Indirect { .. }
                    | super::abi::ParamPassing::Reference => {
                        // Large struct: alloca, store, pass pointer
                        let llvm_ty = self.resolve_type(arg_ty);
                        let alloca = self.builder.alloca(llvm_ty, "icall.arg.tmp");
                        self.builder.store(self.var(a), alloca);
                        arg_vals.push(alloca);
                        param_types.push(ptr_ty);
                    }
                    super::abi::ParamPassing::Void => {}
                    super::abi::ParamPassing::Direct => {
                        arg_vals.push(self.var(a));
                        param_types.push(self.resolve_type(arg_ty));
                    }
                }
            }

            let ret_ty = self.resolve_type(ty);
            tracing::trace!(
                ?ty,
                resolved_llvm_ty = ?self.builder.arena.get_type(ret_ty),
                "emit_apply_indirect: resolved return type"
            );
            if let Some(val) =
                self.builder
                    .call_indirect(ret_ty, &param_types, fn_ptr, &arg_vals, "icall")
            {
                self.def_var_repr(dst, val, func);
            }
        } else {
            tracing::error!(
                closure_var = closure.raw(),
                "emit_apply_indirect: extract_value failed — fn_ptr or env_ptr is None"
            );
        }
    }

    /// Emit a `PartialApply` instruction (closure creation).
    ///
    /// For **non-capturing lambdas** (callee is in `non_capturing_lambdas`),
    /// the lambda was declared with closure-compatible ABI (`ccc` + phantom
    /// `ptr %_env`), so we can use the lambda's function pointer directly as
    /// `fn_ptr` without generating a `_ori_partial_N` trampoline. The closure
    /// is `{ lambda_fn_ptr, null }`.
    ///
    /// For **capturing lambdas**, generates a wrapper function that bridges
    /// the closure calling convention `(env_ptr, user_args...)` to the
    /// lambda's flat convention `(captures..., user_args...)`, allocates an
    /// RC-tracked environment struct, and builds `{ wrapper_fn_ptr, env_ptr }`.
    fn emit_partial_apply(
        &mut self,
        dst: ArcVarId,
        _ty: Idx,
        callee: Name,
        args: &[ArcVarId],
        func: &ArcFunction,
    ) {
        let callee_name_str = self.interner.lookup(callee);
        let is_non_capturing = self.ctx.non_capturing_lambdas.contains(&callee);

        tracing::debug!(
            name = callee_name_str,
            captures = args.len(),
            non_capturing = is_non_capturing,
            "ArcIrEmitter: PartialApply — closure creation"
        );

        // Look up the callee (lambda function), already compiled and registered
        let Some(&(callee_func_id, ref callee_abi)) = self.ctx.functions.get(&callee) else {
            tracing::warn!(
                name = callee_name_str,
                "emit_partial_apply: callee not found"
            );
            let closure_ty = self.builder.closure_type();
            let null_ptr = self.builder.const_null_ptr();
            let closure =
                self.builder
                    .build_struct(closure_ty, &[null_ptr, null_ptr], "partial_apply");
            self.def_var(dst, EmittedValue::Aggregate(closure));
            return;
        };

        // Non-capturing fast path: lambda already has closure-compatible ABI,
        // so use its function pointer directly — no wrapper needed.
        if is_non_capturing && args.is_empty() {
            let fn_ptr = self.builder.get_function_ptr(callee_func_id);
            let null_env = self.builder.const_null_ptr();
            let closure_ty = self.builder.closure_type();
            let closure =
                self.builder
                    .build_struct(closure_ty, &[fn_ptr, null_env], "partial_apply.direct");
            self.def_var(dst, EmittedValue::Aggregate(closure));
            return;
        }

        let callee_abi = callee_abi.clone();
        let num_captures = args.len();

        // Capture types (from ARC IR variable types)
        let capture_types: Vec<Idx> = args.iter().map(|&v| func.var_type(v)).collect();

        // Remaining user params (the closure awaits these)
        let remaining_params: Vec<super::abi::ParamAbi> =
            callee_abi.params[num_captures..].to_vec();

        // == Allocate and pack the environment ==
        let env_ptr = if capture_types.is_empty() {
            self.builder.const_null_ptr()
        } else {
            self.build_closure_env(args, &capture_types)
        };

        // == Generate wrapper function ==
        let target_is_nounwind = self.ctx.nounwind_functions.contains(&callee);
        let wrapper_fn_ptr = self.generate_closure_wrapper(
            callee_func_id,
            &callee_abi,
            &capture_types,
            &remaining_params,
            target_is_nounwind,
        );

        // == Build fat-pointer closure { wrapper_fn_ptr, env_ptr } ==
        let closure_ty = self.builder.closure_type();
        let closure =
            self.builder
                .build_struct(closure_ty, &[wrapper_fn_ptr, env_ptr], "partial_apply");
        self.def_var(dst, EmittedValue::Aggregate(closure));
    }

    /// Allocate and pack a closure environment struct.
    ///
    /// Layout: `{ ptr drop_fn, cap_0_ty, cap_1_ty, ... }`
    /// Allocated via `ori_rc_alloc` (RC-tracked heap memory).
    fn build_closure_env(&mut self, capture_vars: &[ArcVarId], capture_types: &[Idx]) -> ValueId {
        // Build env struct type: { drop_fn: ptr, cap_0, cap_1, ... }
        let ptr_llvm = self.builder.scx().type_ptr().into();
        let mut env_fields: Vec<inkwell::types::BasicTypeEnum<'_>> = vec![ptr_llvm];
        for &cap_ty in capture_types {
            env_fields.push(self.type_resolver.resolve(cap_ty));
        }
        let env_struct = self.builder.scx().type_struct(&env_fields, false);
        let env_struct_ty_id = self.builder.register_type(env_struct.into());

        // Compute size via LLVM's target layout, falling back to
        // summing field sizes for compound captures (str, tuple, struct).
        let env_size = env_struct
            .size_of()
            .and_then(inkwell::values::IntValue::get_zero_extended_constant)
            .unwrap_or_else(|| TypeLayoutResolver::type_store_size(env_struct.into()));

        // Allocate via ori_rc_alloc(size, align=8)
        let size_val = self.builder.const_i64(env_size as i64);
        let align_val = self.builder.const_i64(8);
        let rc_alloc_func = self.builder.runtime_fn("ori_rc_alloc");
        let data_ptr = self
            .builder
            .call(rc_alloc_func, &[size_val, align_val], "env.data")
            .unwrap_or_else(|| self.builder.const_null_ptr());

        // Generate drop function for this environment
        let drop_fn_id = self.generate_env_drop_fn(env_struct_ty_id, capture_types, env_size);
        let drop_fn_ptr = self.builder.get_function_ptr(drop_fn_id);

        // Store drop_fn at field 0
        let drop_field = self
            .builder
            .struct_gep(env_struct_ty_id, data_ptr, 0, "env.drop_fn");
        self.builder.store(drop_fn_ptr, drop_field);

        // Store each capture at fields 1..N
        #[expect(
            clippy::cast_possible_truncation,
            reason = "capture count bounded by lambda arity, well within u32 range"
        )]
        for (i, &cap_var) in capture_vars.iter().enumerate() {
            let cap_val = self.var(cap_var);
            let field_ptr = self.builder.struct_gep(
                env_struct_ty_id,
                data_ptr,
                (i + 1) as u32,
                &format!("env.cap.{i}"),
            );
            self.builder.store(cap_val, field_ptr);
        }

        data_ptr
    }

    /// Generate a drop function for a closure environment.
    ///
    /// The drop function RC-decrements each captured variable that is
    /// reference-counted, then frees the environment via `ori_rc_free`.
    fn generate_env_drop_fn(
        &mut self,
        env_struct_ty_id: LLVMTypeId,
        capture_types: &[Idx],
        env_size: u64,
    ) -> FunctionId {
        let partial_id = self.partial_apply_counter;
        self.partial_apply_counter += 1;
        let func_name = format!("_ori_partial_{partial_id}_drop");

        // Save builder position
        let saved_pos = self.builder.save_position();
        let saved_func = self.builder.current_function();

        // Declare: void @_ori_partial_N_drop(ptr %data)
        let ptr_ty = self.builder.ptr_type();
        let func_id = self.builder.declare_void_function(&func_name, &[ptr_ty]);
        self.builder.set_ccc(func_id);
        self.builder.add_nounwind_attribute(func_id);
        self.builder.add_cold_attribute(func_id);

        // Generate body
        let entry = self.builder.append_block(func_id, "entry");
        self.builder.position_at_end(entry);
        self.builder.set_current_function(func_id);

        let data_ptr = self.builder.get_param(func_id, 0);

        // RC dec each captured variable that needs it
        #[expect(
            clippy::cast_possible_truncation,
            reason = "capture count bounded by lambda arity, well within u32 range"
        )]
        for (i, &cap_ty) in capture_types.iter().enumerate() {
            // Check if this capture type needs RC management
            let needs_rc = self.classifier.needs_rc(cap_ty);
            if needs_rc {
                let field_ty = self.resolve_type(cap_ty);
                let field_ptr = self.builder.struct_gep(
                    env_struct_ty_id,
                    data_ptr,
                    (i + 1) as u32, // +1: field 0 is drop_fn
                    &format!("cap.{i}.ptr"),
                );
                let field_val = self.builder.load(field_ty, field_ptr, &format!("cap.{i}"));
                let data_ptrs = self.extract_rc_data_ptrs(field_val, cap_ty);
                let drop_fn = self.get_or_generate_drop_fn(cap_ty);
                let rc_dec_id = self.builder.runtime_fn("ori_rc_dec");
                for data_ptr_val in data_ptrs {
                    self.builder.call(rc_dec_id, &[data_ptr_val, drop_fn], "");
                }
            }
        }

        // Free the env struct
        let size_val = self.builder.const_i64(env_size as i64);
        let align_val = self.builder.const_i64(8);
        let rc_free_id = self.builder.runtime_fn("ori_rc_free");
        self.builder
            .call(rc_free_id, &[data_ptr, size_val, align_val], "");
        self.builder.ret_void();

        // Restore builder position
        self.builder.restore_position(saved_pos);
        if let Some(f) = saved_func {
            self.builder.set_current_function(f);
        }

        func_id
    }

    /// Generate a wrapper function for a closure.
    ///
    /// The wrapper bridges the closure calling convention `(env_ptr, user_args...)`
    /// to the lambda's flat calling convention `(captures..., user_args...)`.
    ///
    /// ```text
    /// define ccc ret_type @_ori_partial_N(ptr %env, <user_param_types...>) {
    ///   %cap.0 = gep env_struct, %env, 0, 1 → load
    ///   ...
    ///   %result = call fastcc ret_type @callee(%cap.0, ..., %user_param_0, ...)
    ///   ret ret_type %result
    /// }
    /// ```
    fn generate_closure_wrapper(
        &mut self,
        callee_func_id: FunctionId,
        callee_abi: &FunctionAbi,
        capture_types: &[Idx],
        remaining_params: &[super::abi::ParamAbi],
        target_is_nounwind: bool,
    ) -> ValueId {
        let partial_id = self.partial_apply_counter;
        self.partial_apply_counter += 1;
        let wrapper_name = format!("_ori_partial_{partial_id}");

        // Save builder position
        let saved_pos = self.builder.save_position();
        let saved_func = self.builder.current_function();

        // Build wrapper parameter types: ptr %env + remaining user params
        let ptr_ty = self.builder.ptr_type();
        let mut wrapper_param_types = Vec::with_capacity(1 + remaining_params.len());
        wrapper_param_types.push(ptr_ty); // env_ptr
        for param in remaining_params {
            match &param.passing {
                super::abi::ParamPassing::Direct => {
                    let ty = self.resolve_type(param.ty);
                    wrapper_param_types.push(ty);
                }
                super::abi::ParamPassing::Indirect { .. } | super::abi::ParamPassing::Reference => {
                    wrapper_param_types.push(ptr_ty);
                }
                super::abi::ParamPassing::Void => {}
            }
        }

        // Determine return type
        let ret_ty = self.resolve_type(callee_abi.return_abi.ty);
        let has_sret = matches!(callee_abi.return_abi.passing, ReturnPassing::Sret { .. });
        let is_void = matches!(callee_abi.return_abi.passing, ReturnPassing::Void);

        // Declare wrapper function
        let wrapper_func_id = if is_void || has_sret {
            self.builder
                .declare_void_function(&wrapper_name, &wrapper_param_types)
        } else {
            self.builder
                .declare_function(&wrapper_name, &wrapper_param_types, ret_ty)
        };
        self.builder.set_ccc(wrapper_func_id);
        if target_is_nounwind {
            self.builder.add_nounwind_attribute(wrapper_func_id);
        }

        // Generate wrapper body
        let entry = self.builder.append_block(wrapper_func_id, "entry");
        self.builder.position_at_end(entry);
        self.builder.set_current_function(wrapper_func_id);

        let env_ptr_val = self.builder.get_param(wrapper_func_id, 0);

        // Build env struct type for GEP (same layout as build_closure_env)
        let ptr_llvm = self.builder.scx().type_ptr().into();
        let mut env_fields: Vec<inkwell::types::BasicTypeEnum<'_>> = vec![ptr_llvm];
        for &cap_ty in capture_types {
            env_fields.push(self.type_resolver.resolve(cap_ty));
        }
        let env_struct = self.builder.scx().type_struct(&env_fields, false);
        let env_struct_ty_id = self.builder.register_type(env_struct.into());

        // Unpack captures from env struct (fields 1..N)
        let mut callee_args = Vec::with_capacity(callee_abi.params.len());

        // Handle sret: if callee uses sret, allocate a temp and pass it first
        let sret_alloca = if has_sret {
            let alloca = self.builder.alloca(ret_ty, "sret.tmp");
            callee_args.push(alloca);
            Some(alloca)
        } else {
            None
        };

        #[expect(
            clippy::cast_possible_truncation,
            reason = "capture count bounded by lambda arity, well within u32 range"
        )]
        for (i, &cap_ty) in capture_types.iter().enumerate() {
            let field_ty = self.resolve_type(cap_ty);
            let field_ptr = self.builder.struct_gep(
                env_struct_ty_id,
                env_ptr_val,
                (i + 1) as u32,
                &format!("cap.{i}.ptr"),
            );
            // Check callee ABI: if this capture param is Indirect/Reference,
            // pass the pointer directly; otherwise load and pass by value.
            // Note: callee_abi.params does NOT include sret (sret is in return_abi),
            // so params[i] directly maps to the i-th capture parameter.
            let param_passing = callee_abi.params.get(i).map(|p| &p.passing);
            if matches!(
                param_passing,
                Some(
                    super::abi::ParamPassing::Indirect { .. } | super::abi::ParamPassing::Reference
                )
            ) {
                callee_args.push(field_ptr);
            } else {
                let cap_val = self.builder.load(field_ty, field_ptr, &format!("cap.{i}"));
                callee_args.push(cap_val);
            }
        }

        // Forward remaining user params (wrapper params 1..N)
        let mut wrapper_param_idx: u32 = 1; // 0 = env_ptr
        for param in remaining_params {
            if param.passing != super::abi::ParamPassing::Void {
                let user_val = self.builder.get_param(wrapper_func_id, wrapper_param_idx);
                callee_args.push(user_val);
                wrapper_param_idx += 1;
            }
        }

        // Call the actual lambda function
        let result = self.builder.call(callee_func_id, &callee_args, "result");

        // Emit return
        if has_sret {
            if let Some(alloca) = sret_alloca {
                // Load from sret alloca and return... but wrapper is void for sret.
                // Actually, the wrapper itself is called indirectly via ccc.
                // ApplyIndirect doesn't use sret — it uses direct returns.
                // So the wrapper must load from sret and return directly.
                let loaded = self.builder.load(ret_ty, alloca, "sret.load");
                self.builder.ret(loaded);
            }
        } else if is_void {
            self.builder.ret_void();
        } else if let Some(val) = result {
            self.builder.ret(val);
        } else {
            let zero = self.builder.const_i64(0);
            self.builder.ret(zero);
        }

        // Restore builder position
        self.builder.restore_position(saved_pos);
        if let Some(f) = saved_func {
            self.builder.set_current_function(f);
        }

        self.builder.get_function_ptr(wrapper_func_id)
    }

    /// Emit a `Project` instruction (field extraction).
    ///
    /// For tagged union payload fields (Result, Enum), the LLVM storage type
    /// may differ from the expected type (e.g., `int` payload stored in a
    /// `{i64, ptr}` slot of `Result<int, str>`). These use alloca + GEP + load
    /// for type-safe extraction through pointer reinterpretation.
    fn emit_project(
        &mut self,
        dst: ArcVarId,
        ty: Idx,
        value: ArcVarId,
        field: u32,
        func: &ArcFunction,
    ) {
        let val = self.var(value);
        let result_ty = self.resolve_type(ty);

        // For enum/Result payload fields (index > 0), the storage type may
        // differ from the variant's actual type. Use alloca + GEP + load to
        // reinterpret the bytes correctly through pointer casting.
        if field > 0 {
            let val_ty = func.var_type(value);
            let val_type_info = self.type_info.get(val_ty);
            if matches!(
                val_type_info,
                super::type_info::TypeInfo::Result { .. } | super::type_info::TypeInfo::Enum { .. }
            ) {
                let is_general_enum =
                    matches!(val_type_info, super::type_info::TypeInfo::Enum { .. });
                let llvm_val_ty = self.resolve_type(val_ty);
                let alloca = self.builder.alloca(llvm_val_ty, "proj.alloca");
                self.builder.store(val, alloca);
                if is_general_enum {
                    // General enum: payload is [M x i64] at struct field 1.
                    // Index into the payload array with i64-stride GEP.
                    let payload_ptr =
                        self.builder
                            .struct_gep(llvm_val_ty, alloca, 1, "proj.payload");
                    let i64_ty = self.builder.i64_type();
                    let slot_idx = self.builder.const_i64(i64::from(field - 1));
                    let slot_ptr = self.builder.gep(
                        i64_ty,
                        payload_ptr,
                        &[slot_idx],
                        &format!("proj.{field}.gep"),
                    );

                    if is_boxed_enum_field(self.pool, val_ty, ty) {
                        // Recursive field: stored as RC pointer in the payload.
                        // Load the pointer, then load the struct from the heap.
                        let ptr_ty = self.builder.ptr_type();
                        let rc_ptr =
                            self.builder
                                .load(ptr_ty, slot_ptr, &format!("proj.{field}.ptr"));
                        let loaded = self
                            .builder
                            .load(result_ty, rc_ptr, &format!("proj.{field}"));
                        self.def_var_repr(dst, loaded, func);
                    } else {
                        let loaded =
                            self.builder
                                .load(result_ty, slot_ptr, &format!("proj.{field}"));
                        self.def_var_repr(dst, loaded, func);
                    }
                } else {
                    // Result: payload is a typed field at struct index 1.
                    let gep = self.builder.struct_gep(
                        llvm_val_ty,
                        alloca,
                        field,
                        &format!("proj.{field}.gep"),
                    );
                    let loaded = self.builder.load(result_ty, gep, &format!("proj.{field}"));
                    self.def_var_repr(dst, loaded, func);
                }
                return;
            }
        }

        if let Some(extracted) = self
            .builder
            .extract_value(val, field, &format!("proj.{field}"))
        {
            self.def_var_repr(dst, extracted, func);
        } else {
            // Fallback: GEP-based field access for heap-allocated types
            let val_ty = func.var_type(value);
            let llvm_val_ty = self.resolve_type(val_ty);
            let gep =
                self.builder
                    .struct_gep(llvm_val_ty, val, field, &format!("proj.{field}.gep"));
            let loaded = self.builder.load(result_ty, gep, &format!("proj.{field}"));
            self.def_var_repr(dst, loaded, func);
        }
    }

    // -----------------------------------------------------------------------
    // Instruction emission
    // -----------------------------------------------------------------------

    /// Emit a single `ArcInstr` as LLVM IR.
    fn emit_instr(&mut self, instr: &ArcInstr, func: &ArcFunction) {
        tracing::trace!(?instr, "emit_instr");
        match instr {
            ArcInstr::Let { dst, ty, value } => {
                let val = self.emit_value(value, *ty, func);
                self.def_var_repr(*dst, val, func);
            }

            ArcInstr::Apply {
                dst,
                ty: _,
                func: callee,
                args,
                arg_ownership: _,
            } => self.emit_apply(*dst, *callee, args, func),

            ArcInstr::ApplyIndirect {
                dst,
                ty,
                closure,
                args,
            } => self.emit_apply_indirect(*dst, *ty, *closure, args, func),

            ArcInstr::PartialApply {
                dst,
                ty,
                func: callee,
                args,
            } => self.emit_partial_apply(*dst, *ty, *callee, args, func),

            ArcInstr::Project {
                dst,
                ty,
                value,
                field,
            } => self.emit_project(*dst, *ty, *value, *field, func),

            ArcInstr::Construct {
                dst,
                ty,
                ctor,
                args,
            } => {
                let val = self.emit_construct(*ty, ctor, args);
                self.def_var_repr(*dst, val, func);
            }

            // RC operations — dispatched by strategy (no Pool queries)
            ArcInstr::RcInc {
                var,
                count,
                strategy,
            } => {
                self.emit_rc_inc(*var, *count, *strategy, func);
            }

            ArcInstr::RcDec { var, strategy } => {
                self.emit_rc_dec(*var, *strategy, func);
            }

            ArcInstr::IsShared { dst, var } => {
                // Inline refcount check: data_ptr - 8 = strong_count (i64).
                // Shared when strong_count > 1 (more than one owner).
                //
                // Only valid for RcPointer values (heap-allocated behind an RC
                // header). Aggregates (struct, tuple) and fat values (str) are
                // inline SSA values with no RC header — they are always
                // "shared" (force the slow Construct path).
                let repr = func.var_repr(*var).unwrap_or(ValueRepr::Scalar);
                if repr == ValueRepr::RcPointer {
                    let data_ptr = self.var(*var);
                    let i8_ty = self.builder.i8_type();
                    let neg8 = self.builder.const_i64(-8);
                    let rc_ptr = self.builder.gep(i8_ty, data_ptr, &[neg8], "rc_ptr");
                    let i64_ty = self.builder.i64_type();
                    let rc_val = self.builder.load(i64_ty, rc_ptr, "rc_val");
                    let one = self.builder.const_i64(1);
                    let is_shared = self.builder.icmp_sgt(rc_val, one, "is_shared");
                    self.def_var(*dst, EmittedValue::Immediate(is_shared));
                } else {
                    // Non-pointer value: no RC header to check.
                    // Emit `true` (always shared) to force the slow path
                    // which uses Construct instead of in-place Set.
                    tracing::trace!(
                        var = var.raw(),
                        ?repr,
                        "IsShared on non-pointer value — emitting true"
                    );
                    let always_shared = self.builder.const_bool(true);
                    self.def_var(*dst, EmittedValue::Immediate(always_shared));
                }
            }

            ArcInstr::Reset { var, token } => {
                // Reset marks a value for potential reuse. After expansion by
                // Section 09, this becomes IsShared + conditional.
                // The token IS the variable (reuse its memory if unique).
                let emitted = self.var_emitted(*var);
                self.def_var(*token, emitted);
            }

            ArcInstr::Reuse {
                token,
                dst,
                ty,
                ctor,
                args,
            } => {
                // Defensive fallback: after expand_reuse, Reuse instructions are
                // eliminated — the fast path uses Set/SetTag and the slow path uses
                // Construct. If Reuse appears (e.g., expansion was skipped), fall
                // back to fresh construction.
                tracing::debug!("ArcIrEmitter: Reuse instruction not expanded — using Construct");
                let val = self.emit_construct(*ty, ctor, args);
                self.def_var_repr(*dst, val, func);
                let _ = token;
            }

            ArcInstr::Set { base, field, value } => {
                // In-place field update (only valid when uniquely owned).
                // After expand_reuse, this only appears in the fast path for
                // heap-allocated RC'd objects (pointer-typed base).
                let repr = func.var_repr(*base).unwrap_or(ValueRepr::Scalar);
                if repr == ValueRepr::RcPointer {
                    let base_val = self.var(*base);
                    let new_val = self.var(*value);
                    let base_ty = func.var_type(*base);
                    let llvm_ty = self.resolve_type(base_ty);

                    // GEP + store for heap-allocated RC'd objects.
                    // The base is a pointer to the struct data on the heap.
                    let field_ptr = self.builder.struct_gep(
                        llvm_ty,
                        base_val,
                        *field,
                        &format!("set.{field}.ptr"),
                    );
                    self.builder.store(new_val, field_ptr);
                    // base pointer unchanged — mutation is in-place
                } else {
                    // Non-pointer base: this block is unreachable (IsShared
                    // emitted `true` for non-pointer values, so the branch
                    // always takes the slow Construct path). Emit nothing.
                    tracing::trace!(
                        base = base.raw(),
                        field,
                        ?repr,
                        "Set on non-pointer value — skipping (unreachable)"
                    );
                }
            }

            ArcInstr::SetTag { base, tag } => {
                // In-place tag update for enum variants.
                // Tag is field 0 of the enum representation: { i8 tag, ... }
                let base_val = self.var(*base);
                let base_ty = func.var_type(*base);
                let llvm_ty = self.resolve_type(base_ty);

                let tag_ptr = self.builder.struct_gep(llvm_ty, base_val, 0, "set.tag.ptr");
                let tag_val = self.builder.const_i64(*tag as i64);
                self.builder.store(tag_val, tag_ptr);
                // base pointer unchanged — mutation is in-place
            }

            ArcInstr::Select {
                dst,
                cond,
                true_val,
                false_val,
                ..
            } => {
                let c = self.var(*cond);
                let t = self.var(*true_val);
                let f = self.var(*false_val);
                let result = self.builder.select(c, t, f, "sel");
                self.def_var(*dst, EmittedValue::Immediate(result));
            }
        }
    }

    // -----------------------------------------------------------------------
    // Value emission (for ArcValue in Let instructions)
    // -----------------------------------------------------------------------

    /// Emit an `ArcValue` as an LLVM value.
    fn emit_value(&mut self, value: &ArcValue, ty: Idx, func: &ArcFunction) -> ValueId {
        match value {
            ArcValue::Var(v) => self.var(*v),

            ArcValue::Literal(lit) => self.emit_literal(lit),

            ArcValue::PrimOp { op, args } => {
                let arg_vals: Vec<ValueId> = args.iter().map(|a| self.var(*a)).collect();
                self.emit_primop(*op, &arg_vals, ty, func, args)
            }
        }
    }

    /// Emit a literal value.
    fn emit_literal(&mut self, lit: &LitValue) -> ValueId {
        match lit {
            LitValue::Int(n) => self.builder.const_i64(*n),
            LitValue::Float(bits) => self.builder.const_f64(f64::from_bits(*bits)),
            LitValue::Bool(b) => self.builder.const_bool(*b),
            LitValue::Char(c) => self.builder.const_i32(*c as i32),
            LitValue::Unit => self.builder.const_i64(0),
            LitValue::String(name) => {
                let s = self.interner.lookup(*name);
                // Use ori_str_from_raw to create an RC-managed heap copy of
                // the string literal. This ensures the data pointer has a
                // valid RC header for ARC RcInc/RcDec operations.
                let global = self.builder.build_global_string_ptr(s, "str");
                let len = self.builder.const_i64(s.len() as i64);
                let func_id = self.builder.runtime_fn("ori_str_from_raw");
                self.builder
                    .call(func_id, &[global, len], "str.val")
                    .unwrap_or_else(|| {
                        // Fallback: build inline struct (no RC safety)
                        let str_ty = self.builder.register_type(
                            self.builder
                                .scx()
                                .type_struct(
                                    &[
                                        self.builder.scx().type_i64().into(),
                                        self.builder.scx().type_ptr().into(),
                                    ],
                                    false,
                                )
                                .into(),
                        );
                        self.builder.build_struct(str_ty, &[len, global], "str.val")
                    })
            }
            LitValue::Duration { value, unit } => {
                let nanos = unit.to_nanos(*value);
                self.builder.const_i64(nanos)
            }
            LitValue::Size { value, unit } => {
                let bytes = unit.to_bytes(*value);
                self.builder.const_i64(bytes as i64)
            }
        }
    }

    /// Emit a primitive operation.
    fn emit_primop(
        &mut self,
        op: PrimOp,
        arg_vals: &[ValueId],
        _ty: Idx,
        func: &ArcFunction,
        arc_args: &[ArcVarId],
    ) -> ValueId {
        match op {
            PrimOp::Binary(bin_op) => {
                let lhs = arg_vals[0];
                let rhs = arg_vals[1];
                let lhs_ty = func.var_type(arc_args[0]);
                self.emit_binary_op(bin_op, lhs, rhs, lhs_ty)
            }
            PrimOp::Unary(un_op) => {
                let operand = arg_vals[0];
                let operand_ty = func.var_type(arc_args[0]);
                self.emit_unary_op(un_op, operand, operand_ty)
            }
        }
    }

    /// Emit a binary operation.
    ///
    /// For primitive types, emits direct LLVM instructions. For non-primitive
    /// types, dispatches to the corresponding operator trait method
    /// (e.g., `+` → `Add.add()`, `==` → `Eq.equals()`, `<` → `Comparable.compare()`).
    fn emit_binary_op(&mut self, op: BinaryOp, lhs: ValueId, rhs: ValueId, lhs_ty: Idx) -> ValueId {
        // Trait dispatch for non-primitive types (user-defined operator impls)
        if !lhs_ty.is_primitive() {
            // Arithmetic operators (Add, Sub, Mul, etc.)
            if let Some(result) = self.emit_binary_op_via_trait(op, lhs, rhs, lhs_ty) {
                return result;
            }
            // Comparison operators (==, !=, <, >, <=, >=)
            if let Some(result) = self.emit_comparison_via_trait(op, lhs, rhs, lhs_ty) {
                return result;
            }
        }

        let type_info = self.type_info.get(lhs_ty);
        let is_float = matches!(type_info, super::type_info::TypeInfo::Float);
        let is_str = matches!(type_info, super::type_info::TypeInfo::Str);

        // List + list → concat (same as str + str → concat)
        if matches!(op, BinaryOp::Add) {
            if let super::type_info::TypeInfo::List { element } = type_info {
                if let Some(val) = self.emit_list_concat(lhs, rhs, element) {
                    return val;
                }
            }
        }

        match op {
            BinaryOp::Add if is_float => self.builder.fadd(lhs, rhs, "add"),
            BinaryOp::Add if is_str => self.emit_str_runtime_call("ori_str_concat", lhs, rhs, true),
            BinaryOp::Add => self.builder.add(lhs, rhs, "add"),
            BinaryOp::Sub if is_float => self.builder.fsub(lhs, rhs, "sub"),
            BinaryOp::Sub => self.builder.sub(lhs, rhs, "sub"),
            BinaryOp::Mul if is_float => self.builder.fmul(lhs, rhs, "mul"),
            BinaryOp::Mul => self.builder.mul(lhs, rhs, "mul"),
            BinaryOp::Div if is_float => self.builder.fdiv(lhs, rhs, "div"),
            BinaryOp::Div => self.builder.sdiv(lhs, rhs, "div"),
            BinaryOp::Mod if is_float => self.builder.frem(lhs, rhs, "rem"),
            BinaryOp::Mod => self.builder.srem(lhs, rhs, "rem"),
            BinaryOp::Eq if is_float => self.builder.fcmp_oeq(lhs, rhs, "eq"),
            BinaryOp::Eq if is_str => self.emit_str_runtime_call("ori_str_eq", lhs, rhs, false),
            BinaryOp::Eq => self.builder.icmp_eq(lhs, rhs, "eq"),
            BinaryOp::NotEq if is_float => self.builder.fcmp_one(lhs, rhs, "ne"),
            BinaryOp::NotEq if is_str => self.emit_str_runtime_call("ori_str_ne", lhs, rhs, false),
            BinaryOp::NotEq => self.builder.icmp_ne(lhs, rhs, "ne"),
            BinaryOp::Lt if is_float => self.builder.fcmp_olt(lhs, rhs, "lt"),
            BinaryOp::Lt if is_str => self
                .emit_str_cmp_predicate(lhs, rhs, builtins::CmpPredicate::Less)
                .unwrap_or_else(|| self.builder.icmp_slt(lhs, rhs, "lt")),
            BinaryOp::Lt => self.builder.icmp_slt(lhs, rhs, "lt"),
            BinaryOp::Gt if is_float => self.builder.fcmp_ogt(lhs, rhs, "gt"),
            BinaryOp::Gt if is_str => self
                .emit_str_cmp_predicate(lhs, rhs, builtins::CmpPredicate::Greater)
                .unwrap_or_else(|| self.builder.icmp_sgt(lhs, rhs, "gt")),
            BinaryOp::Gt => self.builder.icmp_sgt(lhs, rhs, "gt"),
            BinaryOp::LtEq if is_float => self.builder.fcmp_ole(lhs, rhs, "le"),
            BinaryOp::LtEq if is_str => self
                .emit_str_cmp_predicate(lhs, rhs, builtins::CmpPredicate::LessOrEqual)
                .unwrap_or_else(|| self.builder.icmp_sle(lhs, rhs, "le")),
            BinaryOp::LtEq => self.builder.icmp_sle(lhs, rhs, "le"),
            BinaryOp::GtEq if is_float => self.builder.fcmp_oge(lhs, rhs, "ge"),
            BinaryOp::GtEq if is_str => self
                .emit_str_cmp_predicate(lhs, rhs, builtins::CmpPredicate::GreaterOrEqual)
                .unwrap_or_else(|| self.builder.icmp_sge(lhs, rhs, "ge")),
            BinaryOp::GtEq => self.builder.icmp_sge(lhs, rhs, "ge"),
            BinaryOp::And => self.builder.and(lhs, rhs, "and"),
            BinaryOp::Or => self.builder.or(lhs, rhs, "or"),
            BinaryOp::BitAnd => self.builder.and(lhs, rhs, "bitand"),
            BinaryOp::BitOr => self.builder.or(lhs, rhs, "bitor"),
            BinaryOp::BitXor => self.builder.xor(lhs, rhs, "bitxor"),
            BinaryOp::Shl => self.builder.shl(lhs, rhs, "shl"),
            BinaryOp::Shr => self.builder.ashr(lhs, rhs, "shr"),
            BinaryOp::FloorDiv => self.builder.sdiv(lhs, rhs, "floordiv"),
            BinaryOp::Coalesce => {
                // opt ?? default → extract tag, if Some(0) return payload else default
                // Result: same pattern — Ok(0) return payload else default
                let tag = self
                    .builder
                    .extract_value(lhs, 0, "coal.tag")
                    .unwrap_or(lhs);
                let payload = self
                    .builder
                    .extract_value(lhs, 1, "coal.val")
                    .unwrap_or(lhs);
                let zero = self.builder.const_i64(0);
                let is_some = self.builder.icmp_eq(tag, zero, "is_some");
                self.builder.select(is_some, payload, rhs, "coal")
            }
            BinaryOp::Range | BinaryOp::RangeInclusive | BinaryOp::MatMul => {
                // Range/matmul ops are desugared or trait-dispatched before reaching ARC IR
                tracing::warn!(?op, "ArcIrEmitter: desugared op in binary expression");
                self.builder.const_i64(0)
            }
        }
    }

    /// Emit a unary operation.
    ///
    /// For primitive types, emits direct LLVM instructions. For non-primitive
    /// types, dispatches to the corresponding operator trait method
    /// (e.g., `-` → `Negate.negate()`).
    fn emit_unary_op(&mut self, op: UnaryOp, operand: ValueId, operand_ty: Idx) -> ValueId {
        // Trait dispatch for non-primitive types (user-defined operator impls)
        if !operand_ty.is_primitive() {
            if let Some(result) = self.emit_unary_op_via_trait(op, operand, operand_ty) {
                return result;
            }
        }

        let is_float = matches!(
            self.type_info.get(operand_ty),
            super::type_info::TypeInfo::Float
        );

        match op {
            UnaryOp::Neg if is_float => self.builder.fneg(operand, "neg"),
            UnaryOp::Neg => self.builder.neg(operand, "neg"),
            UnaryOp::Not => self.builder.not(operand, "not"),
            UnaryOp::BitNot => {
                let all_ones = self.builder.const_i64(-1);
                self.builder.xor(operand, all_ones, "bitnot")
            }
            UnaryOp::Try => {
                // Try is desugared before reaching ARC IR
                tracing::warn!("ArcIrEmitter: try op in unary expression");
                self.builder.const_i64(0)
            }
        }
    }

    // -----------------------------------------------------------------------
    // Operator trait dispatch
    // -----------------------------------------------------------------------

    /// Dispatch a binary operator to a trait method for non-primitive types.
    ///
    /// Maps the operator to its trait method name (e.g., `+` → `"add"`),
    /// looks up the compiled method function, and emits a method call.
    fn emit_binary_op_via_trait(
        &mut self,
        op: BinaryOp,
        lhs: ValueId,
        rhs: ValueId,
        lhs_ty: Idx,
    ) -> Option<ValueId> {
        let method_name = op.trait_method_name()?;
        let type_name = *self.ctx.type_idx_to_name.get(&lhs_ty)?;
        let interned_method = self.interner.intern(method_name);
        // Scope the immutable borrow of method_functions: extract only what
        // we need so we can call &mut self methods below.
        let (func_id, params, ret_passing, ret_ty_idx) = {
            let (fid, abi) = self
                .ctx
                .method_functions
                .get(&(type_name, interned_method))?;
            (
                *fid,
                abi.params.clone(),
                abi.return_abi.passing.clone(),
                abi.return_abi.ty,
            )
        };

        let raw_args = [lhs, rhs];
        let passed_args = self.apply_param_passing(&raw_args, &params);

        match &ret_passing {
            ReturnPassing::Sret { .. } => {
                let ret_ty = self.resolve_type(ret_ty_idx);
                self.call_with_sret(func_id, &passed_args, ret_ty, "op_trait")
            }
            ReturnPassing::Direct | ReturnPassing::Void => {
                self.builder.call(func_id, &passed_args, "op_trait")
            }
        }
    }

    /// Dispatch comparison operators to Eq/Comparable trait methods.
    ///
    /// Comparison operators are not in `trait_method_name()` because they use
    /// a different dispatch model than arithmetic operators:
    /// - `==`/`!=` → `Eq.equals(self, other) -> bool`
    /// - `<`/`>`/`<=`/`>=` → `Comparable.compare(self, other) -> Ordering`
    ///   then check the i8 result against ordering constants.
    fn emit_comparison_via_trait(
        &mut self,
        op: BinaryOp,
        lhs: ValueId,
        rhs: ValueId,
        lhs_ty: Idx,
    ) -> Option<ValueId> {
        // Map comparison operators to their trait method and post-processing.
        // Note: Eq.method_name() is "eq" (not "equals") per DerivedTrait definition.
        let (method_name, negate) = match op {
            BinaryOp::Eq => ("eq", false),
            BinaryOp::NotEq => ("eq", true),
            BinaryOp::Lt | BinaryOp::Gt | BinaryOp::LtEq | BinaryOp::GtEq => {
                return self.emit_ordering_comparison(op, lhs, rhs, lhs_ty);
            }
            _ => return None,
        };

        // Tuple equality: compare element-wise inline (no trait impl).
        // Tuples aren't in type_idx_to_name so trait dispatch won't find them.
        if let super::type_info::TypeInfo::Tuple { elements } = self.type_info.get(lhs_ty) {
            let result = self.emit_tuple_equals(lhs, rhs, &elements);
            return if negate {
                result.map(|r| self.builder.not(r, "neq"))
            } else {
                result
            };
        }

        let type_name = *self.ctx.type_idx_to_name.get(&lhs_ty)?;
        let interned_method = self.interner.intern(method_name);
        let (func_id, params, ret_passing) = {
            let (fid, abi) = self
                .ctx
                .method_functions
                .get(&(type_name, interned_method))?;
            (*fid, abi.params.clone(), abi.return_abi.passing.clone())
        };

        let raw_args = [lhs, rhs];
        let passed_args = self.apply_param_passing(&raw_args, &params);

        let result = match &ret_passing {
            ReturnPassing::Direct | ReturnPassing::Void => {
                self.builder.call(func_id, &passed_args, "eq_trait")
            }
            ReturnPassing::Sret { .. } => {
                // equals() returns bool — should always be Direct
                self.builder.call(func_id, &passed_args, "eq_trait")
            }
        }?;

        if negate {
            Some(self.builder.not(result, "neq"))
        } else {
            Some(result)
        }
    }

    /// Emit `<`, `>`, `<=`, `>=` via `Comparable.compare()` + ordering check.
    ///
    /// `compare(self, other)` returns `Ordering` (i8): 0=Less, 1=Equal, 2=Greater.
    fn emit_ordering_comparison(
        &mut self,
        op: BinaryOp,
        lhs: ValueId,
        rhs: ValueId,
        lhs_ty: Idx,
    ) -> Option<ValueId> {
        let type_name = *self.ctx.type_idx_to_name.get(&lhs_ty)?;
        let interned_method = self.interner.intern("compare");
        let (func_id, params, ret_passing, ret_ty_idx) = {
            let (fid, abi) = self
                .ctx
                .method_functions
                .get(&(type_name, interned_method))?;
            (
                *fid,
                abi.params.clone(),
                abi.return_abi.passing.clone(),
                abi.return_abi.ty,
            )
        };

        let raw_args = [lhs, rhs];
        let passed_args = self.apply_param_passing(&raw_args, &params);

        let ordering = match &ret_passing {
            ReturnPassing::Sret { .. } => {
                let ret_ty = self.resolve_type(ret_ty_idx);
                self.call_with_sret(func_id, &passed_args, ret_ty, "cmp_trait")?
            }
            ReturnPassing::Direct | ReturnPassing::Void => {
                self.builder.call(func_id, &passed_args, "cmp_trait")?
            }
        };

        // Ordering is i8: 0=Less, 1=Equal, 2=Greater
        // Map comparison operators to equality/inequality checks on the ordering value.
        let less = self.builder.const_i8(0);
        let greater = self.builder.const_i8(2);
        let result = match op {
            BinaryOp::Lt => self.builder.icmp_eq(ordering, less, "lt"),
            BinaryOp::Gt => self.builder.icmp_eq(ordering, greater, "gt"),
            BinaryOp::LtEq => self.builder.icmp_ne(ordering, greater, "le"),
            BinaryOp::GtEq => self.builder.icmp_ne(ordering, less, "ge"),
            _ => unreachable!("only Lt/Gt/LtEq/GtEq reach here"),
        };
        Some(result)
    }

    /// Dispatch a unary operator to a trait method for non-primitive types.
    ///
    /// Maps the operator to its trait method name (e.g., `-` → `"negate"`),
    /// looks up the compiled method function, and emits a method call.
    fn emit_unary_op_via_trait(
        &mut self,
        op: UnaryOp,
        operand: ValueId,
        operand_ty: Idx,
    ) -> Option<ValueId> {
        let method_name = op.trait_method_name()?;
        let type_name = *self.ctx.type_idx_to_name.get(&operand_ty)?;
        let interned_method = self.interner.intern(method_name);
        let (func_id, params, ret_passing, ret_ty_idx) = {
            let (fid, abi) = self
                .ctx
                .method_functions
                .get(&(type_name, interned_method))?;
            (
                *fid,
                abi.params.clone(),
                abi.return_abi.passing.clone(),
                abi.return_abi.ty,
            )
        };

        let raw_args = [operand];
        let passed_args = self.apply_param_passing(&raw_args, &params);

        match &ret_passing {
            ReturnPassing::Sret { .. } => {
                let ret_ty = self.resolve_type(ret_ty_idx);
                self.call_with_sret(func_id, &passed_args, ret_ty, "op_trait")
            }
            ReturnPassing::Direct | ReturnPassing::Void => {
                self.builder.call(func_id, &passed_args, "op_trait")
            }
        }
    }

    // -----------------------------------------------------------------------
    // Constructor emission
    // -----------------------------------------------------------------------

    /// Emit a `Construct` instruction.
    fn emit_construct(&mut self, ty: Idx, ctor: &CtorKind, args: &[ArcVarId]) -> ValueId {
        let arg_vals: Vec<ValueId> = args.iter().map(|a| self.var(*a)).collect();
        let llvm_ty = self.resolve_type(ty);

        match ctor {
            CtorKind::Struct(_) | CtorKind::Tuple => {
                // Build a struct value from fields
                self.builder.build_struct(llvm_ty, &arg_vals, "ctor")
            }

            CtorKind::EnumVariant { variant, .. } => {
                // Enum layout is { i64 tag, [M x i64] payload } where M is
                // sized for the largest variant. Fields are stored at i64-
                // aligned slots within the payload array.
                // Use alloca + GEP + store; mem2reg eliminates the alloca.
                let tag_val = self.builder.const_i64(i64::from(*variant));
                let alloca = self.builder.alloca(llvm_ty, "variant");
                let tag_gep = self.builder.struct_gep(llvm_ty, alloca, 0, "variant.tag");
                self.builder.store(tag_val, tag_gep);
                if !arg_vals.is_empty() {
                    let payload_ptr =
                        self.builder
                            .struct_gep(llvm_ty, alloca, 1, "variant.payload");
                    let i64_ty = self.builder.i64_type();

                    // Look up variant field types for recursive field detection.
                    let resolved_enum = self.pool.resolve_fully(ty);
                    let variant_field_types = if self.pool.tag(resolved_enum) == Tag::Enum {
                        let all_variants = self.pool.enum_variants(resolved_enum);
                        all_variants
                            .get(*variant as usize)
                            .map(|(_, fields)| fields.clone())
                            .unwrap_or_default()
                    } else {
                        Vec::new()
                    };

                    for (i, &val) in arg_vals.iter().enumerate() {
                        let idx = self.builder.const_i64(i as i64);
                        let slot = self
                            .builder
                            .gep(i64_ty, payload_ptr, &[idx], "variant.field");

                        let field_ty = variant_field_types.get(i).copied();
                        if field_ty.is_some_and(|ft| is_boxed_enum_field(self.pool, ty, ft)) {
                            // Recursive field: RC-allocate and store pointer.
                            let size = self.element_store_size(ty);
                            let rc_ptr = self.rc_alloc(size, 8);
                            self.builder.store(val, rc_ptr);
                            self.builder.store(rc_ptr, slot);
                        } else {
                            self.builder.store(val, slot);
                        }
                    }
                }
                self.builder.load(llvm_ty, alloca, "variant")
            }

            CtorKind::ListLiteral => {
                // List construction: allocate data, store elements, build struct
                let count = arg_vals.len();
                let type_info = self.type_info.get(ty);
                let elem_idx = match &type_info {
                    super::type_info::TypeInfo::List { element } => *element,
                    _ => ori_types::Idx::INT,
                };
                let elem_llvm_ty = self.resolve_type(elem_idx);
                let elem_size = self.element_store_size(elem_idx);

                let cap_val = self.builder.const_i64(count as i64);
                let esize_val = self.builder.const_i64(elem_size as i64);

                let alloc_fn = self.builder.runtime_fn("ori_list_alloc_data");
                let data_ptr = self
                    .builder
                    .call(alloc_fn, &[cap_val, esize_val], "list.data")
                    .unwrap_or_else(|| self.builder.const_null_ptr());

                // Store each element into the data buffer
                for (i, &val) in arg_vals.iter().enumerate() {
                    let idx = self.builder.const_i64(i as i64);
                    let elem_ptr =
                        self.builder
                            .gep(elem_llvm_ty, data_ptr, &[idx], "list.elem_ptr");
                    self.builder.store(val, elem_ptr);
                }

                // Build list struct: {i64 len, i64 cap, ptr data}
                self.builder
                    .build_struct(llvm_ty, &[cap_val, cap_val, data_ptr], "list")
            }

            CtorKind::MapLiteral => {
                // Map literal: args are [key0, val0, key1, val1, ...]
                let count = arg_vals.len() / 2;
                let type_info = self.type_info.get(ty);
                let (key_idx, val_idx) = match &type_info {
                    super::type_info::TypeInfo::Map { key, value } => (*key, *value),
                    _ => (Idx::INT, Idx::INT),
                };
                let key_llvm_ty = self.resolve_type(key_idx);
                let val_llvm_ty = self.resolve_type(val_idx);
                let key_size = self.element_store_size(key_idx);
                let val_size = self.element_store_size(val_idx);

                let count_val = self.builder.const_i64(count as i64);
                let key_esize = self.builder.const_i64(key_size as i64);
                let val_esize = self.builder.const_i64(val_size as i64);

                let alloc_fn = self.builder.runtime_fn("ori_list_alloc_data");

                let keys_ptr = self
                    .builder
                    .call(alloc_fn, &[count_val, key_esize], "map.keys")
                    .unwrap_or_else(|| self.builder.const_null_ptr());

                let vals_ptr = self
                    .builder
                    .call(alloc_fn, &[count_val, val_esize], "map.vals")
                    .unwrap_or_else(|| self.builder.const_null_ptr());

                // Store keys and values into their respective arrays
                for i in 0..count {
                    let idx = self.builder.const_i64(i as i64);
                    let key_ptr = self
                        .builder
                        .gep(key_llvm_ty, keys_ptr, &[idx], "map.key_ptr");
                    self.builder.store(arg_vals[i * 2], key_ptr);

                    let val_ptr = self
                        .builder
                        .gep(val_llvm_ty, vals_ptr, &[idx], "map.val_ptr");
                    self.builder.store(arg_vals[i * 2 + 1], val_ptr);
                }

                // Build map struct: {i64 count, i64 cap, ptr keys, ptr vals}
                self.builder.build_struct(
                    llvm_ty,
                    &[count_val, count_val, keys_ptr, vals_ptr],
                    "map",
                )
            }

            CtorKind::SetLiteral => {
                // Set literal: same layout as list {i64 len, i64 cap, ptr data}
                let count = arg_vals.len();
                let type_info = self.type_info.get(ty);
                let elem_idx = match &type_info {
                    super::type_info::TypeInfo::Set { element } => *element,
                    _ => Idx::INT,
                };
                let elem_llvm_ty = self.resolve_type(elem_idx);
                let elem_size = self.element_store_size(elem_idx);

                let cap_val = self.builder.const_i64(count as i64);
                let esize_val = self.builder.const_i64(elem_size as i64);

                let alloc_fn = self.builder.runtime_fn("ori_list_alloc_data");
                let data_ptr = self
                    .builder
                    .call(alloc_fn, &[cap_val, esize_val], "set.data")
                    .unwrap_or_else(|| self.builder.const_null_ptr());

                for (i, &val) in arg_vals.iter().enumerate() {
                    let idx = self.builder.const_i64(i as i64);
                    let elem_ptr = self
                        .builder
                        .gep(elem_llvm_ty, data_ptr, &[idx], "set.elem_ptr");
                    self.builder.store(val, elem_ptr);
                }

                // Build set struct: {i64 len, i64 cap, ptr data}
                self.builder
                    .build_struct(llvm_ty, &[cap_val, cap_val, data_ptr], "set")
            }

            CtorKind::Closure { .. } => {
                // Closures are always emitted via `PartialApply` in ARC IR,
                // which calls `emit_partial_apply()` → `build_closure_env()`.
                // `Construct { ctor: Closure }` is never produced by the lowerer.
                unreachable!("closures use PartialApply, not Construct")
            }
        }
    }

    // -----------------------------------------------------------------------
    // ABI helpers
    // -----------------------------------------------------------------------

    /// Apply parameter passing modes to argument values.
    ///
    /// Apply param passing: `Indirect`/`Reference` (alloca+store+pass ptr),
    /// `Direct` (pass through), `Void` (skip).
    fn apply_param_passing(
        &mut self,
        args: &[ValueId],
        params: &[super::abi::ParamAbi],
    ) -> Vec<ValueId> {
        let mut result = Vec::with_capacity(args.len());
        let mut arg_idx = 0;

        for param_abi in params {
            if arg_idx >= args.len() {
                break;
            }

            match &param_abi.passing {
                super::abi::ParamPassing::Indirect { .. } | super::abi::ParamPassing::Reference => {
                    let param_ty = self.resolve_type(param_abi.ty);
                    let alloca = self.builder.create_entry_alloca(
                        self.current_function,
                        "ref_arg",
                        param_ty,
                    );
                    self.builder.store(args[arg_idx], alloca);
                    result.push(alloca);
                    arg_idx += 1;
                }
                super::abi::ParamPassing::Direct => {
                    result.push(args[arg_idx]);
                    arg_idx += 1;
                }
                super::abi::ParamPassing::Void => {
                    // Void params are not physically passed — skip
                }
            }
        }

        // Pass remaining args directly (shouldn't happen in well-typed code)
        while arg_idx < args.len() {
            result.push(args[arg_idx]);
            arg_idx += 1;
        }

        result
    }

    /// Call a function with sret (struct return via hidden pointer).
    fn call_with_sret(
        &mut self,
        func_id: FunctionId,
        args: &[ValueId],
        ret_ty: LLVMTypeId,
        name: &str,
    ) -> Option<ValueId> {
        let sret_alloca = self.builder.alloca(ret_ty, "sret.tmp");
        let mut full_args = Vec::with_capacity(1 + args.len());
        full_args.push(sret_alloca);
        full_args.extend_from_slice(args);
        self.builder.call(func_id, &full_args, name);
        Some(self.builder.load(ret_ty, sret_alloca, "sret.load"))
    }

    // -----------------------------------------------------------------------
    // RC data pointer extraction
    // -----------------------------------------------------------------------

    /// Extract the RC-managed data pointer(s) from a value based on its type.
    ///
    /// `ori_rc_inc`/`ori_rc_dec` take raw pointers to RC-allocated heap data.
    /// When a value is an inline aggregate (List, Str, Map, Set, structs with
    /// RC fields), we extract the embedded data pointer(s). For compound types
    /// (struct, tuple, option, result, enum) that contain RC fields, we
    /// recursively extract from each RC'd field.
    ///
    /// | Type   | Layout                         | Data Ptr Field(s)  |
    /// |--------|--------------------------------|--------------------|
    /// | List   | `{i64, i64, ptr}`              | field 2            |
    /// | Set    | `{i64, i64, ptr}`              | field 2            |
    /// | Str    | `{i64, ptr}`                   | field 1            |
    /// | Map    | `{i64, i64, ptr, ptr}`         | field 2, field 3   |
    /// | Struct | `{field0, field1, ...}`         | recurse per field  |
    /// | Tuple  | `{elem0, elem1, ...}`          | recurse per elem   |
    /// | Option | `{i64 tag, T payload}`         | recurse into inner |
    /// | Result | `{i64 tag, payload}`           | recurse into ok/err|
    /// | Enum   | `{i64 tag, payload}`           | recurse into fields|
    /// | Other  | already a ptr                  | use directly        |
    fn extract_rc_data_ptrs(&mut self, val: ValueId, ty: Idx) -> Vec<ValueId> {
        // Resolve type variables and Named/Applied/Alias to get the concrete tag.
        // The type checker may leave unresolved Var indices in compound types
        // (e.g., Option<Var(96)> where Var(96) → int via VarState::Link).
        let resolved = self.pool.resolve_fully(ty);
        let tag = self.pool.tag(resolved);
        match tag {
            Tag::List | Tag::Set => {
                // {i64 len, i64 cap, ptr data} — data at field 2
                if let Some(ptr) = self.builder.extract_value(val, 2, "rc.data_ptr") {
                    vec![ptr]
                } else {
                    vec![val]
                }
            }
            Tag::Str => {
                // {i64 len, ptr data} — data at field 1
                if let Some(ptr) = self.builder.extract_value(val, 1, "rc.data_ptr") {
                    vec![ptr]
                } else {
                    vec![val]
                }
            }
            Tag::Map => {
                // {i64 len, i64 cap, ptr keys, ptr vals} — keys at 2, vals at 3
                let mut ptrs = Vec::with_capacity(2);
                if let Some(keys) = self.builder.extract_value(val, 2, "rc.keys_ptr") {
                    ptrs.push(keys);
                }
                if let Some(vals) = self.builder.extract_value(val, 3, "rc.vals_ptr") {
                    ptrs.push(vals);
                }
                if ptrs.is_empty() {
                    vec![val]
                } else {
                    ptrs
                }
            }
            Tag::Struct => self.extract_rc_from_struct_fields(val, resolved),
            Tag::Tuple => self.extract_rc_from_tuple_elems(val, resolved),
            Tag::Option => {
                // {i8 tag, T payload} — recurse into inner type at field 1
                let inner = self.pool.option_inner(resolved);
                if self.classifier.needs_rc(inner) {
                    if let Some(field) = self.builder.extract_value(val, 1, "rc.opt_inner") {
                        return self.extract_rc_data_ptrs(field, inner);
                    }
                }
                vec![] // scalar option — no RC needed
            }
            Tag::Result => {
                // Result has two possible types; we can't statically know which
                // is active. Skip RC here — the ARC pipeline should handle
                // result fields individually.
                vec![]
            }
            Tag::Enum => {
                // Enum variant tag + payload — can't statically know which
                // variant is active. Skip RC at the aggregate level.
                vec![]
            }
            _ => vec![val],
        }
    }

    /// Extract RC data pointers from a struct's fields.
    fn extract_rc_from_struct_fields(&mut self, val: ValueId, ty: Idx) -> Vec<ValueId> {
        let fields = self.pool.struct_fields(ty);
        let mut ptrs = Vec::new();
        #[expect(
            clippy::cast_possible_truncation,
            reason = "field count bounded by struct definition, well within u32 range"
        )]
        for (i, (_, field_ty)) in fields.into_iter().enumerate() {
            if self.classifier.needs_rc(field_ty) {
                if let Some(field_val) =
                    self.builder
                        .extract_value(val, i as u32, &format!("rc.field.{i}"))
                {
                    ptrs.extend(self.extract_rc_data_ptrs(field_val, field_ty));
                }
            }
        }
        ptrs
    }

    /// Extract RC data pointers from a tuple's elements.
    fn extract_rc_from_tuple_elems(&mut self, val: ValueId, ty: Idx) -> Vec<ValueId> {
        let elems = self.pool.tuple_elems(ty);
        let mut ptrs = Vec::new();
        #[expect(
            clippy::cast_possible_truncation,
            reason = "element count bounded by tuple arity, well within u32 range"
        )]
        for (i, elem_ty) in elems.into_iter().enumerate() {
            if self.classifier.needs_rc(elem_ty) {
                if let Some(elem_val) =
                    self.builder
                        .extract_value(val, i as u32, &format!("rc.elem.{i}"))
                {
                    ptrs.extend(self.extract_rc_data_ptrs(elem_val, elem_ty));
                }
            }
        }
        ptrs
    }

    // -----------------------------------------------------------------------
    // Inline enum/result RcDec
    // -----------------------------------------------------------------------

    /// Emit inline tag-based cleanup for enum-like types (Result, Enum).
    ///
    /// These types are stack-allocated (no RC header) but may contain
    /// RC-typed fields in their variants. We store the value to a
    /// temporary alloca, load the tag, switch on it, and Dec the
    /// appropriate variant's RC fields.
    ///
    /// For `Result<int, str>`: tag 0 (Ok) → nothing; tag 1 (Err) → Dec str.
    fn emit_inline_enum_dec(&mut self, val: ValueId, resolved_ty: Idx, pool_tag: Tag) {
        // Collect per-variant RC field info
        let variant_rc_fields: Vec<Vec<(u32, Idx)>> = match pool_tag {
            Tag::Result => {
                let ok_ty = self.pool.result_ok(resolved_ty);
                let err_ty = self.pool.result_err(resolved_ty);
                let ok_fields = if self.classifier.needs_rc(ok_ty) {
                    vec![(0_u32, ok_ty)]
                } else {
                    vec![]
                };
                let err_fields = if self.classifier.needs_rc(err_ty) {
                    vec![(0_u32, err_ty)]
                } else {
                    vec![]
                };
                vec![ok_fields, err_fields]
            }
            Tag::Enum => {
                let variants = self.pool.enum_variants(resolved_ty);
                variants
                    .iter()
                    .map(|(_, field_tys)| {
                        field_tys
                            .iter()
                            .enumerate()
                            .filter(|(_, ty)| self.classifier.needs_rc(**ty))
                            .map(|(i, ty)| {
                                #[expect(
                                    clippy::cast_possible_truncation,
                                    reason = "variant field index fits u32"
                                )]
                                (i as u32, *ty)
                            })
                            .collect()
                    })
                    .collect()
            }
            _ => return,
        };

        // If no variant has RC fields, nothing to clean up
        if variant_rc_fields.iter().all(Vec::is_empty) {
            return;
        }

        // Store value to alloca so we can use GEP for field access
        let enum_llvm_ty = self.resolve_type(resolved_ty);
        let alloca = self.builder.alloca(enum_llvm_ty, "rc_dec.enum");
        self.builder.store(val, alloca);

        // Load tag (i64 at field 0)
        let i64_ty = self.builder.i64_type();
        let tag_ptr = self
            .builder
            .struct_gep(enum_llvm_ty, alloca, 0, "rc_dec.tag.ptr");
        let tag_val = self.builder.load(i64_ty, tag_ptr, "rc_dec.tag");

        // Convergence block
        let done_block = self
            .builder
            .append_block(self.current_function, "rc_dec.done");

        // Build switch cases for variants with RC fields
        let mut cases = Vec::new();
        for (i, fields) in variant_rc_fields.iter().enumerate() {
            if fields.is_empty() {
                continue;
            }
            let block = self
                .builder
                .append_block(self.current_function, &format!("rc_dec.v{i}"));
            let tag_const = self.builder.const_i64(i as i64);
            cases.push((tag_const, block, fields.as_slice()));
        }

        let switch_cases: Vec<_> = cases.iter().map(|(tag, block, _)| (*tag, *block)).collect();
        self.builder.switch(tag_val, done_block, &switch_cases);

        // Emit per-variant cleanup
        for &(_, block, fields) in &cases {
            self.builder.position_at_end(block);

            for &(field_index, field_type) in fields {
                // Result: typed payload fields at struct index 1+
                // General Enum: payload is [M x i64] at struct field 1
                if pool_tag == Tag::Result {
                    let field_llvm_ty = self.resolve_type(field_type);
                    let struct_idx = 1 + field_index;
                    let field_ptr = self.builder.struct_gep(
                        enum_llvm_ty,
                        alloca,
                        struct_idx,
                        "rc_dec.payload.ptr",
                    );
                    let field_val = self
                        .builder
                        .load(field_llvm_ty, field_ptr, "rc_dec.payload");
                    self.dec_value_rc(field_val, field_type);
                } else if is_boxed_enum_field(self.pool, resolved_ty, field_type) {
                    // Recursive field: stored as RC pointer in the payload.
                    let payload_ptr =
                        self.builder
                            .struct_gep(enum_llvm_ty, alloca, 1, "rc_dec.payload");
                    let i64_ty = self.builder.i64_type();
                    let idx = self.builder.const_i64(i64::from(field_index));
                    let field_ptr =
                        self.builder
                            .gep(i64_ty, payload_ptr, &[idx], "rc_dec.field.ptr");
                    let ptr_ty = self.builder.ptr_type();
                    let rc_ptr = self.builder.load(ptr_ty, field_ptr, "rc_dec.field.rc");
                    let drop_fn = self.get_or_generate_drop_fn(field_type);
                    self.call_rc_dec_all(&[rc_ptr], drop_fn);
                } else {
                    let field_llvm_ty = self.resolve_type(field_type);
                    let payload_ptr =
                        self.builder
                            .struct_gep(enum_llvm_ty, alloca, 1, "rc_dec.payload");
                    let i64_ty = self.builder.i64_type();
                    let idx = self.builder.const_i64(i64::from(field_index));
                    let field_ptr =
                        self.builder
                            .gep(i64_ty, payload_ptr, &[idx], "rc_dec.field.ptr");
                    let field_val = self.builder.load(field_llvm_ty, field_ptr, "rc_dec.field");
                    self.dec_value_rc(field_val, field_type);
                }
            }

            self.builder.br(done_block);
        }

        self.builder.position_at_end(done_block);
    }

    // -----------------------------------------------------------------------
    // List take (sret helper for for-yield finalization)
    // -----------------------------------------------------------------------

    /// Emit `ori_list_take(list_ptr, out_ptr)` with manual sret handling.
    ///
    /// `ori_list_take` uses an explicit sret pattern: `void(ptr list, ptr out)`.
    /// We alloca a `{i64, i64, ptr}` result, call the function, then load.
    fn emit_list_take(&mut self, list_var: ArcVarId, _func: &ArcFunction) -> Option<ValueId> {
        let func_id = self.builder.runtime_fn("ori_list_take");
        let list_ptr = self.var(list_var);

        // Alloca {i64, i64, ptr} for the result
        let list_struct_ty = self.builder.register_type(
            self.builder
                .scx()
                .type_struct(
                    &[
                        self.builder.scx().type_i64().into(),
                        self.builder.scx().type_i64().into(),
                        self.builder.scx().type_ptr().into(),
                    ],
                    false,
                )
                .into(),
        );
        let out_alloca = self.builder.create_entry_alloca(
            self.current_function,
            "list_take.out",
            list_struct_ty,
        );

        // Call ori_list_take(list_ptr, out_alloca) — void return
        self.builder
            .call(func_id, &[list_ptr, out_alloca], "list_take");

        // Load the result struct from the alloca
        Some(
            self.builder
                .load(list_struct_ty, out_alloca, "list_take.val"),
        )
    }

    // -----------------------------------------------------------------------
    // Aggregate-to-pointer coercion
    // -----------------------------------------------------------------------

    // Catch(expr:) EH helpers

    /// Emit `ori_catch_cleanup(exc_ptr)` to free a caught Rust exception.
    ///
    /// Calls `_Unwind_DeleteException` via the runtime wrapper, which invokes
    /// the cleanup callback in the Itanium ABI `_Unwind_Exception` header.
    /// This properly frees the Rust-allocated panic payload without requiring
    /// C++ EH ABI functions (`__cxa_begin_catch`/`__cxa_end_catch`), which
    /// are incompatible with Rust's panic infrastructure.
    ///
    /// Called in catch-style unwind blocks right after the landing pad,
    /// before any RC cleanup or catch handler logic.
    fn emit_catch_cleanup(&mut self, exc_ptr: ValueId) {
        let func_id = self.builder.runtime_fn("ori_catch_cleanup");
        self.builder.call(func_id, &[exc_ptr], "");
    }

    /// Coerce an aggregate value to a pointer for runtime function calls.
    ///
    /// Runtime functions like `ori_print` expect `ptr` arguments (pointers to
    /// structs), but ARC IR passes aggregate values directly. When we detect
    /// that a call arg is an aggregate but the callee expects `ptr`, we
    /// alloca+store+pass the pointer.
    fn coerce_aggregate_to_ptr(&mut self, val: ValueId, ty: Idx) -> ValueId {
        let tag = self.pool.tag(ty);
        match tag {
            Tag::Str | Tag::List | Tag::Set | Tag::Map => {
                let llvm_ty = self.resolve_type(ty);
                let alloca =
                    self.builder
                        .create_entry_alloca(self.current_function, "rt_arg", llvm_ty);
                self.builder.store(val, alloca);
                alloca
            }
            _ => val,
        }
    }

    /// Coerce any value (including scalars) to a pointer via alloca+store.
    ///
    /// Unlike `coerce_aggregate_to_ptr` which only handles struct types,
    /// this works for ALL types. Used by `ori_list_push` which needs a
    /// `*const u8` pointer to any element's bytes.
    fn coerce_any_to_ptr(&mut self, val: ValueId, ty: Idx) -> ValueId {
        let llvm_ty = self.resolve_type(ty);
        let alloca = self
            .builder
            .create_entry_alloca(self.current_function, "elem_arg", llvm_ty);
        self.builder.store(val, alloca);
        alloca
    }

    // -----------------------------------------------------------------------
    // String runtime call helpers
    // -----------------------------------------------------------------------

    // -----------------------------------------------------------------------
    // hash_combine (free function)
    // -----------------------------------------------------------------------

    /// Emit `hash_combine(a, b)` inline using the boost `hash_combine` pattern.
    ///
    /// `a ^ (b + 0x9e3779b9 + (a << 6) + (a >> 2))`
    pub(crate) fn emit_hash_combine(&mut self, a: ValueId, b: ValueId) -> ValueId {
        let magic = self.builder.const_i64(0x9e37_79b9_i64);
        let six = self.builder.const_i64(6);
        let two = self.builder.const_i64(2);

        let a_shl6 = self.builder.shl(a, six, "hc.shl");
        let a_shr2 = self.builder.ashr(a, two, "hc.shr");
        let sum1 = self.builder.add(b, magic, "hc.sum1");
        let sum2 = self.builder.add(sum1, a_shl6, "hc.sum2");
        let sum3 = self.builder.add(sum2, a_shr2, "hc.sum3");
        self.builder.xor(a, sum3, "hc.result")
    }

    // -----------------------------------------------------------------------
    // Format call decomposition
    // -----------------------------------------------------------------------

    /// Intercept `ori_format_*` calls and decompose the string spec argument.
    ///
    /// ARC IR emits `Apply("ori_format_int", [val, spec_str])` with 2 args.
    /// Runtime expects `ori_format_int(val, spec_ptr, spec_len)` — 3 args.
    /// The `spec_str` is `{i64 len, ptr data}` that needs decomposition.
    fn try_emit_format_call(
        &mut self,
        callee_name: &str,
        args: &[ArcVarId],
        func: &ArcFunction,
    ) -> Option<ValueId> {
        if args.len() < 2 {
            return None;
        }

        let func_id = match callee_name {
            "ori_format_int" => self.builder.runtime_fn("ori_format_int"),
            "ori_format_float" => self.builder.runtime_fn("ori_format_float"),
            "ori_format_str" => self.builder.runtime_fn("ori_format_str"),
            "ori_format_bool" => self.builder.runtime_fn("ori_format_bool"),
            "ori_format_char" => self.builder.runtime_fn("ori_format_char"),
            _ => return None,
        };

        // args[0] = the value to format
        let value = self.var(args[0]);
        // args[1] = spec string {i64 len, ptr data}
        let spec_str = self.var(args[1]);

        // For ori_format_str, the value arg is also a string struct — coerce to ptr.
        let value_arg = if callee_name == "ori_format_str" {
            let val_ty = func.var_type(args[0]);
            self.coerce_aggregate_to_ptr(value, val_ty)
        } else {
            value
        };

        // Decompose spec string: extract len (field 0) and data (field 1)
        let spec_len = self.builder.extract_value(spec_str, 0, "fmt.spec_len")?;
        let spec_ptr = self.builder.extract_value(spec_str, 1, "fmt.spec_ptr")?;

        // Call runtime: ori_format_*(value, spec_ptr, spec_len)
        self.builder
            .call(func_id, &[value_arg, spec_ptr, spec_len], "fmt")
    }

    // -----------------------------------------------------------------------
    // String runtime call helpers
    // -----------------------------------------------------------------------

    /// Call a string runtime function: `ori_str_concat`, `ori_str_eq`, `ori_str_ne`.
    ///
    /// String values are `{ i64, ptr }` structs passed by pointer to the runtime.
    /// `returns_str` controls the return type: `true` → `{ i64, ptr }`, `false` → `i1`.
    fn emit_str_runtime_call(
        &mut self,
        func_name: &'static str,
        lhs: ValueId,
        rhs: ValueId,
        returns_str: bool,
    ) -> ValueId {
        let func_id = self.builder.runtime_fn(func_name);

        // Alloca + store both operands (runtime takes pointers to string structs)
        let str_ty = self.resolve_type(ori_types::Idx::STR);
        let lhs_ptr = self
            .builder
            .create_entry_alloca(self.current_function, "str_op.lhs", str_ty);
        self.builder.store(lhs, lhs_ptr);
        let rhs_ptr = self
            .builder
            .create_entry_alloca(self.current_function, "str_op.rhs", str_ty);
        self.builder.store(rhs, rhs_ptr);

        let result = self.builder.call(func_id, &[lhs_ptr, rhs_ptr], func_name);

        if returns_str {
            // ori_str_concat returns { i64, ptr } — load it from the alloca
            result.unwrap_or_else(|| {
                tracing::warn!("ArcIrEmitter: string runtime call returned no value");
                self.builder.const_i64(0)
            })
        } else {
            // ori_str_eq / ori_str_ne return i1 (bool)
            result.unwrap_or_else(|| self.builder.const_bool(false))
        }
    }
}

#[cfg(test)]
mod tests;
