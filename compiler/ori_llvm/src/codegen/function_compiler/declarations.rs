//! Phase-one function and impl-method declarations.

use ori_ir::{Function, Name, Span};
use ori_types::FunctionSig;
use tracing::{debug, trace};

use crate::codegen::abi::{
    abi_size, compute_function_abi_with_ownership, CallConv, FunctionAbi, ParamPassing,
    ReturnPassing,
};
use crate::codegen::value_id::{FunctionId, LLVMTypeId};

use super::FunctionCompiler;

impl<'scx: 'ctx, 'ctx> FunctionCompiler<'_, 'scx, 'ctx, '_> {
    /// Declare all module functions from type checker signatures.
    ///
    /// Iterates over `module.functions` paired with their `FunctionSig` from the
    /// type checker. Generic functions are skipped (they require monomorphization).
    pub fn declare_all(&mut self, module_functions: &[Function], function_sigs: &[FunctionSig]) {
        // Build a name→sig lookup. function_sigs is deduped by name (one per
        // unique function). Multi-clause functions have multiple entries in
        // module_functions but only one sig — lookup is by name to avoid
        // positional misalignment.
        let sig_map: rustc_hash::FxHashMap<Name, &FunctionSig> =
            function_sigs.iter().map(|s| (s.name, s)).collect();

        let mut seen = rustc_hash::FxHashSet::default();
        for func in module_functions {
            // Skip duplicate clause declarations (multi-clause functions).
            if !seen.insert(func.name) {
                continue;
            }

            let Some(sig) = sig_map.get(&func.name) else {
                continue;
            };

            // Skip generic functions
            if sig.requires_specialization() {
                trace!(
                    name = %self.interner.lookup(func.name),
                    "skipping source-template function declaration"
                );
                continue;
            }

            self.declare_function(func.name, sig, func.span);
        }

        self.declare_error_constructor();
    }

    /// Declare a single function from its type checker signature.
    ///
    /// The LLVM symbol uses the mangled name (e.g., `_ori_add`), while the
    /// `functions` map key uses the interned `Name` for internal lookups.
    pub(super) fn declare_function(&mut self, name: Name, sig: &FunctionSig, span: Span) {
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
    pub(super) fn declare_function_llvm_with_extra_params(
        &mut self,
        symbol: &str,
        abi: &FunctionAbi,
        extra_leading_params: &[LLVMTypeId],
    ) -> FunctionId {
        let mut llvm_param_types =
            Vec::with_capacity(abi.params.len() + extra_leading_params.len() + 1);

        let return_llvm_type = self.type_resolver.resolve_boundary(abi.return_abi.ty);
        let return_llvm_id = self.builder.register_type(return_llvm_type);

        if matches!(abi.return_abi.passing, ReturnPassing::Sret { .. }) {
            llvm_param_types.push(self.builder.ptr_type());
        }

        llvm_param_types.extend_from_slice(extra_leading_params);

        for param in &abi.params {
            match &param.passing {
                ParamPassing::Direct => {
                    let ty = self.type_resolver.resolve_boundary(param.ty);
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

        // `uwtable` is required for stack unwinding on every EH-capable target.
        self.builder.add_uwtable_attribute(func_id);

        // noundef on all non-Void params — Ori values are always fully defined.
        // Direct params: the value itself is noundef (no poison/undef).
        // Indirect/Reference params: the pointer is noundef (always a valid,
        // defined address — never poison or undef). Also add readonly for borrowed.
        //
        // Extra leading params (e.g., phantom env ptr for non-capturing lambdas)
        // also get noundef — a null pointer is still a defined value (not undef/poison).
        let sret_offset = u32::from(matches!(abi.return_abi.passing, ReturnPassing::Sret { .. }));
        for (i, _) in extra_leading_params.iter().enumerate() {
            self.builder
                .add_noundef_param_attribute(func_id, sret_offset + i as u32);
        }
        let mut nidx = sret_offset + extra_leading_params.len() as u32;
        for param in &abi.params {
            if matches!(param.passing, ParamPassing::Direct) {
                self.builder.add_noundef_param_attribute(func_id, nidx);
                nidx += 1;
            } else if !matches!(param.passing, ParamPassing::Void) {
                // Indirect/Reference pointer params: noundef (pointer is defined),
                // nonnull (Ori never passes null pointers), dereferenceable(N)
                // (pointer points to at least N bytes of valid memory),
                // + readonly if borrowed.
                self.builder.add_noundef_param_attribute(func_id, nidx);
                self.builder.add_nonnull_param_attribute(func_id, nidx);
                // dereferenceable(N): abi_size may underestimate due to missing
                // alignment padding; underestimation is legal — LLVM treats
                // dereferenceable as a minimum guarantee.
                let size = abi_size(param.ty, self.type_info, self.repr_plan());
                if size > 0 {
                    self.builder
                        .add_dereferenceable_param_attribute(func_id, nidx, size);
                }
                if param.readonly {
                    self.builder.add_readonly_param_attribute(func_id, nidx);
                }
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
        let (func_id, abi) = self.declare_impl_method(name, symbol, sig, span);
        self.codegen_ctx
            .functions
            .insert(name, (func_id, abi.clone()));

        self.declare_length_projection_clone(name, &abi);
    }

    pub(super) fn declare_length_projection_clone(&mut self, name: Name, abi: &FunctionAbi) {
        let Some(&(clone_name, _)) = self.length_projection_clones.get(&name) else {
            return;
        };
        if self.codegen_ctx.functions.contains_key(&clone_name) {
            return;
        }
        let clone_symbol = self
            .mangler
            .mangle_function(self.module_path, self.interner.lookup(clone_name));
        let clone_function = self.declare_function_llvm(&clone_symbol, abi);
        self.builder.set_internal_linkage(clone_function);
        self.codegen_ctx
            .functions
            .insert(clone_name, (clone_function, abi.clone()));
        for (&site, &callee) in &self.length_projection_calls {
            if callee == name {
                self.codegen_ctx
                    .length_projection_call_targets
                    .insert(site, clone_name);
            }
        }
    }

    /// Declare an impl method LLVM function without registering it in the bare
    /// `functions` map.
    ///
    /// Impl methods must be resolved via the type-qualified `method_functions`
    /// map (keyed by `(type_name, method_name)`). Inserting them into the bare
    /// `functions` map (keyed by `method_name` alone) causes wrong-function calls:
    /// when `Box$to_str` is registered under the bare key `to_str`, a later call
    /// to `to_str` on an `int` field inside `Box$to_str`'s body resolves to the
    /// struct method instead of the primitive method.
    ///
    /// Use this for all impl methods (inherent and trait). The caller is
    /// responsible for inserting into `method_functions` and `type_idx_to_name`.
    pub(super) fn declare_impl_method(
        &mut self,
        name: Name,
        symbol: &str,
        sig: &FunctionSig,
        span: Span,
    ) -> (FunctionId, FunctionAbi) {
        self.declare_impl_method_with_fact_name(name, name, symbol, sig, span)
    }

    /// Declare an impl method whose realized callable facts use a distinct,
    /// type-qualified identity.
    ///
    /// `source_name` remains the user-facing method name used for diagnostics
    /// and debug information. `fact_name` selects the exact ownership contract
    /// frozen into the bound executable artifact.
    pub(super) fn declare_impl_method_with_fact_name(
        &mut self,
        source_name: Name,
        fact_name: Name,
        symbol: &str,
        sig: &FunctionSig,
        span: Span,
    ) -> (FunctionId, FunctionAbi) {
        let name_str = self.interner.lookup(source_name);

        let abi = compute_function_abi_with_ownership(
            sig,
            self.type_info,
            self.annotated_sigs.get(&fact_name),
            self.arc_classifier,
            self.repr_plan(),
        );

        debug!(
            name = name_str,
            symbol,
            params = abi.params.len(),
            call_conv = ?abi.call_conv,
            return_passing = ?abi.return_abi.passing,
            "declaring impl method"
        );

        let func_id = self.declare_function_llvm(symbol, &abi);

        if let Some(dc) = self.debug_context {
            if span != Span::DUMMY {
                match dc.create_function_at_offset(name_str, span.start) {
                    Ok(subprogram) => {
                        let func_val = self.builder.get_function_value(func_id);
                        dc.builder().attach_function(func_val, subprogram);
                    }
                    Err(err) => {
                        // Debug info is best-effort; the function still
                        // compiles, but the miss must be visible.
                        tracing::warn!(name = name_str, ?err, "debug info attachment failed");
                    }
                }
            }
        }

        (func_id, abi)
    }
}
