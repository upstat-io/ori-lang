//! Terminator emission for the ARC IR emitter.
//!
//! Translates [`ArcTerminator`] variants into LLVM control flow: `ret`, `br`,
//! `cond_br`, `switch`, `invoke`/`call`, `resume`, and `unreachable`.

use ori_arc::ir::{ArcBlockId, ArcFunction, ArcTerminator, ArcVarId};
use rustc_hash::FxHashMap;

use crate::codegen::abi::{FunctionAbi, ReturnPassing};
use crate::codegen::eh_model::EhModel;
use crate::codegen::value_id::ValueId;

use super::ArcIrEmitter;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Emit an `ArcTerminator` as LLVM control flow.
    pub(super) fn emit_terminator(
        &mut self,
        term: &ArcTerminator,
        current_block: ArcBlockId,
        _phi_nodes: &[Vec<(ArcVarId, ValueId)>],
        abi: &FunctionAbi,
        landingpad_values: &FxHashMap<usize, ValueId>,
        arc_func: &ArcFunction,
    ) {
        tracing::trace!(?term, block = current_block.index(), "emit_terminator");
        match term {
            ArcTerminator::Return { value } => self.emit_return_terminator(*value, abi),
            ArcTerminator::Jump { target, args } => {
                self.emit_jump_terminator(*target, args, arc_func);
            }
            ArcTerminator::Branch {
                cond,
                then_block,
                else_block,
            } => {
                debug_assert!(
                    self.current_cleanup_pad.is_none(),
                    "Branch terminator inside SEH funclet"
                );
                let condition = self.var(*cond);
                self.builder
                    .cond_br(condition, self.block(*then_block), self.block(*else_block));
            }
            ArcTerminator::Switch {
                scrutinee,
                cases,
                default,
            } => self.emit_switch_terminator(*scrutinee, cases, *default),
            ArcTerminator::Invoke { .. } => {
                self.emit_direct_invoke_terminator(term, arc_func);
            }
            ArcTerminator::InvokeIndirect {
                dst,
                ty,
                closure,
                args,
                normal,
                unwind,
                ..
            } => {
                self.emit_invoke_indirect(*dst, *ty, *closure, args, *normal, *unwind, arc_func);
            }
            ArcTerminator::Resume => {
                self.emit_resume_terminator(current_block, landingpad_values);
            }
            ArcTerminator::Unreachable => self.builder.unreachable(),
        }
    }

    fn emit_return_terminator(&mut self, value: ArcVarId, abi: &FunctionAbi) {
        let value = self.var(value);
        match abi.return_abi.passing {
            ReturnPassing::Sret { .. } => {
                if self.sret_forwarded_result != Some(value) {
                    let output = self.builder.get_param(self.current_function, 0);
                    let widened = self.widen_to_boundary(value, abi.return_abi.ty);
                    self.builder.store(widened, output);
                }
                self.builder.ret_void();
            }
            ReturnPassing::Direct => {
                let widened = self.widen_to_boundary(value, abi.return_abi.ty);
                self.builder.ret(widened);
            }
            ReturnPassing::Void => self.builder.ret_void(),
        }
    }

    fn emit_jump_terminator(
        &mut self,
        target: ArcBlockId,
        args: &[ArcVarId],
        arc_func: &ArcFunction,
    ) {
        let target_index = target.index();
        debug_assert_eq!(
            args.len(),
            arc_func.blocks[target_index].params.len(),
            "Jump arg count must match target block param count (block {target_index})"
        );
        if self.current_cleanup_pad.take().is_some() {
            self.builder.record_codegen_error_with_msg(
                "Jump terminator inside cleanuppad; cleanup pads must exit with Resume",
            );
            self.builder.unreachable();
            return;
        }
        if args.is_empty() {
            self.builder.br(self.block(target));
            return;
        }

        let Some(source_block) = self.builder.current_block() else {
            tracing::error!("ARC jump: no current block — skipping phi incoming");
            self.builder.record_codegen_error();
            self.builder.br(self.block(target));
            return;
        };
        for (param_index, &arg) in args.iter().enumerate() {
            let value = self.var(arg);
            let target_var = arc_func.blocks[target_index].params[param_index].0;
            let value = if let Some(&width) = self.narrowed_vars.get(&target_var) {
                let narrow_ty = self.llvm_type_for_int_width(width);
                self.builder.trunc(value, narrow_ty, "phi.trunc")
            } else {
                value
            };
            self.phi_incoming
                .push((target_index, param_index, value, source_block));
        }
        self.builder.br(self.block(target));
    }

    fn emit_switch_terminator(
        &mut self,
        scrutinee: ArcVarId,
        cases: &[(u64, ArcBlockId)],
        default: ArcBlockId,
    ) {
        debug_assert!(
            self.current_cleanup_pad.is_none(),
            "Switch terminator inside SEH funclet"
        );
        let value = self.var(scrutinee);
        let niche_encoding = self
            .niche_scrutinees
            .get(&scrutinee)
            .copied()
            .and_then(|enum_ty| self.get_niche_encoding(enum_ty));
        if let Some(encoding) = niche_encoding {
            self.emit_niche_switch(value, &encoding, cases, default);
            return;
        }

        let llvm_cases = cases
            .iter()
            .map(|&(tag, block)| {
                let tag_value = self.builder.const_int_matching(value, tag);
                (tag_value, self.block(block))
            })
            .collect::<Vec<_>>();
        self.builder.switch(value, self.block(default), &llvm_cases);
    }

    fn emit_direct_invoke_terminator(&mut self, term: &ArcTerminator, arc_func: &ArcFunction) {
        let ArcTerminator::Invoke {
            dst,
            func,
            args,
            mono_instance_id,
            normal,
            unwind,
            ..
        } = term
        else {
            unreachable!("direct invoke helper requires Invoke")
        };

        let is_nounwind = self.ctx.nounwind_functions.contains(func);
        let is_seh_catch = !is_nounwind
            && self.builder.eh_model() == EhModel::Seh
            && !matches!(
                arc_func.blocks[unwind.index()].terminator,
                ArcTerminator::Resume
            );
        if is_seh_catch {
            self.emit_seh_catch_invoke(
                *dst,
                *func,
                args,
                *normal,
                *unwind,
                arc_func,
                *mono_instance_id,
            );
        } else {
            self.emit_invoke(
                *dst,
                *func,
                args,
                *normal,
                *unwind,
                arc_func,
                *mono_instance_id,
            );
        }
    }

    fn emit_resume_terminator(
        &mut self,
        current_block: ArcBlockId,
        landingpad_values: &FxHashMap<usize, ValueId>,
    ) {
        match self.builder.eh_model() {
            EhModel::Itanium => {
                if let Some(&landingpad) = landingpad_values.get(&current_block.index()) {
                    self.builder.resume(landingpad);
                } else {
                    tracing::warn!(
                        block = current_block.index(),
                        "ARC Resume without landingpad — emitting unreachable"
                    );
                    self.builder.unreachable();
                }
            }
            EhModel::Seh => {
                if let Some(pad) = self.current_cleanup_pad.take() {
                    self.builder.cleanupret(pad, None);
                } else {
                    tracing::warn!(
                        block = current_block.index(),
                        "ARC Resume without cleanuppad — emitting unreachable"
                    );
                    self.builder.unreachable();
                }
            }
        }
    }

    /// Emit a niche-aware switch as `icmp eq` + `cond_br`.
    ///
    /// For 2-variant niche enums (Option, Result): compare the raw niche field
    /// value against the niche sentinel, then branch to the niche variant block
    /// or the data variant block.
    fn emit_niche_switch(
        &mut self,
        scrut_val: ValueId,
        encoding: &super::tag_access::TagEncoding,
        cases: &[(u64, ori_arc::ir::ArcBlockId)],
        default: ori_arc::ir::ArcBlockId,
    ) {
        let Some((_, niche_value, niche_variant_idx)) = encoding.niche_fields() else {
            self.builder
                .record_codegen_error_with_msg("niche switch requires a niche tag encoding");
            self.builder.br(self.block(default));
            return;
        };

        // Find blocks for each logical variant.
        let niche_block = cases
            .iter()
            .find(|(tag, _)| *tag == u64::from(niche_variant_idx))
            .map(|(_, b)| self.block(*b));
        let data_block = cases
            .iter()
            .find(|(tag, _)| *tag != u64::from(niche_variant_idx))
            .map_or_else(|| self.block(default), |(_, b)| self.block(*b));

        let is_niche = self.niche_is_sentinel(scrut_val, niche_value, "is.niche");
        if let Some(nb) = niche_block {
            self.builder.cond_br(is_niche, nb, data_block);
        } else {
            // No case for the niche variant — branch directly to data.
            self.builder.br(data_block);
        }
    }
}
