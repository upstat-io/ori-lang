//! `FunctionCompiler` accessors, debug-scope helpers, and param/return emit
//! helpers.
//!
//! Read-only borrows of the compiler's maps + resolvers, the debug-scope
//! enter/exit pair, and the ABI-aware return / param-load helpers consumed by
//! the sibling emission modules.

use super::{
    warn, FunctionAbi, FunctionCompiler, FunctionId, FxHashMap, Idx, IrBuilder, LLVMTypeId, Name,
    ParamPassing, Pool, ReturnPassing, Span, TypeInfoStore, ValueId,
};
use ori_repr::monomorphize::ImportSig;

impl<'scx: 'ctx, 'ctx, 'tcx> FunctionCompiler<'_, 'scx, 'ctx, 'tcx> {
    /// Enter debug scope for the function being compiled.
    pub(super) fn enter_debug_scope(&self, func_id: FunctionId) {
        if let Some(dc) = self.debug_context {
            let func_val = self.builder.get_function_value(func_id);
            if let Some(subprogram) = func_val.get_subprogram() {
                dc.enter_function(subprogram);
            }
        }
    }

    /// Exit debug scope after function compilation.
    pub(super) fn exit_debug_scope(&self) {
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
    pub(super) fn load_param_values(
        &mut self,
        func_id: FunctionId,
        abi: &FunctionAbi,
    ) -> Vec<ValueId> {
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
                    // IrBuilder::load auto-decomposes struct types via
                    // per-field GEP+load+insert_value (FastISel safety).
                    values.push(self.builder.load(ty_id, ptr, &format!("param.{i}")));
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

    /// Declare external imported functions (for multi-module AOT compilation).
    ///
    /// `name` is the call-site local/aliased Name (the `codegen_ctx.functions`
    /// key `resolve_callee` probes); `symbol` is the exporting module's EXACT
    /// mangled symbol (never re-mangled against the host module path).
    pub fn declare_imports(&mut self, imports: &[ImportSig]) {
        // Several local aliases can share ONE extern symbol (`use { f as g,
        // f as h }`). Declare each symbol once; register every alias Name
        // against the same FunctionId (a second add_function on the same
        // symbol would make LLVM mint a renamed `sym.1` global).
        let mut declared: FxHashMap<&str, (FunctionId, FunctionAbi)> = FxHashMap::default();
        for ImportSig { name, symbol, sig } in imports {
            if let Some(entry) = declared.get(symbol.as_str()) {
                self.codegen_ctx.functions.insert(*name, entry.clone());
            } else {
                self.declare_function_with_symbol(*name, symbol, sig, Span::DUMMY);
                if let Some(entry) = self.codegen_ctx.functions.get(name) {
                    declared.insert(symbol.as_str(), entry.clone());
                }
            }
        }
    }

    // Accessors

    /// Look up a declared function by name.
    pub fn get_function(&self, name: Name) -> Option<&(FunctionId, FunctionAbi)> {
        self.codegen_ctx.functions.get(&name)
    }

    /// Borrow the function map (for call-site ABI lookup).
    pub fn function_map(&self) -> &FxHashMap<Name, (FunctionId, FunctionAbi)> {
        &self.codegen_ctx.functions
    }

    /// Borrow the type-qualified method map.
    pub fn method_function_map(&self) -> &FxHashMap<(Name, Name), (FunctionId, FunctionAbi)> {
        &self.codegen_ctx.method_functions
    }

    /// Borrow the type index → type name mapping.
    pub fn type_idx_to_name_map(&self) -> &FxHashMap<Idx, Name> {
        &self.codegen_ctx.type_idx_to_name
    }

    // Derive Codegen Accessors (pub(crate))

    /// Mutable borrow of the `IrBuilder`.
    pub(crate) fn builder_mut(&mut self) -> &mut IrBuilder<'scx, 'ctx> {
        self.builder
    }

    /// Create an alloca at the function entry block.
    ///
    /// Entry-block placement ensures LLVM's frame lowering accounts for the
    /// alloca during prologue emission. Allocas interleaved with calls can
    /// cause stack corruption in `fastcc` functions at O0 (LLVM `FastISel`
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

    /// Borrow the type pool.
    pub(crate) fn pool(&self) -> &Pool {
        self.pool
    }

    /// Access the repr plan (if present) for element narrowing queries.
    pub(crate) fn repr_plan(&self) -> Option<&ori_repr::ReprPlan> {
        self.type_resolver.repr_plan()
    }

    /// Look up an interned name.
    pub(crate) fn lookup_name(&self, name: Name) -> &str {
        self.interner.lookup_static(name)
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
        self.codegen_ctx.type_idx_to_name.get(&idx).copied()
    }

    /// Whether ARC/LLVM IR verification is enabled (`ORI_VERIFY_ARC=1`).
    pub(crate) fn verify_arc(&self) -> bool {
        self.verify_arc
    }

    /// Look up a method function by type and method name.
    pub(crate) fn get_method_function(
        &self,
        type_name: Name,
        method_name: Name,
    ) -> Option<(FunctionId, FunctionAbi)> {
        self.codegen_ctx
            .method_functions
            .get(&(type_name, method_name))
            .cloned()
    }

    /// Resolve a derived method for a (possibly generic-composite) receiver/field
    /// type `Idx`. Prefers the per-instantiation map keyed by the materialized
    /// concrete `Idx` (`pool.resolve_fully`); falls back to the type-name-keyed
    /// `method_functions` for non-generic types.
    pub(crate) fn get_derived_method_for_type(
        &self,
        type_idx: Idx,
        method_name: Name,
    ) -> Option<(FunctionId, FunctionAbi)> {
        let resolved = self.pool.resolve_fully(type_idx);
        if let Some(hit) = self
            .codegen_ctx
            .mono_derive_functions
            .get(&(resolved, method_name))
        {
            return Some(hit.clone());
        }
        let type_name = self.type_idx_to_name(type_idx)?;
        self.get_method_function(type_name, method_name)
    }

    /// Register a per-instantiation derived method keyed by the materialized
    /// concrete `Idx`. Called by derive codegen after each
    /// generic-composite instantiation's method is emitted.
    pub(crate) fn register_mono_derive_function(
        &mut self,
        concrete_idx: Idx,
        method_name: Name,
        func_id: FunctionId,
        abi: FunctionAbi,
    ) {
        self.codegen_ctx
            .mono_derive_functions
            .insert((concrete_idx, method_name), (func_id, abi));
    }

    /// Map a type `Idx` (concrete instantiation `Applied` or its resolved body)
    /// to a type `Name` so receiver-based method dispatch resolves it.
    pub(crate) fn map_type_idx_to_name(&mut self, idx: Idx, name: Name) {
        self.codegen_ctx.type_idx_to_name.insert(idx, name);
    }
}
