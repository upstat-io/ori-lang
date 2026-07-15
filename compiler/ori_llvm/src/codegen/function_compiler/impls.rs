//! Impl method compilation.
//!
//! Impl methods use the immediate-emit path ([`FunctionCompiler::emit_arc_function`])
//! rather than the two-pass nounwind pipeline. They are compiled **before** the
//! two-pass batch to ensure they are available for call-site resolution.

use ori_ir::canon::CanonResult;
use ori_ir::{Name, Span, TraitDef, TraitItem};
use ori_types::ImplSig;
use rustc_hash::{FxHashMap, FxHashSet};
use tracing::{trace, warn};

use super::FunctionCompiler;

#[derive(Copy, Clone, Eq, PartialEq)]
enum ImplCompilePhase {
    DeclareAndEmit,
    DeclareOnly,
    EmitOnly,
}

impl<'scx: 'ctx, 'ctx> FunctionCompiler<'_, 'scx, 'ctx, '_> {
    /// Compile impl block methods.
    ///
    /// Impl methods use type-qualified mangled names: `_ori_[<module>$]<type>$<method>`.
    /// This ensures different types can define methods with the same name without
    /// LLVM symbol collision (e.g., `Point.distance` → `_ori_Point$distance`).
    ///
    /// Methods are inserted ONLY into `method_functions` (`(type_name, method_name)` key),
    /// NOT into the bare `functions` map. This prevents name collisions where a bare
    /// lookup for `to_str` inside `Box$to_str` would find itself instead of the
    /// correct `int$to_str`.
    ///
    /// `type_idx_to_name` is also populated with two keys per impl method: the
    /// self-receiver type (`sig.param_types[0]`, when present) and the owning impl
    /// type (`owning_type_idx`). The self-receiver key enables receiver type → type
    /// name resolution for `self`-method calls; the owning-type key enables
    /// no-receiver associated-function dispatch (`-> Self` / same-owning-type), where
    /// `lookup_method_by_return_type` keys on the return type and the self-receiver
    /// key is absent.
    pub fn compile_impls(
        &mut self,
        impls: &[ori_ir::ImplDef],
        impl_sigs: &[ImplSig],
        canon: &CanonResult,
        traits: &[TraitDef],
    ) {
        self.compile_impls_inner(
            impls,
            impl_sigs,
            canon,
            traits,
            None,
            ImplCompilePhase::DeclareAndEmit,
        );
    }

    /// Compile impl methods against stable bodies in a bound executable artifact.
    ///
    /// `emission_names` is parallel to `impl_sigs`; every non-generic entry
    /// identifies the exact qualified body already realized by AIMS.
    pub fn compile_impls_from_artifact(
        &mut self,
        impls: &[ori_ir::ImplDef],
        impl_sigs: &[ImplSig],
        canon: &CanonResult,
        traits: &[TraitDef],
        emission_names: &[Option<Name>],
    ) {
        if emission_names.len() != impl_sigs.len() {
            self.builder.record_codegen_error_with_msg(format!(
                "closed executable impl-body table has {} entries for {} signatures",
                emission_names.len(),
                impl_sigs.len()
            ));
            return;
        }
        self.compile_impls_inner(
            impls,
            impl_sigs,
            canon,
            traits,
            Some(emission_names),
            ImplCompilePhase::DeclareOnly,
        );
        self.bind_user_drop_targets();
        self.compile_impls_inner(
            impls,
            impl_sigs,
            canon,
            traits,
            Some(emission_names),
            ImplCompilePhase::EmitOnly,
        );
    }

    fn compile_impls_inner(
        &mut self,
        impls: &[ori_ir::ImplDef],
        impl_sigs: &[ImplSig],
        canon: &CanonResult,
        traits: &[TraitDef],
        emission_names: Option<&[Option<Name>]>,
        phase: ImplCompilePhase,
    ) {
        // Consume impl_sigs positionally — the type checker pushes sigs in the
        // same iteration order: `for impl_def { for method { register_impl_sig } }`,
        // followed by unoverridden default trait methods.
        // A flat HashMap keyed by method Name would lose entries when two types
        // define same-name methods (e.g., Point.distance vs Line.distance).
        let mut sig_iter = impl_sigs.iter();
        let mut emission_name_iter = emission_names.map(<[Option<Name>]>::iter);

        // Build trait map for default method lookup
        let trait_map: FxHashMap<Name, &TraitDef> = traits.iter().map(|t| (t.name, t)).collect();

        for impl_def in impls {
            // Resolve the type name for mangling via the canonical accessor:
            // the LAST path segment is the type name (a qualified path's
            // leading segments are its qualifier), matching the eval backend.
            // INVARIANT: self_path is non-empty per the parser's E1002
            // guarantee — a silent empty-name fallback would mangle every
            // method of a corrupted impl to the colliding `_ori_$method`.
            let Some(type_name) = impl_def.type_name() else {
                unreachable!("ImplDef.self_path empty — parser E1002 guarantees a type path")
            };
            let type_name_str = self.interner.lookup(type_name).to_owned();

            for method in &impl_def.methods {
                self.compile_impl_method_from_sig(
                    &mut sig_iter,
                    method.name,
                    method.span,
                    type_name,
                    &type_name_str,
                    canon,
                    &mut emission_name_iter,
                    phase,
                );
            }

            // For trait impls, compile unoverridden default methods.
            // The type checker registers their sigs in the same order after
            // explicit methods, so sig_iter stays aligned.
            self.compile_trait_default_methods_for_impl(
                impl_def,
                type_name,
                &type_name_str,
                &trait_map,
                &mut sig_iter,
                canon,
                &mut emission_name_iter,
                phase,
            );
        }
    }

    /// Compile the unoverridden default methods of any trait this impl
    /// implements. Each `impl Type: Trait { ... }` inherits every default
    /// body from `Trait` that the impl block does not explicitly override;
    /// the type checker has already registered one sig for each inherited
    /// default in positional order AFTER the explicit-method sigs, so
    /// `sig_iter` stays aligned with the compile order used here.
    ///
    /// No-op when `impl_def` is an inherent (non-trait) impl, when the
    /// trait path does not resolve, or when every method is overridden.
    fn compile_trait_default_methods_for_impl<'sig>(
        &mut self,
        impl_def: &ori_ir::ImplDef,
        type_name: Name,
        type_name_str: &str,
        trait_map: &FxHashMap<Name, &TraitDef>,
        sig_iter: &mut impl Iterator<Item = &'sig ImplSig>,
        canon: &CanonResult,
        emission_names: &mut Option<std::slice::Iter<'_, Option<Name>>>,
        phase: ImplCompilePhase,
    ) {
        let Some(trait_path) = &impl_def.trait_path else {
            return;
        };
        let Some(&trait_name) = trait_path.last() else {
            return;
        };
        let Some(trait_def) = trait_map.get(&trait_name) else {
            return;
        };

        let overridden: FxHashSet<Name> = impl_def.methods.iter().map(|m| m.name).collect();

        for item in &trait_def.items {
            let TraitItem::DefaultMethod(default) = item else {
                continue;
            };
            if overridden.contains(&default.name) {
                continue;
            }
            self.compile_impl_method_from_sig(
                sig_iter,
                default.name,
                default.span,
                type_name,
                type_name_str,
                canon,
                emission_names,
                phase,
            );
        }
    }

    /// Compile a single impl method by consuming the next signature from the
    /// positional sig iterator. Used for both explicit methods and default
    /// trait methods.
    fn compile_impl_method_from_sig<'sig>(
        &mut self,
        sig_iter: &mut impl Iterator<Item = &'sig ImplSig>,
        method_name: Name,
        method_span: Span,
        type_name: Name,
        type_name_str: &str,
        canon: &CanonResult,
        emission_names: &mut Option<std::slice::Iter<'_, Option<Name>>>,
        phase: ImplCompilePhase,
    ) {
        // `receiver` is the owning impl receiver — the type-qualified
        // dispatch key for a no-receiver associated call (`Widget.make()`),
        // which has no self param to source the key from.
        let Some(ImplSig {
            receiver: owning_type_idx,
            name: sig_name,
            sig,
            ..
        }) = sig_iter.next()
        else {
            trace!(
                name = %self.interner.lookup(method_name),
                "no type signature for impl method — exhausted sig iterator"
            );
            return;
        };
        let artifact_name = emission_names
            .as_mut()
            .and_then(|names| names.next().copied().flatten());

        debug_assert_eq!(
            *sig_name, method_name,
            "impl sig/method name mismatch: sig has {sig_name:?}, method has {method_name:?}"
        );

        if sig.is_generic() {
            return;
        }

        if self.executable_program.is_some() && artifact_name.is_none() {
            self.builder.record_codegen_error_with_msg(format!(
                "closed executable has no stable impl body for {}.{}",
                type_name_str,
                self.interner.lookup(method_name)
            ));
            return;
        }

        let fact_name = artifact_name.unwrap_or(method_name);
        let (func_id, abi) = if phase == ImplCompilePhase::EmitOnly {
            let Some(declared) = self.codegen_ctx.functions.get(&fact_name).cloned() else {
                self.builder.record_codegen_error_with_msg(format!(
                    "closed executable impl body {} was not declared before emission",
                    self.interner.lookup(fact_name)
                ));
                return;
            };
            declared
        } else {
            // Artifact targets own unique physical identities. Source-level
            // `(type, method)` symbols collide when inherent and trait methods
            // share a name; a bound projection therefore mangles the exact
            // realized target while unbound low-level fixtures retain the
            // source ABI symbol.
            let symbol = artifact_name.map_or_else(
                || {
                    self.mangler.mangle_method(
                        self.module_path,
                        type_name_str,
                        self.interner.lookup(method_name),
                    )
                },
                |target| {
                    self.mangler
                        .mangle_function(self.module_path, self.interner.lookup(target))
                },
            );
            let declared = self.declare_impl_method_with_fact_name(
                method_name,
                fact_name,
                &symbol,
                sig,
                method_span,
            );

            // Populate the legacy type-qualified method map for unbound
            // user-method consumers. Bound direct calls and user-drop glue use
            // exact executable targets instead.
            self.codegen_ctx
                .method_functions
                .insert((type_name, method_name), declared.clone());
            self.codegen_ctx
                .functions
                .insert(fact_name, declared.clone());
            declared
        };

        self.register_impl_dispatch_types(sig, *owning_type_idx, type_name, method_name, phase);

        if phase == ImplCompilePhase::DeclareOnly {
            return;
        }

        let emission = if phase == ImplCompilePhase::EmitOnly {
            let Some((arc_function, lambdas)) = self.clone_bound_family(fact_name, &abi) else {
                return;
            };
            self.enter_debug_scope(func_id);
            self.builder.set_current_function(func_id);
            self.emit_arc_function(fact_name, func_id, &abi, arc_function, lambdas)
        } else {
            let body = canon
                .method_root_for(type_name, method_name)
                .or_else(|| canon.root_for(method_name))
                .unwrap_or(canon.root);
            self.define_function_body(fact_name, func_id, &abi, body, canon, sig.is_fbip)
        };

        // Keep scanning after a per-method projection failure so one bad body
        // does not hide independent diagnostics in the same module.
        if let Err(err) = emission {
            warn!(
                method = %self.interner.lookup(method_name),
                type_name = type_name_str,
                ?err,
                "PC-2 contract violation — skipping impl method"
            );
        }
    }

    /// Register and verify the type keys used by legacy impl-method dispatch.
    fn register_impl_dispatch_types(
        &mut self,
        sig: &ori_types::FunctionSig,
        owning_type_idx: ori_types::Idx,
        type_name: Name,
        method_name: Name,
        phase: ImplCompilePhase,
    ) {
        // The self receiver can differ from the impl receiver after
        // monomorphization. Associated functions have no self parameter, so
        // both identities are required to cover receiver and return dispatch.
        if let Some(&self_type_idx) = sig.param_types.first() {
            self.codegen_ctx
                .type_idx_to_name
                .insert(self_type_idx, type_name);
        }
        self.codegen_ctx
            .type_idx_to_name
            .insert(owning_type_idx, type_name);

        debug_assert!(
            self.codegen_ctx
                .method_functions
                .contains_key(&(type_name, method_name)),
            "method_functions registration failed for {}.{}",
            self.interner.lookup(type_name),
            self.interner.lookup(method_name),
        );

        if phase == ImplCompilePhase::DeclareOnly {
            return;
        }
        if let Some(&self_type_idx) = sig.param_types.first() {
            debug_assert!(
                self.codegen_ctx
                    .type_idx_to_name
                    .contains_key(&self_type_idx),
                "type_idx_to_name registration failed for '{}' (Idx {:?})",
                self.interner.lookup(type_name),
                self_type_idx,
            );
        }
        debug_assert!(
            self.codegen_ctx
                .type_idx_to_name
                .contains_key(&owning_type_idx),
            "type_idx_to_name owning-type registration failed for '{}' (Idx {:?})",
            self.interner.lookup(type_name),
            owning_type_idx,
        );
    }
}
