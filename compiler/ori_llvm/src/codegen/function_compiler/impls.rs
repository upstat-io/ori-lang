//! Impl method, test, and derived trait compilation.
//!
//! Impl methods use the immediate-emit path ([`FunctionCompiler::emit_arc_function`])
//! rather than the two-pass nounwind pipeline. They are compiled **before** the
//! two-pass batch to ensure they are available for call-site resolution.

use ori_arc::lower_function_can;
use ori_ir::canon::CanonResult;
use ori_ir::{Name, Span, TestDef, TraitDef, TraitItem};
use ori_types::{FunctionSig, Idx};
use rustc_hash::{FxHashMap, FxHashSet};
use tracing::{debug, trace};

use super::FunctionCompiler;
use crate::codegen::abi::{CallConv, FunctionAbi, ReturnAbi, ReturnPassing};
use crate::codegen::value_id::{FunctionId, ValueId};

impl<'scx: 'ctx, 'ctx> FunctionCompiler<'_, 'scx, 'ctx, '_> {
    /// Compile test definitions as wrapper functions.
    ///
    /// On platforms with Itanium EH (Linux, macOS), each test produces two
    /// layers:
    /// 1. **Inner body** (`_ori_test_<name>_body`): the actual test code compiled
    ///    through the full ARC pipeline.
    /// 2. **Outer wrapper** (`_ori_test_<name>`): uses `invoke` to call the inner
    ///    body with a catch-all `landingpad`. Uncaught panics are caught here
    ///    and stored via `ori_catch_cleanup` so the JIT runner can read the
    ///    panic message.
    ///
    /// On Windows JIT (MSVC target with Itanium EH model), the LLVM JIT cannot
    /// compile Itanium-style `landingpad` for an MSVC target, so we emit a
    /// single function without the invoke/landingpad wrapper. The JIT runner
    /// uses `jit_run_protected` (C++ try/catch) for panic recovery instead.
    ///
    /// Returns a map of `test_name → wrapper_function_name` for the JIT to call.
    pub fn compile_tests(
        &mut self,
        tests: &[&TestDef],
        canon: &CanonResult,
    ) -> FxHashMap<Name, String> {
        let mut test_wrappers = FxHashMap::default();

        // On Windows JIT, landingpad with Itanium EH on an MSVC target causes
        // stack overflow during LLVM JIT compilation. Skip the invoke wrapper.
        let use_invoke_wrapper = !(self.builder.is_jit() && cfg!(target_os = "windows"));

        for test in tests {
            let test_name_str = self.interner.lookup(test.name);
            let wrapper_name = self
                .mangler
                .mangle_function(self.module_path, &format!("test_{test_name_str}"));

            debug!(name = test_name_str, wrapper = %wrapper_name, "compiling test");

            let body = canon.root_for(test.name).unwrap_or(canon.root);

            let abi = FunctionAbi {
                params: vec![],
                return_abi: ReturnAbi {
                    ty: Idx::UNIT,
                    passing: ReturnPassing::Void,
                },
                call_conv: CallConv::C,
            };

            if use_invoke_wrapper {
                let body_name = format!("{wrapper_name}_body");

                // --- Inner body function (the actual test code) ---
                let body_func_id = self.builder.declare_void_function(&body_name, &[]);
                self.builder.set_ccc(body_func_id);
                self.builder.set_current_function(body_func_id);

                let mut problems = Vec::new();
                let (arc_func, lambdas) = lower_function_can(
                    test.name,
                    &[],
                    Idx::UNIT,
                    body,
                    canon,
                    self.interner,
                    self.pool,
                    &mut problems,
                    false,
                    None,
                );

                self.emit_arc_function(test.name, body_func_id, &abi, arc_func, lambdas);

                // --- Outer wrapper with catch-all exception handling ---
                let outer_func_id = self.builder.declare_void_function(&wrapper_name, &[]);
                self.builder.set_ccc(outer_func_id);
                self.builder.set_current_function(outer_func_id);

                let eh_model = self.builder.eh_model();
                let personality_name = eh_model.personality_name();
                let personality_id = self.builder.runtime_fn(personality_name);
                self.builder.set_personality(outer_func_id, personality_id);

                let entry_block = self.builder.append_block(outer_func_id, "entry");
                let normal_block = self.builder.append_block(outer_func_id, "normal");
                let catch_block = self.builder.append_block(outer_func_id, "catch");

                self.builder.position_at_end(entry_block);
                self.builder
                    .invoke(body_func_id, &[], normal_block, catch_block, "");

                self.builder.position_at_end(normal_block);
                self.builder.ret_void();

                self.builder.position_at_end(catch_block);
                let lp = self.builder.landingpad_catch_all(personality_id, "lp.test");
                if let Some(exc_ptr) = self.builder.extract_value(lp, 0, "exc.ptr") {
                    let cleanup_fn = self.builder.runtime_fn("ori_catch_cleanup");
                    self.builder.call(cleanup_fn, &[exc_ptr], "");
                }
                self.builder.ret_void();
            } else {
                // Windows JIT: single function, no invoke/landingpad wrapper.
                // Panic recovery handled by jit_run_protected on the Rust side.
                let func_id = self.builder.declare_void_function(&wrapper_name, &[]);
                self.builder.set_ccc(func_id);
                self.builder.set_current_function(func_id);

                let mut problems = Vec::new();
                let (arc_func, lambdas) = lower_function_can(
                    test.name,
                    &[],
                    Idx::UNIT,
                    body,
                    canon,
                    self.interner,
                    self.pool,
                    &mut problems,
                    false,
                    None,
                );

                self.emit_arc_function(test.name, func_id, &abi, arc_func, lambdas);
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
    /// Methods are inserted ONLY into `method_functions` (`(type_name, method_name)` key),
    /// NOT into the bare `functions` map. This prevents name collisions where a bare
    /// lookup for `to_str` inside `Box$to_str` would find itself instead of the
    /// correct `int$to_str` (BUG-04-003).
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
        // Declare the LLVM function but do NOT insert into the bare `functions`
        // map. Impl methods must be resolved only through the type-qualified
        // `method_functions` map to prevent wrong-callee dispatch: registering
        // `Box$to_str` under the bare key `to_str` would cause any unresolved
        // `to_str` call (e.g., on an `int` field inside `Box$to_str`) to
        // incorrectly resolve to the struct method.
        let (func_id, abi) = self.declare_impl_method(method_name, &symbol, sig, method_span);

        // Populate type-qualified method map for dispatch
        if let Some(tnn) = type_name_name {
            self.codegen_ctx
                .method_functions
                .insert((tnn, method_name), (func_id, abi.clone()));

            // Map the self type Idx → type Name for receiver resolution
            if let Some(&self_type_idx) = sig.param_types.first() {
                self.codegen_ctx.type_idx_to_name.insert(self_type_idx, tnn);
            }

            // Verify round-trip: what we registered is immediately retrievable.
            debug_assert!(
                self.codegen_ctx
                    .method_functions
                    .contains_key(&(tnn, method_name)),
                "method_functions registration failed for {}.{}",
                self.interner.lookup(tnn),
                self.interner.lookup(method_name),
            );
            if let Some(&self_type_idx) = sig.param_types.first() {
                debug_assert!(
                    self.codegen_ctx
                        .type_idx_to_name
                        .contains_key(&self_type_idx),
                    "type_idx_to_name registration failed for '{}' (Idx {:?})",
                    self.interner.lookup(tnn),
                    self_type_idx,
                );
            }
        }

        // Look up the canonical body for this impl method
        let body = type_name_name
            .and_then(|tnn| canon.method_root_for(tnn, method_name))
            .or_else(|| canon.root_for(method_name))
            .unwrap_or(canon.root);

        self.define_function_body(method_name, func_id, &abi, body, canon, sig.is_fbip);
    }

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
        super::super::derive_codegen::compile_derives(self, module, user_types);
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

        self.codegen_ctx
            .method_functions
            .insert((type_name, method_name), (func_id, abi.clone()));
        self.codegen_ctx
            .type_idx_to_name
            .insert(type_idx, type_name);

        // Verify round-trip: registrations are immediately retrievable.
        debug_assert!(
            self.codegen_ctx
                .method_functions
                .contains_key(&(type_name, method_name)),
            "derive: method_functions registration failed for {}.{}",
            self.interner.lookup(type_name),
            self.interner.lookup(method_name),
        );
        debug_assert!(
            self.codegen_ctx.type_idx_to_name.contains_key(&type_idx),
            "derive: type_idx_to_name registration failed for '{}' (Idx {:?})",
            self.interner.lookup(type_name),
            type_idx,
        );

        (func_id, self_value, other_vals)
    }
}
