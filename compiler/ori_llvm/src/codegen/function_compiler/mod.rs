//! Two-pass function compilation for V2 codegen.
//!
//! `FunctionCompiler` implements the declare-then-define pattern:
//!
//! 1. **Phase 1 (declare)**: Walk all functions, compute `FunctionAbi` from
//!    `ori_types::FunctionSig`, declare LLVM functions with correct types,
//!    calling conventions, and attributes (sret, noalias).
//!
//! 2. **Phase 2 (define)**: Walk all functions again, create `ExprLowerer`
//!    for each, bind parameters to scope, lower body expression, emit return.
//!
//! This replaces `ModuleCompiler::compile_function_with_sig()` and
//! `compile_test()` with ABI-driven compilation that gets calling conventions
//! and sret handling correct from the start.

use std::cell::Cell;

use inkwell::types::BasicTypeEnum;
use ori_arc::{lower_function_can, AnnotatedSig, ArcClassifier};
use ori_ir::canon::{CanId, CanonResult};
use ori_ir::{Function, Name, Span, StringInterner, TestDef, TraitDef, TraitItem};
use ori_types::{FunctionSig, Idx, Pool};
use rustc_hash::{FxHashMap, FxHashSet};
use tracing::{debug, trace, warn};

use crate::aot::debug::{DebugContext, DebugLevel};
use crate::aot::mangle::Mangler;

use super::abi::{
    compute_function_abi, compute_function_abi_with_ownership, CallConv, FunctionAbi, ParamPassing,
    ReturnPassing,
};
use super::arc_emitter::ArcIrEmitter;
use super::expr_lowerer::ExprLowerer;
use super::ir_builder::IrBuilder;
use super::scope::Scope;
use super::type_info::{TypeInfoStore, TypeLayoutResolver};
use super::value_id::{FunctionId, LLVMTypeId, ValueId};

// ---------------------------------------------------------------------------
// FunctionCompiler
// ---------------------------------------------------------------------------

/// Two-pass function compiler.
///
/// Holds the mapping from function `Name` → `(FunctionId, FunctionAbi)`,
/// enabling call sites to look up the callee's ABI for correct argument
/// passing (direct vs. sret).
pub struct FunctionCompiler<'a, 'scx, 'ctx, 'tcx> {
    builder: &'a mut IrBuilder<'scx, 'ctx>,
    type_info: &'a TypeInfoStore<'tcx>,
    type_resolver: &'a TypeLayoutResolver<'a, 'scx, 'ctx>,
    interner: &'a StringInterner,
    pool: &'tcx Pool,
    /// Symbol mangler for generating unique LLVM symbol names.
    mangler: Mangler,
    /// Module path for name mangling (e.g., "", "math", "data/utils").
    module_path: &'a str,
    /// Declared functions: `Name` → (`FunctionId`, ABI).
    functions: FxHashMap<Name, (FunctionId, FunctionAbi)>,
    /// Type-qualified method lookup: `(type_name, method_name)` → (`FunctionId`, ABI).
    ///
    /// Allows same-name methods on different types (e.g., `Point.distance` and
    /// `Line.distance`) to coexist without collision. Populated by `compile_impls`.
    method_functions: FxHashMap<(Name, Name), (FunctionId, FunctionAbi)>,
    /// Maps receiver type `Idx` → type `Name` for method dispatch.
    ///
    /// Used by `ExprLowerer` to resolve `expr_type(receiver)` to a type name
    /// for lookup in `method_functions`. Populated by `compile_impls` using
    /// `FunctionSig.param_types[0]` (the self parameter type).
    type_idx_to_name: FxHashMap<Idx, Name>,
    /// Module-wide lambda counter for unique lambda function names.
    lambda_counter: Cell<u32>,
    /// Borrow inference results: function `Name` → annotated signature.
    /// When present, `Ownership::Borrowed` + non-Scalar parameters use
    /// `ParamPassing::Reference` (pointer, no RC at call site).
    annotated_sigs: Option<&'a FxHashMap<Name, AnnotatedSig>>,
    /// Type classifier for ARC analysis (scalar vs ref classification).
    /// Required when `annotated_sigs` is present.
    arc_classifier: Option<&'a ArcClassifier<'tcx>>,
    /// Debug info context (None for JIT, Some for AOT with debug info enabled).
    debug_context: Option<&'a DebugContext<'ctx>>,
    /// When `true`, use Tier 2 ARC codegen path (ARC IR → LLVM IR with RC).
    /// When `false` (default), use Tier 1 (`ExprLowerer` → LLVM IR, no RC).
    use_arc_codegen: bool,
}

impl<'a, 'scx: 'ctx, 'ctx, 'tcx> FunctionCompiler<'a, 'scx, 'ctx, 'tcx> {
    /// Create a new function compiler.
    ///
    /// `module_path` determines name mangling: `""` for the root module,
    /// `"math"` or `"data/utils"` for nested modules. All LLVM symbols
    /// are mangled (e.g., `add` → `_ori_add`, `math.add` → `_ori_math$add`).
    ///
    /// `annotated_sigs` and `arc_classifier` enable borrow-aware ABI:
    /// when present, `Borrowed` + non-Scalar parameters use `Reference`
    /// passing (pointer, no RC at call site). Pass `None` for both to
    /// use standard size-based passing.
    pub fn new(
        builder: &'a mut IrBuilder<'scx, 'ctx>,
        type_info: &'a TypeInfoStore<'tcx>,
        type_resolver: &'a TypeLayoutResolver<'a, 'scx, 'ctx>,
        interner: &'a StringInterner,
        pool: &'tcx Pool,
        module_path: &'a str,
        annotated_sigs: Option<&'a FxHashMap<Name, AnnotatedSig>>,
        arc_classifier: Option<&'a ArcClassifier<'tcx>>,
        debug_context: Option<&'a DebugContext<'ctx>>,
    ) -> Self {
        Self {
            builder,
            type_info,
            type_resolver,
            interner,
            pool,
            mangler: Mangler::new(),
            module_path,
            functions: FxHashMap::default(),
            method_functions: FxHashMap::default(),
            type_idx_to_name: FxHashMap::default(),
            lambda_counter: Cell::new(0),
            annotated_sigs,
            arc_classifier,
            debug_context,
            use_arc_codegen: false,
        }
    }

    /// Enable Tier 2 ARC codegen for all functions compiled through this instance.
    ///
    /// Requires `arc_classifier` to be set (passed in constructor). When enabled,
    /// `define_function_body` runs the full ARC pipeline (lower → borrow → liveness
    /// → RC insert → detect/expand reuse → RC eliminate → `ArcIrEmitter`) instead
    /// of the Tier 1 `ExprLowerer` path.
    pub fn set_arc_codegen(&mut self, enabled: bool) {
        self.use_arc_codegen = enabled;
    }

    // -----------------------------------------------------------------------
    // Phase 1: Declare
    // -----------------------------------------------------------------------

    /// Declare all module functions from type checker signatures.
    ///
    /// Iterates over `module.functions` paired with their `FunctionSig` from the
    /// type checker. Generic functions are skipped (they require monomorphization).
    pub fn declare_all(&mut self, module_functions: &[Function], function_sigs: &[FunctionSig]) {
        for (func, sig) in module_functions.iter().zip(function_sigs.iter()) {
            // Skip generic functions
            if sig.is_generic() {
                trace!(
                    name = %self.interner.lookup(func.name),
                    "skipping generic function declaration"
                );
                continue;
            }

            self.declare_function(func.name, sig, func.span);
        }
    }

    /// Declare a single function from its type checker signature.
    ///
    /// The LLVM symbol uses the mangled name (e.g., `_ori_add`), while the
    /// `functions` map key uses the interned `Name` for internal lookups.
    fn declare_function(&mut self, name: Name, sig: &FunctionSig, span: Span) {
        let name_str = self.interner.lookup(name);
        let symbol = self.mangler.mangle_function(self.module_path, name_str);
        self.declare_function_with_symbol(name, &symbol, sig, span);
    }

    /// Declare an LLVM function from pre-computed ABI and symbol name.
    ///
    /// Shared core for function declaration: builds LLVM parameter types
    /// (sret pointer, direct, indirect/reference), declares the function
    /// (direct vs void return), sets calling convention, and applies sret
    /// attributes. Callers handle ABI computation, debug info, and registration.
    fn declare_function_llvm(&mut self, symbol: &str, abi: &FunctionAbi) -> FunctionId {
        let mut llvm_param_types = Vec::with_capacity(abi.params.len() + 1);

        let return_llvm_type = self.type_resolver.resolve(abi.return_abi.ty);
        let return_llvm_id = self.builder.register_type(return_llvm_type);

        if matches!(abi.return_abi.passing, ReturnPassing::Sret { .. }) {
            llvm_param_types.push(self.builder.ptr_type());
        }

        for param in &abi.params {
            match &param.passing {
                ParamPassing::Direct => {
                    let ty = self.type_resolver.resolve(param.ty);
                    llvm_param_types.push(self.builder.register_type(ty));
                }
                ParamPassing::Indirect { .. } | ParamPassing::Reference => {
                    llvm_param_types.push(self.builder.ptr_type());
                }
                ParamPassing::Void => {}
            }
        }

        let func_id = match &abi.return_abi.passing {
            ReturnPassing::Direct => {
                self.builder
                    .declare_function(symbol, &llvm_param_types, return_llvm_id)
            }
            ReturnPassing::Sret { .. } | ReturnPassing::Void => self
                .builder
                .declare_void_function(symbol, &llvm_param_types),
        };

        match abi.call_conv {
            CallConv::Fast => self.builder.set_fastcc(func_id),
            CallConv::C => self.builder.set_ccc(func_id),
        }

        if let ReturnPassing::Sret { .. } = &abi.return_abi.passing {
            self.builder.add_sret_attribute(func_id, 0, return_llvm_id);
            self.builder.add_noalias_attribute(func_id, 0);
        }

        func_id
    }

    /// Declare a function with an explicit LLVM symbol name.
    ///
    /// Computes ABI from signature, delegates to [`Self::declare_function_llvm`]
    /// for LLVM-level declaration, then attaches debug info and registers the
    /// function for internal lookup.
    fn declare_function_with_symbol(
        &mut self,
        name: Name,
        symbol: &str,
        sig: &FunctionSig,
        span: Span,
    ) {
        let name_str = self.interner.lookup(name);

        let abi = match (self.annotated_sigs, self.arc_classifier) {
            (Some(sigs), Some(classifier)) => {
                let annotated = sigs.get(&name);
                compute_function_abi_with_ownership(sig, self.type_info, annotated, classifier)
            }
            _ => compute_function_abi(sig, self.type_info),
        };

        debug!(
            name = name_str,
            symbol,
            params = abi.params.len(),
            call_conv = ?abi.call_conv,
            return_passing = ?abi.return_abi.passing,
            "declaring function"
        );

        let func_id = self.declare_function_llvm(symbol, &abi);

        if let Some(dc) = self.debug_context {
            if span != Span::DUMMY {
                if let Ok(subprogram) = dc.create_function_at_offset(name_str, span.start) {
                    let func_val = self.builder.get_function_value(func_id);
                    dc.di().attach_function(func_val, subprogram);
                }
            }
        }

        self.functions.insert(name, (func_id, abi));
    }

    // -----------------------------------------------------------------------
    // Phase 2: Define
    // -----------------------------------------------------------------------

    /// Define all module function bodies.
    ///
    /// Must be called after `declare_all()`. For each non-generic function,
    /// creates an `ExprLowerer`, binds parameters, lowers the body, and emits
    /// the return instruction.
    pub fn define_all(
        &mut self,
        module_functions: &[Function],
        function_sigs: &[FunctionSig],
        canon: &CanonResult,
    ) {
        for (func, sig) in module_functions.iter().zip(function_sigs.iter()) {
            if sig.is_generic() {
                continue;
            }

            // Look up previously declared function
            let Some(&(func_id, ref abi)) = self.functions.get(&func.name) else {
                warn!(
                    name = %self.interner.lookup(func.name),
                    "function not declared — skipping definition"
                );
                self.builder.record_codegen_error();
                continue;
            };
            let abi = abi.clone();

            // Look up the canonical body for this function
            let body = canon.root_for(func.name).unwrap_or(canon.root);
            self.define_function_body(func.name, func_id, &abi, body, canon);
        }
    }

    /// Define a single function body.
    ///
    /// Dispatches to Tier 1 (`ExprLowerer`) or Tier 2 (`ArcIrEmitter`) based
    /// on `use_arc_codegen`. Both paths produce correct LLVM IR; Tier 2 adds
    /// RC lifecycle operations (`ori_rc_inc`/`ori_rc_dec`).
    fn define_function_body(
        &mut self,
        name: Name,
        func_id: FunctionId,
        abi: &FunctionAbi,
        body: CanId,
        canon: &CanonResult,
    ) {
        if self.use_arc_codegen {
            if let Some(classifier) = self.arc_classifier {
                self.define_function_body_arc(name, func_id, abi, body, canon, classifier);
                return;
            }
            // Fall through to Tier 1 if no classifier available
            warn!("ARC codegen requested but no classifier — falling back to Tier 1");
        }
        self.define_function_body_tier1(name, func_id, abi, body, canon);
    }

    /// Tier 1: `ExprLowerer`-based codegen (no RC operations).
    fn define_function_body_tier1(
        &mut self,
        name: Name,
        func_id: FunctionId,
        abi: &FunctionAbi,
        body: CanId,
        canon: &CanonResult,
    ) {
        let name_str = self.interner.lookup(name);
        debug!(name = name_str, tier = 1, "defining function body");

        self.enter_debug_scope(func_id);

        // Create entry block
        let entry_block = self.builder.append_block(func_id, "entry");
        self.builder.position_at_end(entry_block);
        self.builder.set_current_function(func_id);

        // Bind parameters to scope
        let scope = self.bind_parameters(func_id, abi);

        // Lower the body expression
        let mut lowerer = ExprLowerer::new(
            self.builder,
            self.type_info,
            self.type_resolver,
            scope,
            canon,
            self.interner,
            self.pool,
            func_id,
            &self.functions,
            &self.method_functions,
            &self.type_idx_to_name,
            &self.lambda_counter,
            self.module_path,
            self.debug_context,
        );

        let result = lowerer.lower(body);

        // Check if the block is already terminated (e.g., by panic, break, unreachable)
        if let Some(block) = self.builder.current_block() {
            if self.builder.block_has_terminator(block) {
                self.exit_debug_scope();
                return;
            }
        }

        // Emit return instruction based on ABI
        self.emit_return(func_id, abi, result, name_str);
        self.exit_debug_scope();
    }

    /// Tier 2: ARC IR → LLVM IR codegen (with RC lifecycle).
    ///
    /// Runs the full ARC pipeline: lower → liveness → RC insert → detect/expand
    /// reuse → RC eliminate → `ArcIrEmitter`. The emitter handles block creation,
    /// parameter binding, and return emission internally.
    fn define_function_body_arc(
        &mut self,
        name: Name,
        func_id: FunctionId,
        abi: &FunctionAbi,
        body: CanId,
        canon: &CanonResult,
        classifier: &ArcClassifier,
    ) {
        let name_str = self.interner.lookup(name);
        debug!(name = name_str, tier = 2, "defining function body (ARC)");

        self.enter_debug_scope(func_id);
        self.builder.set_current_function(func_id);

        // Build parameter list for ARC IR lowering: (Name, Idx) pairs
        let params: Vec<(Name, Idx)> = abi.params.iter().map(|p| (p.name, p.ty)).collect();
        let return_type = abi.return_abi.ty;

        // Step 1: Lower canonical IR → ARC IR
        let mut problems = Vec::new();
        let (mut arc_func, _lambdas) = lower_function_can(
            name,
            &params,
            return_type,
            body,
            canon,
            self.interner,
            self.pool,
            &mut problems,
        );

        for problem in &problems {
            debug!(?problem, "ARC lowering problem");
        }

        // Step 1.5: Apply borrow inference annotations to ARC IR params.
        // Lowering defaults all params to Ownership::Owned (lower/mod.rs).
        // Without this, RC insertion generates unnecessary RcInc/RcDec for
        // params that borrow inference determined should be Borrowed.
        if let Some(sigs) = self.annotated_sigs {
            if let Some(sig) = sigs.get(&name) {
                for (param, annotated) in arc_func.params.iter_mut().zip(&sig.params) {
                    param.ownership = annotated.ownership;
                }
            }
        }

        // Step 2: Run full ARC pipeline (insert → detect → expand → eliminate)
        let empty_sigs = FxHashMap::default();
        let sigs = self.annotated_sigs.unwrap_or(&empty_sigs);
        ori_arc::run_arc_pipeline(&mut arc_func, classifier, sigs);

        trace!(
            name = name_str,
            blocks = arc_func.blocks.len(),
            "ARC pipeline complete"
        );

        // Step 3: Emit LLVM IR from ARC IR
        let mut emitter = ArcIrEmitter::new(
            self.builder,
            self.type_info,
            self.type_resolver,
            self.interner,
            self.pool,
            func_id,
            &self.functions,
            &self.method_functions,
            &self.type_idx_to_name,
        );
        emitter.emit_function(&arc_func, abi);

        self.exit_debug_scope();
    }

    /// Enter debug scope for the function being compiled.
    fn enter_debug_scope(&self, func_id: FunctionId) {
        if let Some(dc) = self.debug_context {
            let func_val = self.builder.get_function_value(func_id);
            if let Some(subprogram) = func_val.get_subprogram() {
                dc.enter_function(subprogram);
            }
        }
    }

    /// Exit debug scope after function compilation.
    fn exit_debug_scope(&self) {
        if let Some(dc) = self.debug_context {
            dc.exit_function();
        }
    }

    /// Emit the return instruction based on ABI passing mode.
    pub(crate) fn emit_return(
        &mut self,
        func_id: FunctionId,
        abi: &FunctionAbi,
        result: Option<ValueId>,
        name_str: &str,
    ) {
        match &abi.return_abi.passing {
            ReturnPassing::Sret { .. } => {
                if let Some(val) = result {
                    let sret_ptr = self.builder.get_param(func_id, 0);
                    self.builder.store(val, sret_ptr);
                }
                self.builder.ret_void();
            }
            ReturnPassing::Direct => {
                if let Some(val) = result {
                    self.builder.ret(val);
                } else {
                    warn!(name = name_str, "direct return function produced no value");
                    self.builder.record_codegen_error();
                    self.builder.ret_void();
                }
            }
            ReturnPassing::Void => {
                self.builder.ret_void();
            }
        }
    }

    /// Load all parameter values from an LLVM function, respecting ABI passing.
    ///
    /// Returns one `ValueId` per non-Void parameter in ABI order. Direct params
    /// are returned as-is; Indirect/Reference params are loaded from their
    /// pointers. Does not set value names or bind to scope — callers handle that.
    fn load_param_values(&mut self, func_id: FunctionId, abi: &FunctionAbi) -> Vec<ValueId> {
        let has_sret = matches!(abi.return_abi.passing, ReturnPassing::Sret { .. });
        let mut llvm_idx: u32 = u32::from(has_sret);
        let mut values = Vec::with_capacity(abi.params.len());

        for (i, param) in abi.params.iter().enumerate() {
            match &param.passing {
                ParamPassing::Direct => {
                    values.push(self.builder.get_param(func_id, llvm_idx));
                    llvm_idx += 1;
                }
                ParamPassing::Indirect { .. } => {
                    let ptr = self.builder.get_param(func_id, llvm_idx);
                    let ty = self.type_resolver.resolve(param.ty);
                    let ty_id = self.builder.register_type(ty);
                    values.push(self.load_indirect_param(ptr, ty, ty_id, i));
                    llvm_idx += 1;
                }
                ParamPassing::Reference => {
                    let ptr = self.builder.get_param(func_id, llvm_idx);
                    let ty = self.type_resolver.resolve(param.ty);
                    let ty_id = self.builder.register_type(ty);
                    values.push(self.builder.load(ty_id, ptr, &format!("param.{i}")));
                    llvm_idx += 1;
                }
                ParamPassing::Void => {}
            }
        }

        values
    }

    /// Load an Indirect parameter, using per-field GEP for struct types.
    ///
    /// A single `load %LargeStruct, ptr` instruction can trigger stack
    /// corruption in LLVM's JIT at O0 when the aggregate exceeds register
    /// capacity (>16 bytes on x86_64). FastISel mishandles the spill slots
    /// for the large aggregate SSA value, causing subsequent stores to
    /// overwrite unrelated stack data.
    ///
    /// The fix: load each struct field individually via `struct_gep` + `load`,
    /// then reassemble via `insert_value`. Each individual load is ≤16 bytes
    /// and well-handled by FastISel.
    fn load_indirect_param(
        &mut self,
        ptr: ValueId,
        ty: BasicTypeEnum<'ctx>,
        ty_id: LLVMTypeId,
        param_idx: usize,
    ) -> ValueId {
        let BasicTypeEnum::StructType(st) = ty else {
            // Non-struct Indirect: single load (rare — only for very large
            // non-struct types, which don't exist in current Ori).
            return self.builder.load(ty_id, ptr, &format!("param.{param_idx}"));
        };

        let num_fields = st.count_fields();
        let mut agg = self.builder.const_zero(ty);

        for f in 0..num_fields {
            let field_ty = st.get_field_type_at_index(f).expect("field index in range");
            let field_ty_id = self.builder.register_type(field_ty);
            let field_ptr =
                self.builder
                    .struct_gep(ty_id, ptr, f, &format!("param.{param_idx}.f{f}.ptr"));
            let field_val =
                self.builder
                    .load(field_ty_id, field_ptr, &format!("param.{param_idx}.f{f}"));
            agg = self
                .builder
                .insert_value(agg, field_val, f, &format!("param.{param_idx}.s{f}"));
        }

        agg
    }

    /// Bind function parameters to a `Scope`, accounting for sret offset.
    ///
    /// Uses [`Self::load_param_values`] to load raw values, then names them
    /// and binds to the scope. `Reference` parameters are received as pointers
    /// and loaded on entry, so downstream code sees values (not pointers).
    fn bind_parameters(&mut self, func_id: FunctionId, abi: &FunctionAbi) -> Scope {
        let values = self.load_param_values(func_id, abi);
        let mut scope = Scope::new();
        let emit_debug = self
            .debug_context
            .is_some_and(|dc| dc.level() == DebugLevel::Full);

        let mut val_iter = values.into_iter();
        let mut dwarf_arg_no: u32 = 1;

        for param in &abi.params {
            match &param.passing {
                ParamPassing::Void => {
                    dwarf_arg_no += 1;
                }
                _ => {
                    let val = val_iter
                        .next()
                        .expect("load_param_values: one value per non-Void param");
                    let name_str = self.interner.lookup(param.name);
                    self.builder.set_value_name(val, name_str);
                    scope.bind_immutable(param.name, val);

                    if emit_debug {
                        self.emit_param_debug(val, name_str, dwarf_arg_no, param.ty);
                    }
                    dwarf_arg_no += 1;
                }
            }
        }

        scope
    }

    /// Emit `DW_TAG_formal_parameter` debug info for a single parameter.
    fn emit_param_debug(&mut self, val: ValueId, name: &str, arg_no: u32, ty: Idx) {
        let dc = self.debug_context.expect("checked by caller");
        let Some(di_ty) = dc.resolve_debug_type(ty, self.pool) else {
            return;
        };
        let Some(block_id) = self.builder.current_block() else {
            return;
        };
        let block = self.builder.raw_block(block_id);
        let raw_val = self.builder.raw_value(val);
        dc.emit_param_debug_info(raw_val, name, arg_no, di_ty, block);
    }

    // -----------------------------------------------------------------------
    // Tests and Impls
    // -----------------------------------------------------------------------

    /// Compile test definitions as void → void wrapper functions.
    ///
    /// Returns a map of `test_name → wrapper_function_name` for the JIT to call.
    pub fn compile_tests(
        &mut self,
        tests: &[&TestDef],
        canon: &CanonResult,
    ) -> FxHashMap<Name, String> {
        let mut test_wrappers = FxHashMap::default();

        for test in tests {
            let test_name_str = self.interner.lookup(test.name);
            let wrapper_name = self
                .mangler
                .mangle_function(self.module_path, &format!("test_{test_name_str}"));

            debug!(name = test_name_str, wrapper = %wrapper_name, "compiling test");

            // Look up the canonical body for this test
            let body = canon.root_for(test.name).unwrap_or(canon.root);

            // Declare void → void wrapper
            let func_id = self.builder.declare_void_function(&wrapper_name, &[]);
            self.builder.set_fastcc(func_id);

            // Define body
            let entry_block = self.builder.append_block(func_id, "entry");
            self.builder.position_at_end(entry_block);
            self.builder.set_current_function(func_id);

            let mut lowerer = ExprLowerer::new(
                self.builder,
                self.type_info,
                self.type_resolver,
                Scope::new(),
                canon,
                self.interner,
                self.pool,
                func_id,
                &self.functions,
                &self.method_functions,
                &self.type_idx_to_name,
                &self.lambda_counter,
                self.module_path,
                self.debug_context,
            );

            lowerer.lower(body);

            // Ensure terminator
            if let Some(block) = self.builder.current_block() {
                if !self.builder.block_has_terminator(block) {
                    self.builder.ret_void();
                }
            }

            test_wrappers.insert(test.name, wrapper_name);
        }

        test_wrappers
    }

    /// Compile impl block methods.
    ///
    /// Impl methods use type-qualified mangled names: `_ori_[<module>$]<type>$<method>`.
    /// This ensures different types can define methods with the same name without
    /// LLVM symbol collision (e.g., `Point.distance` → `_ori_Point$distance`).
    ///
    /// Methods are inserted into both:
    /// - `functions` (bare `method.name` key, for backward compat)
    /// - `method_functions` (`(type_name, method_name)` key, for type-qualified dispatch)
    ///
    /// `type_idx_to_name` is also populated to map `sig.param_types[0]` (the self
    /// parameter type) to the type name, enabling receiver type → type name resolution
    /// during method call lowering.
    pub fn compile_impls(
        &mut self,
        impls: &[ori_ir::ImplDef],
        impl_sigs: &[(Name, FunctionSig)],
        canon: &CanonResult,
        traits: &[TraitDef],
    ) {
        // Consume impl_sigs positionally — the type checker pushes sigs in the
        // same iteration order: `for impl_def { for method { register_impl_sig } }`,
        // followed by unoverridden default trait methods.
        // A flat HashMap keyed by method Name would lose entries when two types
        // define same-name methods (e.g., Point.distance vs Line.distance).
        let mut sig_iter = impl_sigs.iter();

        // Build trait map for default method lookup
        let trait_map: FxHashMap<Name, &TraitDef> = traits.iter().map(|t| (t.name, t)).collect();

        for impl_def in impls {
            // Resolve the type name from self_path for mangling
            let type_name_name = impl_def.self_path.first().copied();
            let type_name = type_name_name
                .map(|n| self.interner.lookup(n).to_owned())
                .unwrap_or_default();

            for method in &impl_def.methods {
                self.compile_impl_method_from_sig(
                    &mut sig_iter,
                    method.name,
                    method.span,
                    type_name_name,
                    &type_name,
                    canon,
                );
            }

            // For trait impls, compile unoverridden default methods.
            // The type checker registers their sigs in the same order after
            // explicit methods, so sig_iter stays aligned.
            if let Some(trait_path) = &impl_def.trait_path {
                if let Some(&trait_name) = trait_path.last() {
                    if let Some(trait_def) = trait_map.get(&trait_name) {
                        let overridden: FxHashSet<Name> =
                            impl_def.methods.iter().map(|m| m.name).collect();

                        for item in &trait_def.items {
                            if let TraitItem::DefaultMethod(default) = item {
                                if !overridden.contains(&default.name) {
                                    self.compile_impl_method_from_sig(
                                        &mut sig_iter,
                                        default.name,
                                        default.span,
                                        type_name_name,
                                        &type_name,
                                        canon,
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// Compile a single impl method by consuming the next signature from the
    /// positional sig iterator. Used for both explicit methods and default
    /// trait methods.
    fn compile_impl_method_from_sig<'sig>(
        &mut self,
        sig_iter: &mut impl Iterator<Item = &'sig (Name, FunctionSig)>,
        method_name: Name,
        method_span: Span,
        type_name_name: Option<Name>,
        type_name: &str,
        canon: &CanonResult,
    ) {
        let Some((sig_name, sig)) = sig_iter.next() else {
            trace!(
                name = %self.interner.lookup(method_name),
                "no type signature for impl method — exhausted sig iterator"
            );
            return;
        };

        debug_assert_eq!(
            *sig_name, method_name,
            "impl sig/method name mismatch: sig has {sig_name:?}, method has {method_name:?}"
        );

        if sig.is_generic() {
            return;
        }

        // Use type-qualified mangled name for LLVM symbol
        let method_str = self.interner.lookup(method_name);
        let symbol = if type_name.is_empty() {
            self.mangler.mangle_function(self.module_path, method_str)
        } else {
            self.mangler
                .mangle_method(self.module_path, type_name, method_str)
        };
        self.declare_function_with_symbol(method_name, &symbol, sig, method_span);

        let Some(&(func_id, ref abi)) = self.functions.get(&method_name) else {
            return;
        };
        let abi = abi.clone();

        // Populate type-qualified method map for dispatch
        if let Some(tnn) = type_name_name {
            self.method_functions
                .insert((tnn, method_name), (func_id, abi.clone()));

            // Map the self type Idx → type Name for receiver resolution
            if let Some(&self_type_idx) = sig.param_types.first() {
                self.type_idx_to_name.insert(self_type_idx, tnn);
            }
        }

        // Look up the canonical body for this impl method
        let body = type_name_name
            .and_then(|tnn| canon.method_root_for(tnn, method_name))
            .or_else(|| canon.root_for(method_name))
            .unwrap_or(canon.root);

        self.define_function_body(method_name, func_id, &abi, body, canon);
    }

    /// Declare external imported functions (for multi-module AOT compilation).
    pub fn declare_imports(&mut self, imports: &[(Name, FunctionSig)]) {
        for (name, sig) in imports {
            self.declare_function(*name, sig, Span::DUMMY);
        }
    }

    // -----------------------------------------------------------------------
    // AOT Entry Points
    // -----------------------------------------------------------------------

    /// Generate a C-compatible `main()` wrapper that calls the Ori `@main` function.
    ///
    /// The wrapper bridges the C calling convention (`ccc`) to Ori's internal
    /// calling convention (`fastcc`). Four `@main` signatures are supported:
    ///
    /// | Ori signature               | C wrapper                                    |
    /// |-----------------------------|----------------------------------------------|
    /// | `@main () -> void`          | `define i32 @main() { call @_ori_main(); ret 0 }` |
    /// | `@main () -> int`           | `define i32 @main() { trunc call @_ori_main() }` |
    /// | `@main (args) -> void`      | `define i32 @main(i32, ptr) { ... }`         |
    /// | `@main (args) -> int`       | `define i32 @main(i32, ptr) { ... }`         |
    ///
    /// Must be called after `declare_all()` + `define_all()` so the `@main`
    /// function is already compiled. Returns `false` if no `@main` was found.
    pub fn generate_main_wrapper(
        &mut self,
        main_name: Name,
        main_sig: &FunctionSig,
        panic_name: Option<Name>,
    ) -> bool {
        let Some(&(ori_main_id, ref abi)) = self.functions.get(&main_name) else {
            debug!("no @main function declared — skipping entry point wrapper");
            return false;
        };
        let abi = abi.clone();

        // Generate panic trampoline if @panic handler exists
        let panic_trampoline = panic_name.and_then(|name| self.generate_panic_trampoline(name));

        let has_args = !main_sig.param_types.is_empty();
        let returns_int = main_sig.return_type == Idx::INT;

        debug!(
            has_args,
            returns_int,
            has_panic = panic_trampoline.is_some(),
            "generating C main() entry point wrapper"
        );

        // C main signature: i32 @main() or i32 @main(i32 %argc, ptr %argv)
        let i32_ty = self.builder.i32_type();
        let c_main_params = if has_args {
            let ptr_ty = self.builder.ptr_type();
            vec![i32_ty, ptr_ty]
        } else {
            vec![]
        };

        let c_main_id = self
            .builder
            .declare_function("main", &c_main_params, i32_ty);
        self.builder.set_ccc(c_main_id);

        let entry = self.builder.append_block(c_main_id, "entry");
        self.builder.position_at_end(entry);
        self.builder.set_current_function(c_main_id);

        // Register panic handler trampoline if present
        if let Some(trampoline_id) = panic_trampoline {
            let ptr_ty = self.builder.ptr_type();
            let register_fn = self
                .builder
                .get_or_declare_void_function("ori_register_panic_handler", &[ptr_ty]);
            let trampoline_ptr = self.builder.get_function_ptr(trampoline_id);
            self.builder.call(register_fn, &[trampoline_ptr], "");
        }

        // Build args for calling the Ori @main function
        let call_args = if has_args {
            // Call ori_args_from_argv(arg_count, arg_values) → Ori [str]
            let arg_count = self.builder.get_param(c_main_id, 0);
            let arg_values = self.builder.get_param(c_main_id, 1);

            let ptr_ty = self.builder.ptr_type();
            let scx = self.builder.scx();
            let list_struct_ty = scx.type_struct(
                &[
                    scx.type_i64().into(),
                    scx.type_i64().into(),
                    scx.type_ptr().into(),
                ],
                false,
            );
            let list_ty_id = self.builder.register_type(list_struct_ty.into());
            let args_fn = self.builder.get_or_declare_function(
                "ori_args_from_argv",
                &[i32_ty, ptr_ty],
                list_ty_id,
            );
            let args_val = self.builder.call(args_fn, &[arg_count, arg_values], "args");
            if let Some(val) = args_val {
                vec![val]
            } else {
                vec![]
            }
        } else {
            vec![]
        };

        // Call the Ori @main function
        match &abi.return_abi.passing {
            super::abi::ReturnPassing::Direct => {
                let result = self
                    .builder
                    .call(ori_main_id, &call_args, "ori_main_result");
                if returns_int {
                    // Truncate i64 → i32 for C exit code
                    if let Some(val) = result {
                        let exit_code = self.builder.trunc(val, i32_ty, "exit_code");
                        self.builder.ret(exit_code);
                    } else {
                        let zero = self.builder.const_i32(0);
                        self.builder.ret(zero);
                    }
                } else {
                    let zero = self.builder.const_i32(0);
                    self.builder.ret(zero);
                }
            }
            super::abi::ReturnPassing::Void | super::abi::ReturnPassing::Sret { .. } => {
                self.builder.call(ori_main_id, &call_args, "");
                let zero = self.builder.const_i32(0);
                self.builder.ret(zero);
            }
        }

        true
    }

    /// Generate a panic handler trampoline.
    ///
    /// The trampoline bridges the C runtime to the user's `@panic` function:
    /// 1. Receives flat C values from the runtime (msg ptr/len, file ptr/len, line, col)
    /// 2. Constructs the Ori `PanicInfo` struct in LLVM IR
    /// 3. Calls the user's compiled `@panic` function
    ///
    /// Returns `Some(FunctionId)` of the trampoline, or `None` if the `@panic`
    /// function was not declared.
    fn generate_panic_trampoline(&mut self, panic_name: Name) -> Option<FunctionId> {
        let Some(&(user_panic_id, _)) = self.functions.get(&panic_name) else {
            debug!("no @panic function declared — skipping trampoline");
            return None;
        };

        debug!("generating panic handler trampoline");

        let ptr_ty = self.builder.ptr_type();
        let i64_ty = self.builder.i64_type();

        // Trampoline signature: (ptr msg_data, i64 msg_len, ptr file_data, i64 file_len, i64 line, i64 col) -> void
        let trampoline_id = self.builder.declare_void_function(
            "_ori_panic_trampoline",
            &[ptr_ty, i64_ty, ptr_ty, i64_ty, i64_ty, i64_ty],
        );
        self.builder.set_ccc(trampoline_id);

        let entry = self.builder.append_block(trampoline_id, "entry");
        self.builder.position_at_end(entry);
        self.builder.set_current_function(trampoline_id);

        // Extract parameters
        let msg_data = self.builder.get_param(trampoline_id, 0);
        let msg_len = self.builder.get_param(trampoline_id, 1);
        let file_data = self.builder.get_param(trampoline_id, 2);
        let file_len = self.builder.get_param(trampoline_id, 3);
        let line = self.builder.get_param(trampoline_id, 4);
        let col = self.builder.get_param(trampoline_id, 5);

        // Construct PanicInfo struct:
        //   PanicInfo = { str message, TraceEntry location, [TraceEntry] stack_trace, Option<int> thread_id }
        //
        // Where:
        //   str         = { i64 len, ptr data }
        //   TraceEntry  = { str function, str file, int line, int column }
        //                = { {i64, ptr}, {i64, ptr}, i64, i64 }
        //   [TraceEntry] = { i64 len, i64 cap, ptr data }
        //   Option<int>  = { i8 tag, i64 value }

        let scx = self.builder.scx();

        // str type: { i64, ptr }
        let str_struct_ty = scx.type_struct(&[scx.type_i64().into(), scx.type_ptr().into()], false);

        // TraceEntry type: { str, str, i64, i64 }
        let trace_entry_ty = scx.type_struct(
            &[
                str_struct_ty.into(),
                str_struct_ty.into(),
                scx.type_i64().into(),
                scx.type_i64().into(),
            ],
            false,
        );

        // [TraceEntry] type: { i64, i64, ptr }
        let list_ty = scx.type_struct(
            &[
                scx.type_i64().into(),
                scx.type_i64().into(),
                scx.type_ptr().into(),
            ],
            false,
        );

        // Option<int> type: { i8, i64 }
        let option_int_ty = scx.type_struct(&[scx.type_i8().into(), scx.type_i64().into()], false);

        // PanicInfo type: { str, TraceEntry, [TraceEntry], Option<int> }
        let panic_info_ty = scx.type_struct(
            &[
                str_struct_ty.into(),
                trace_entry_ty.into(),
                list_ty.into(),
                option_int_ty.into(),
            ],
            false,
        );

        // Register all types
        let str_ty_id = self.builder.register_type(str_struct_ty.into());
        let trace_entry_ty_id = self.builder.register_type(trace_entry_ty.into());
        let list_ty_id = self.builder.register_type(list_ty.into());
        let option_int_ty_id = self.builder.register_type(option_int_ty.into());
        let panic_info_ty_id = self.builder.register_type(panic_info_ty.into());

        // Build message: str = { msg_len, msg_data }
        let message = self
            .builder
            .build_struct(str_ty_id, &[msg_len, msg_data], "message");

        // Build empty function name: str = { 0, null }
        let zero_i64 = self.builder.const_i64(0);
        let null_ptr = self.builder.const_null_ptr();
        let empty_str = self
            .builder
            .build_struct(str_ty_id, &[zero_i64, null_ptr], "empty_fn");

        // Build file name: str = { file_len, file_data }
        let file_str = self
            .builder
            .build_struct(str_ty_id, &[file_len, file_data], "file");

        // Build location: TraceEntry = { empty_fn, file, line, col }
        let location = self.builder.build_struct(
            trace_entry_ty_id,
            &[empty_str, file_str, line, col],
            "location",
        );

        // Build empty stack_trace: [TraceEntry] = { 0, 0, null }
        let stack_trace =
            self.builder
                .build_struct(list_ty_id, &[zero_i64, zero_i64, null_ptr], "stack_trace");

        // Build thread_id: Option<int> = { 0 (None tag), 0 }
        let zero_i8 = self.builder.const_i8(0);
        let thread_id =
            self.builder
                .build_struct(option_int_ty_id, &[zero_i8, zero_i64], "thread_id");

        // Build PanicInfo = { message, location, stack_trace, thread_id }
        let panic_info = self.builder.build_struct(
            panic_info_ty_id,
            &[message, location, stack_trace, thread_id],
            "panic_info",
        );

        // Call the user's @panic function
        self.builder.call(user_panic_id, &[panic_info], "");

        // Emit ret void (handler returns normally → runtime proceeds with default)
        self.builder.ret_void();

        Some(trampoline_id)
    }

    // -----------------------------------------------------------------------
    // Derived Trait Methods
    // -----------------------------------------------------------------------

    /// Compile derived trait methods for types with `#[derive(...)]`.
    ///
    /// Generates synthetic LLVM functions for derived traits (Eq, Clone,
    /// Hashable, Printable) and registers them in `method_functions` for
    /// normal method dispatch.
    pub fn compile_derives(
        &mut self,
        module: &ori_ir::Module,
        user_types: &[ori_types::TypeEntry],
    ) {
        super::derive_codegen::compile_derives(self, module, user_types);
    }

    /// Declare a derived method LLVM function, create entry block, bind params.
    ///
    /// Delegates to [`Self::declare_function_llvm`] for declaration and
    /// [`Self::load_param_values`] for parameter loading. Registers the method
    /// in `method_functions` and `type_idx_to_name` for dispatch.
    ///
    /// Returns `(func_id, self_value, other_param_values)`.
    pub(crate) fn declare_and_bind_derive(
        &mut self,
        symbol: &str,
        abi: &FunctionAbi,
        type_name: Name,
        method_name: Name,
        type_idx: Idx,
    ) -> (FunctionId, ValueId, Vec<ValueId>) {
        let func_id = self.declare_function_llvm(symbol, abi);

        let entry = self.builder.append_block(func_id, "entry");
        self.builder.position_at_end(entry);
        self.builder.set_current_function(func_id);

        let values = self.load_param_values(func_id, abi);
        let self_value = values
            .first()
            .copied()
            .unwrap_or_else(|| self.builder.const_i64(0));
        let other_vals = values.into_iter().skip(1).collect();

        self.method_functions
            .insert((type_name, method_name), (func_id, abi.clone()));
        self.type_idx_to_name.insert(type_idx, type_name);
        self.functions.insert(method_name, (func_id, abi.clone()));

        (func_id, self_value, other_vals)
    }

    // -----------------------------------------------------------------------
    // Accessors
    // -----------------------------------------------------------------------

    /// Look up a declared function by name.
    pub fn get_function(&self, name: Name) -> Option<&(FunctionId, FunctionAbi)> {
        self.functions.get(&name)
    }

    /// Borrow the function map (for call-site ABI lookup).
    pub fn function_map(&self) -> &FxHashMap<Name, (FunctionId, FunctionAbi)> {
        &self.functions
    }

    /// Borrow the type-qualified method map.
    pub fn method_function_map(&self) -> &FxHashMap<(Name, Name), (FunctionId, FunctionAbi)> {
        &self.method_functions
    }

    /// Borrow the type index → type name mapping.
    pub fn type_idx_to_name_map(&self) -> &FxHashMap<Idx, Name> {
        &self.type_idx_to_name
    }

    // -----------------------------------------------------------------------
    // Derive Codegen Accessors (pub(crate))
    // -----------------------------------------------------------------------

    /// Mutable borrow of the `IrBuilder`.
    pub(crate) fn builder_mut(&mut self) -> &mut IrBuilder<'scx, 'ctx> {
        self.builder
    }

    /// Create an alloca at the function entry block.
    ///
    /// Entry-block placement ensures LLVM's frame lowering accounts for the
    /// alloca during prologue emission. Allocas interleaved with calls can
    /// cause stack corruption in `fastcc` functions at O0 (LLVM FastISel
    /// miscalculates stack adjustments).
    pub(crate) fn entry_alloca(&mut self, ty: LLVMTypeId, name: &str) -> ValueId {
        let func = self
            .builder
            .current_function
            .expect("entry_alloca called without current function");
        self.builder.create_entry_alloca(func, name, ty)
    }

    /// Borrow the type info store.
    pub(crate) fn type_info(&self) -> &TypeInfoStore<'tcx> {
        self.type_info
    }

    /// Resolve a type Idx to its LLVM representation.
    pub(crate) fn resolve_type(&self, idx: Idx) -> inkwell::types::BasicTypeEnum<'ctx> {
        self.type_resolver.resolve(idx)
    }

    /// Look up an interned name.
    pub(crate) fn lookup_name(&self, name: Name) -> &str {
        self.interner.lookup(name)
    }

    /// Intern a string.
    pub(crate) fn intern(&self, s: &str) -> Name {
        self.interner.intern(s)
    }

    /// Generate a mangled method symbol.
    pub(crate) fn mangle_method(&self, type_name: &str, method_name: &str) -> String {
        self.mangler
            .mangle_method(self.module_path, type_name, method_name)
    }

    /// Look up a type name from a type Idx.
    pub(crate) fn type_idx_to_name(&self, idx: Idx) -> Option<Name> {
        self.type_idx_to_name.get(&idx).copied()
    }

    /// Look up a method function by type and method name.
    pub(crate) fn get_method_function(
        &self,
        type_name: Name,
        method_name: Name,
    ) -> Option<(FunctionId, FunctionAbi)> {
        self.method_functions
            .get(&(type_name, method_name))
            .cloned()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(
    clippy::doc_markdown,
    clippy::default_trait_access,
    reason = "test code — style relaxed for clarity"
)]
mod tests;
