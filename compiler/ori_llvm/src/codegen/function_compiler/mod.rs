//! Two-pass function compilation for V2 codegen.
//!
//! `FunctionCompiler` implements the declare-then-define pattern:
//!
//! 1. **Phase 1 (declare)**: Walk all functions, compute `FunctionAbi` from
//!    `ori_types::FunctionSig`, declare LLVM functions with correct types,
//!    calling conventions, and attributes (sret, noalias).
//!
//! 2. **Phase 2 (define)**: Walk all functions again, lower through the ARC
//!    pipeline (`CanExpr` → ARC IR → `ArcIrEmitter` → LLVM IR).
//!
//! Submodules:
//! - [`define_phase`]: Function body definition (Phase 2) and ARC processing
//! - [`nounwind`]: Two-pass nounwind analysis (prepare → analyze → emit)
//! - [`impls`]: Impl method, test, and derived trait compilation
//! - [`entry_point`]: AOT `main()` wrapper and panic trampoline

mod define_phase;
mod entry_point;
mod impls;
mod nounwind;

pub use nounwind::PreparedFunction;

use std::cell::Cell;

use ori_arc::{AnnotatedSig, ArcClassifier, MemoryContract, UniquenessSummary};
use ori_ir::{Function, Name, Span, StringInterner};
use ori_types::{FunctionSig, Idx, Pool};
use rustc_hash::FxHashMap;
use tracing::{debug, trace, warn};

use crate::aot::debug::DebugContext;
use crate::aot::mangle::Mangler;

use super::abi::{
    compute_function_abi_with_ownership, CallConv, FunctionAbi, ParamPassing, ReturnPassing,
};
use super::arc_emitter::CodegenContext;
use super::ir_builder::IrBuilder;
use super::type_info::{TypeInfoStore, TypeLayoutResolver};
use super::value_id::{FunctionId, LLVMTypeId, ValueId};

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
    /// Shared function-resolution lookup tables passed to [`ArcIrEmitter`].
    codegen_ctx: CodegenContext,
    /// Module-wide lambda counter for unique lambda function names.
    lambda_counter: Cell<u32>,
    /// Borrow inference results: function `Name` → annotated signature.
    /// `Ownership::Borrowed` + non-Scalar parameters use
    /// `ParamPassing::Reference` (pointer, no RC at call site).
    annotated_sigs: &'a FxHashMap<Name, AnnotatedSig>,
    /// Type classifier for ARC analysis (scalar vs ref classification).
    arc_classifier: &'a ArcClassifier<'tcx>,
    /// Debug info context (None for JIT, Some for AOT with debug info enabled).
    debug_context: Option<&'a DebugContext<'ctx>>,
    /// Interprocedural uniqueness summaries (unused — AIMS computes internally).
    uniqueness_summaries: FxHashMap<Name, UniquenessSummary>,
    /// Pre-computed AIMS interprocedural contracts for param/arg ownership.
    /// Populated by [`ori_arc::compute_aims_contracts`] before the per-function loop.
    aims_contracts: FxHashMap<Name, MemoryContract>,
    /// Whether to run ARC IR verification in release builds.
    /// In debug builds, verification always runs regardless of this flag.
    verify_arc: bool,
}

impl<'a, 'scx: 'ctx, 'ctx, 'tcx> FunctionCompiler<'a, 'scx, 'ctx, 'tcx> {
    /// Create a new function compiler.
    ///
    /// `module_path` determines name mangling: `""` for the root module,
    /// `"math"` or `"data/utils"` for nested modules. All LLVM symbols
    /// are mangled (e.g., `add` → `_ori_add`, `math.add` → `_ori_math$add`).
    ///
    /// `annotated_sigs` and `arc_classifier` drive borrow-aware ABI:
    /// `Borrowed` + non-Scalar parameters use `Reference` passing
    /// (pointer, no RC at call site).
    pub fn new(
        builder: &'a mut IrBuilder<'scx, 'ctx>,
        type_info: &'a TypeInfoStore<'tcx>,
        type_resolver: &'a TypeLayoutResolver<'a, 'scx, 'ctx>,
        interner: &'a StringInterner,
        pool: &'tcx Pool,
        module_path: &'a str,
        annotated_sigs: &'a FxHashMap<Name, AnnotatedSig>,
        arc_classifier: &'a ArcClassifier<'tcx>,
        debug_context: Option<&'a DebugContext<'ctx>>,
        uniqueness_summaries: FxHashMap<Name, UniquenessSummary>,
        aims_contracts: FxHashMap<Name, MemoryContract>,
        verify_arc: bool,
    ) -> Self {
        Self {
            builder,
            type_info,
            type_resolver,
            interner,
            pool,
            mangler: Mangler::new(),
            module_path,
            codegen_ctx: CodegenContext::default(),
            lambda_counter: Cell::new(0),
            annotated_sigs,
            arc_classifier,
            debug_context,
            uniqueness_summaries,
            aims_contracts,
            verify_arc,
        }
    }

    // Phase 1: Declare

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
    pub(super) fn declare_function_llvm(&mut self, symbol: &str, abi: &FunctionAbi) -> FunctionId {
        self.declare_function_llvm_with_extra_params(symbol, abi, &[])
    }

    /// Declare an LLVM function with optional extra leading params before ABI params.
    ///
    /// `extra_leading_params` are inserted after the sret pointer (if any) but
    /// before the ABI-derived parameters. Used by non-capturing lambdas to
    /// prepend a phantom `ptr %_env` parameter that makes the declaration
    /// compatible with the closure calling convention `(env_ptr, user_args...)`.
    fn declare_function_llvm_with_extra_params(
        &mut self,
        symbol: &str,
        abi: &FunctionAbi,
        extra_leading_params: &[LLVMTypeId],
    ) -> FunctionId {
        let mut llvm_param_types =
            Vec::with_capacity(abi.params.len() + extra_leading_params.len() + 1);

        let return_llvm_type = self.type_resolver.resolve(abi.return_abi.ty);
        let return_llvm_id = self.builder.register_type(return_llvm_type);

        if matches!(abi.return_abi.passing, ReturnPassing::Sret { .. }) {
            llvm_param_types.push(self.builder.ptr_type());
        }

        llvm_param_types.extend_from_slice(extra_leading_params);

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
            // sret pointer is a fresh caller alloca — safe for noalias.
            // Do NOT add noalias to regular ptr params — RC buffers can alias
            // (e.g., `f(a: xs, b: xs)`). Only sret, ori_rc_alloc returns, and
            // COW StaticUnique call sites qualify.
            self.builder.add_noalias_attribute(func_id, 0);
        }

        // uwtable: required for stack unwinding on all EH-capable targets.
        // See `ir_builder/attributes.rs` for full rationale.
        self.builder.add_uwtable_attribute(func_id);

        // §02.1: noundef on all Direct params/returns — Ori values are always
        // fully defined. Direct params are ≤16 bytes passed by value (registers);
        // both scalars and small aggregates are fully initialized by Ori's type
        // system. Indirect/Reference params are pointers — noundef does not apply.
        let mut nidx = u32::from(matches!(abi.return_abi.passing, ReturnPassing::Sret { .. }))
            + extra_leading_params.len() as u32;
        for param in &abi.params {
            if matches!(param.passing, ParamPassing::Direct) {
                self.builder.add_noundef_param_attribute(func_id, nidx);
                nidx += 1;
            } else if !matches!(param.passing, ParamPassing::Void) {
                nidx += 1;
            }
        }
        if matches!(abi.return_abi.passing, ReturnPassing::Direct) {
            self.builder.add_noundef_return_attribute(func_id);
        }

        func_id
    }

    /// Declare a function with an explicit LLVM symbol name.
    ///
    /// Computes ABI from signature, delegates to [`Self::declare_function_llvm`]
    /// for LLVM-level declaration, then attaches debug info and registers the
    /// function for internal lookup.
    pub(super) fn declare_function_with_symbol(
        &mut self,
        name: Name,
        symbol: &str,
        sig: &FunctionSig,
        span: Span,
    ) {
        let name_str = self.interner.lookup(name);

        let abi = compute_function_abi_with_ownership(
            sig,
            self.type_info,
            self.annotated_sigs.get(&name),
            self.arc_classifier,
        );

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

        self.codegen_ctx.functions.insert(name, (func_id, abi));
    }

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
                    // IrBuilder::load() auto-decomposes struct types via
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
    pub fn declare_imports(&mut self, imports: &[(Name, FunctionSig)]) {
        for (name, sig) in imports {
            self.declare_function(*name, sig, Span::DUMMY);
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
        self.codegen_ctx.type_idx_to_name.get(&idx).copied()
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
}

#[cfg(test)]
#[expect(
    clippy::doc_markdown,
    clippy::default_trait_access,
    reason = "test code — style relaxed for clarity"
)]
mod tests;
