//! Closure wrapper function generation for [`ArcIrEmitter`].
//!
//! Generates `_ori_partial_N` wrapper functions that bridge the closure
//! calling convention `(env_ptr, user_args...)` to the lambda's flat
//! calling convention `(captures..., user_args...)`.

use ori_arc::ownership::Ownership;
use ori_arc::{ClosureAdapterAction, ClosureAdapterPlan, RetainPlanId, RetainPlanKind};
use ori_types::Idx;

use super::context::is_boxed_enum_field;
use super::ArcIrEmitter;
use crate::codegen::abi::{FunctionAbi, ParamAbi, ParamPassing, ReturnPassing};
use crate::codegen::value_id::{FunctionId, ValueId};

struct ClosureWrapperInput<'a> {
    callee_abi: &'a FunctionAbi,
    capture_types: &'a [Idx],
    frozen_adapter: Option<&'a ClosureAdapterPlan>,
    capture_ownership: &'a [Ownership],
    remaining_params: &'a [ParamAbi],
    target_has_phantom_env: bool,
}

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
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
    pub(super) fn generate_closure_wrapper(
        &mut self,
        callee_func_id: FunctionId,
        callee_abi: &FunctionAbi,
        capture_types: &[Idx],
        frozen_adapter: Option<&ClosureAdapterPlan>,
        capture_ownership: &[Ownership],
        remaining_params: &[ParamAbi],
        target_has_phantom_env: bool,
        target_is_nounwind: bool,
    ) -> ValueId {
        let input = ClosureWrapperInput {
            callee_abi,
            capture_types,
            frozen_adapter,
            capture_ownership,
            remaining_params,
            target_has_phantom_env,
        };
        let partial_id = self.partial_apply_counter;
        self.partial_apply_counter += 1;
        let wrapper_name = format!("_ori_partial_{partial_id}");

        // Save builder position
        let saved_pos = self.builder.save_position();
        let saved_func = self.builder.current_function();

        // Determine return type and sret mode
        let ptr_ty = self.builder.ptr_type();
        let ret_ty = self.resolve_boundary_type(callee_abi.return_abi.ty);
        let has_sret = matches!(callee_abi.return_abi.passing, ReturnPassing::Sret { .. });
        let is_void = matches!(callee_abi.return_abi.passing, ReturnPassing::Void);

        // Build wrapper parameter types.
        // When has_sret: [ptr sret_out, ptr env, user_params...]
        // Otherwise:     [ptr env, user_params...]
        let mut wrapper_param_types =
            Vec::with_capacity(usize::from(has_sret) + 1 + remaining_params.len());
        if has_sret {
            wrapper_param_types.push(ptr_ty); // sret output pointer
        }
        wrapper_param_types.push(ptr_ty); // env_ptr
        for param in remaining_params {
            match &param.passing {
                ParamPassing::Direct => {
                    let ty = self.resolve_boundary_type(param.ty);
                    wrapper_param_types.push(ty);
                }
                ParamPassing::Indirect { .. } | ParamPassing::Reference => {
                    wrapper_param_types.push(ptr_ty);
                }
                ParamPassing::Void => {}
            }
        }

        // Declare wrapper function.
        // When has_sret, the wrapper uses explicit sret on its first parameter
        // (matching the lambda's ABI). This is critical on ARM64 where sret
        // goes in X8 — the trampoline's indirect call must agree with both the
        // wrapper and the lambda on sret placement.
        let wrapper_func_id = if is_void || has_sret {
            self.builder
                .declare_void_function(&wrapper_name, &wrapper_param_types)
        } else {
            self.builder
                .declare_function(&wrapper_name, &wrapper_param_types, ret_ty)
        };
        self.builder.set_module_local(wrapper_func_id);
        self.builder.add_uwtable_attribute(wrapper_func_id);
        if target_is_nounwind {
            self.builder.add_nounwind_attribute(wrapper_func_id);
        }

        // Add sret + noalias attributes on the sret parameter.
        if has_sret {
            self.builder.add_sret_attribute(wrapper_func_id, 0, ret_ty);
            self.builder.add_noalias_attribute(wrapper_func_id, 0);
        }

        // noundef on return value — Ori values are always defined.
        if !is_void && !has_sret {
            self.builder.add_noundef_return_attribute(wrapper_func_id);
        }

        // noundef on env pointer and user params — skip the hidden sret
        // pointer (param 0 when has_sret) because sret is a compiler-managed
        // ABI parameter, not a user value. Matches function_compiler.rs which
        // uses sret_offset for the same purpose.
        let sret_offset = u32::from(has_sret);
        #[expect(
            clippy::cast_possible_truncation,
            reason = "wrapper params bounded by lambda arity, well within u32 range"
        )]
        for i in sret_offset..wrapper_param_types.len() as u32 {
            self.builder.add_noundef_param_attribute(wrapper_func_id, i);
        }

        // Generate wrapper body
        let entry = self.builder.append_block(wrapper_func_id, "entry");
        self.builder.position_at_end(entry);
        self.builder.set_current_function(wrapper_func_id);

        // When has_sret: param 0 = sret_out, param 1 = env_ptr
        // Otherwise:     param 0 = env_ptr
        let env_param_idx: u32 = u32::from(has_sret);
        let env_ptr_val = self.builder.get_param(wrapper_func_id, env_param_idx);

        // Set current_function to wrapper so inc_value_rc creates blocks
        // in the right function (it uses self.current_function for append_block).
        let saved_current_function = self.current_function;
        self.current_function = wrapper_func_id;

        let mut callee_args =
            self.unpack_closure_captures(wrapper_func_id, env_ptr_val, has_sret, &input);
        self.forward_closure_params(wrapper_func_id, env_param_idx, &input, &mut callee_args);

        // Call the actual lambda function
        let result = self.builder.call(callee_func_id, &callee_args, "result");

        // Emit return
        if has_sret {
            // Result was written through sret pointer — return void.
            self.builder.ret_void();
        } else if is_void {
            self.builder.ret_void();
        } else if let Some(val) = result {
            self.builder.ret(val);
        } else {
            let zero = self.builder.const_i64(0);
            self.builder.ret(zero);
        }

        // Function-level LLVM IR verification for generated closure wrappers.
        if self.verify_arc {
            let fn_val = self.builder.get_function_value(wrapper_func_id);
            if !fn_val.verify(true) {
                tracing::error!(
                    name = wrapper_name,
                    "LLVM IR verification failed (generate_closure_wrapper)"
                );
                self.builder.record_codegen_error();
            }
        }

        // Restore builder position and emitter's current_function
        self.current_function = saved_current_function;
        self.builder.restore_position(saved_pos);
        if let Some(f) = saved_func {
            self.builder.set_current_function(f);
        }

        self.builder.get_function_ptr(wrapper_func_id)
    }

    fn unpack_closure_captures(
        &mut self,
        wrapper_func_id: FunctionId,
        env_ptr: ValueId,
        has_sret: bool,
        input: &ClosureWrapperInput<'_>,
    ) -> Vec<ValueId> {
        let mut env_fields: Vec<inkwell::types::BasicTypeEnum<'_>> =
            vec![self.builder.scx().type_ptr().into()];
        for &capture_type in input.capture_types {
            env_fields.push(self.type_resolver.resolve(capture_type));
        }
        let env_struct = self.builder.scx().type_struct(&env_fields, false);
        let env_struct_type = self.builder.register_type(env_struct.into());

        let mut callee_args = Vec::with_capacity(
            usize::from(has_sret)
                + usize::from(input.target_has_phantom_env)
                + input.callee_abi.params.len(),
        );
        if has_sret {
            callee_args.push(self.builder.get_param(wrapper_func_id, 0));
        }
        if input.target_has_phantom_env {
            debug_assert!(
                input.capture_types.is_empty(),
                "a non-capturing lambda cannot have closure capture fields"
            );
            callee_args.push(env_ptr);
        }

        #[expect(
            clippy::cast_possible_truncation,
            reason = "capture count bounded by lambda arity, well within u32 range"
        )]
        for (index, &capture_type) in input.capture_types.iter().enumerate() {
            let field_type = self.resolve_type(capture_type);
            let field_ptr = self.builder.struct_gep(
                env_struct_type,
                env_ptr,
                (index + 1) as u32,
                &format!("cap.{index}.ptr"),
            );
            let passing = input
                .callee_abi
                .params
                .get(index)
                .map(|param| &param.passing);
            let frozen_action = input
                .frozen_adapter
                .map(|adapter| adapter.slots()[index].action);
            let needs_legacy_inc = frozen_action.is_none()
                && input
                    .capture_ownership
                    .get(index)
                    .copied()
                    .unwrap_or(Ownership::Owned)
                    == Ownership::Owned
                && self
                    .classifier
                    .has_managed_ownership_obligation(capture_type);

            let pass_by_reference = matches!(
                passing,
                Some(ParamPassing::Indirect { .. } | ParamPassing::Reference)
            );
            let capture = if pass_by_reference {
                if frozen_action
                    .is_some_and(|action| matches!(action, ClosureAdapterAction::Retain(_)))
                    || needs_legacy_inc
                {
                    let loaded =
                        self.builder
                            .load(field_type, field_ptr, &format!("cap.{index}.inc"));
                    self.retain_closure_capture(
                        loaded,
                        capture_type,
                        frozen_action,
                        needs_legacy_inc,
                    );
                }
                field_ptr
            } else {
                let loaded = self
                    .builder
                    .load(field_type, field_ptr, &format!("cap.{index}"));
                self.retain_closure_capture(loaded, capture_type, frozen_action, needs_legacy_inc);
                // The env stores narrowed storage form; the lambda signature is
                // canonical, so the capture widens at the unpack boundary.
                self.widen_to_boundary(loaded, capture_type)
            };
            callee_args.push(capture);
        }
        callee_args
    }

    fn retain_closure_capture(
        &mut self,
        value: ValueId,
        capture_type: Idx,
        frozen_action: Option<ClosureAdapterAction>,
        needs_legacy_inc: bool,
    ) {
        if let Some(ClosureAdapterAction::Retain(plan)) = frozen_action {
            self.emit_frozen_closure_retain_plan(value, plan);
        } else if needs_legacy_inc {
            self.inc_value_rc(value, capture_type, 1);
        }
    }

    fn forward_closure_params(
        &mut self,
        wrapper_func_id: FunctionId,
        env_param_index: u32,
        input: &ClosureWrapperInput<'_>,
        callee_args: &mut Vec<ValueId>,
    ) {
        let mut wrapper_param_index = env_param_index + 1;
        for (residual_index, source_param) in input.remaining_params.iter().enumerate() {
            let target_param = &input.callee_abi.params[input.capture_types.len() + residual_index];
            if source_param.passing == ParamPassing::Void {
                debug_assert_eq!(target_param.passing, ParamPassing::Void);
                continue;
            }

            let incoming = self.builder.get_param(wrapper_func_id, wrapper_param_index);
            let source_is_pointer = matches!(
                source_param.passing,
                ParamPassing::Indirect { .. } | ParamPassing::Reference
            );
            let target_is_pointer = matches!(
                target_param.passing,
                ParamPassing::Indirect { .. } | ParamPassing::Reference
            );
            let retain_plan = input.frozen_adapter.and_then(|adapter| {
                let slot = &adapter.slots()[input.capture_types.len() + residual_index];
                match slot.action {
                    ClosureAdapterAction::Retain(plan) => Some(plan),
                    ClosureAdapterAction::Borrow | ClosureAdapterAction::Copy => None,
                }
            });
            let needs_semantic_value = retain_plan.is_some()
                || target_param.passing == ParamPassing::Direct
                || (!source_is_pointer && target_is_pointer);
            let semantic_value = needs_semantic_value.then(|| {
                if source_is_pointer {
                    let value_type = self.resolve_type(source_param.ty);
                    self.builder
                        .load(value_type, incoming, &format!("arg.{residual_index}.value"))
                } else {
                    incoming
                }
            });

            if let Some(plan) = retain_plan {
                let Some(value) = semantic_value else {
                    // Why: A retain plan makes needs_semantic_value true above.
                    unreachable!("retain bridge requires a semantic value")
                };
                self.emit_frozen_closure_retain_plan(value, plan);
            }

            match target_param.passing {
                ParamPassing::Direct => {
                    let Some(value) = semantic_value else {
                        // Why: Direct target passing makes needs_semantic_value true above.
                        unreachable!("direct target requires a semantic value")
                    };
                    let widened = self.widen_to_boundary(value, target_param.ty);
                    callee_args.push(widened);
                }
                ParamPassing::Indirect { .. } | ParamPassing::Reference => {
                    if source_is_pointer {
                        callee_args.push(incoming);
                    } else {
                        let value_type = self.resolve_type(source_param.ty);
                        let slot = self
                            .builder
                            .alloca(value_type, &format!("arg.{residual_index}.bridge"));
                        let Some(value) = semantic_value else {
                            // Why: Bridging a direct source to a pointer target needs the value.
                            unreachable!("pointer target requires a semantic value")
                        };
                        self.builder.store(value, slot);
                        callee_args.push(slot);
                    }
                }
                ParamPassing::Void => {
                    debug_assert_eq!(source_param.passing, ParamPassing::Void);
                }
            }
            wrapper_param_index += 1;
        }
    }

    /// Project one frozen logical retain action through LLVM's physical layout.
    /// Product edges are followed exactly so projected-field demands cannot
    /// accidentally retain unrelated siblings. Whole-value sum plans delegate
    /// active-variant selection to the existing layout-aware enum emitter.
    fn emit_frozen_closure_retain_plan(&mut self, value: ValueId, root: RetainPlanId) {
        let mut work = vec![(value, root)];
        while let Some((value, plan)) = work.pop() {
            let Some(node) = self.ctx.retain_plans.get(plan).cloned() else {
                self.builder.record_codegen_error_with_msg(format!(
                    "closure adapter references missing retain plan {}",
                    plan.index()
                ));
                return;
            };
            match node.kind {
                RetainPlanKind::SelfOwnedIdentity => {
                    let resolved = self.pool.resolve_fully(node.ty);
                    if matches!(
                        self.pool.tag(resolved),
                        ori_types::Tag::Struct | ori_types::Tag::Enum
                    ) && self.pool.aggregate_type_is_recursive(resolved)
                    {
                        self.call_rc_inc_all(&[value], 1);
                    } else {
                        self.inc_value_rc(value, node.ty, 1);
                    }
                }
                RetainPlanKind::OwnedFields(edges) => {
                    let owner = self.pool.resolve_fully(node.ty);
                    for edge in edges.iter().rev() {
                        let Some(child) = self.ctx.retain_plans.get(edge.child).cloned() else {
                            self.builder.record_codegen_error_with_msg(format!(
                                "closure retain plan {} references missing child {}",
                                plan.index(),
                                edge.child.index()
                            ));
                            return;
                        };
                        let memory_field = self.remap_struct_field(owner, edge.field);
                        let Some(field_value) = self.builder.extract_value(
                            value,
                            memory_field,
                            &format!("closure.retain.f.{}", edge.field),
                        ) else {
                            self.builder.record_codegen_error_with_msg(format!(
                                "closure retain plan {} references absent field {}",
                                plan.index(),
                                edge.field,
                            ));
                            return;
                        };
                        if is_boxed_enum_field(self.pool, owner, child.ty) {
                            self.call_rc_inc_all(&[field_value], 1);
                        } else {
                            work.push((field_value, edge.child));
                        }
                    }
                }
                RetainPlanKind::OwnedVariants(_) => {
                    self.inc_value_rc(value, node.ty, 1);
                }
            }
        }
    }
}
