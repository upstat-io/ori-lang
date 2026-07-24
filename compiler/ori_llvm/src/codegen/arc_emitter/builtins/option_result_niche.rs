//! Niche-encoded Option and Result LLVM dispatch.
//!
//! Debug formatting lives in `debug_render.rs`.

#[cfg(test)]
mod tests;

use ori_types::Idx;

use crate::codegen::arc_emitter::tag_access::TagEncoding;
use crate::codegen::type_info::TypeInfo;
use crate::codegen::value_id::ValueId;

use super::{super::ArcIrEmitter, RenderStyle};

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Niche-encoded Option dispatch.
    ///
    /// Unwrap operations guard the active variant and retain an extracted
    /// payload when the result creates a new owning reference.
    pub(super) fn emit_option_niche(
        &mut self,
        method: &str,
        receiver: ValueId,
        arg_vals: &[ValueId],
        receiver_ty: Idx,
        encoding: &TagEncoding,
    ) -> Option<ValueId> {
        let (niche_idx, niche_value, _) = encoding.niche_fields()?;
        match method {
            "is_some" => {
                let is_some = self.compute_option_is_some(receiver, niche_idx, niche_value)?;
                Some(is_some)
            }
            "is_none" => {
                let field = self
                    .builder
                    .extract_value(receiver, niche_idx, "opt.niche")?;
                Some(self.niche_is_sentinel(field, niche_value, "is_none"))
            }
            "unwrap" => {
                let is_some = self.compute_option_is_some(receiver, niche_idx, niche_value)?;
                self.emit_unwrap_branch(
                    is_some,
                    "called `Option.unwrap()` on a `None` value",
                    "opt_unwrap_niche",
                )?;
                // After unwrap branch, guaranteed Some — retain unconditionally.
                let payload = self.builder.extract_value(receiver, 0, "opt.payload")?;
                let inner_ty = self.pool.option_inner(self.pool.resolve_fully(receiver_ty));
                self.inc_value_rc(payload, inner_ty, 1);
                Some(payload)
            }
            "unwrap_or" if arg_vals.len() >= 2 => {
                let is_some = self.compute_option_is_some(receiver, niche_idx, niche_value)?;
                let payload = self.builder.extract_value(receiver, 0, "opt.payload")?;

                // RC-retain payload when Some: the select copies payload bytes,
                // creating a second reference to inner RC data. Skip for scalar
                // payloads (no RC) to avoid extra blocks that break PHI nodes
                // when unwrap_or is inside a short-circuit `&&`/`||` branch.
                let inner_ty = self.pool.option_inner(self.pool.resolve_fully(receiver_ty));
                let needs_rc = !self.classifier.is_scalar(inner_ty);
                if needs_rc {
                    let inc_bb = self
                        .builder
                        .append_block(self.current_function, "uor.niche.inc");
                    let merge_bb = self
                        .builder
                        .append_block(self.current_function, "uor.niche.merge");
                    self.builder.cond_br(is_some, inc_bb, merge_bb);
                    self.builder.position_at_end(inc_bb);
                    self.inc_value_rc(payload, inner_ty, 1);
                    self.builder.br(merge_bb);
                    self.builder.position_at_end(merge_bb);
                }

                Some(
                    self.builder
                        .select(is_some, payload, arg_vals[1], "unwrap_or"),
                )
            }
            "expect" if arg_vals.len() >= 2 => {
                let is_some = self.compute_option_is_some(receiver, niche_idx, niche_value)?;
                self.emit_expect_branch(is_some, arg_vals[1], "expect")?;
                // After expect branch, guaranteed Some — retain unconditionally.
                let payload = self.builder.extract_value(receiver, 0, "opt.payload")?;
                let inner_ty = self.pool.option_inner(self.pool.resolve_fully(receiver_ty));
                self.inc_value_rc(payload, inner_ty, 1);
                Some(payload)
            }
            // Render mode is decided once at the dispatch boundary — the
            // shared body never re-compares the method name.
            "debug" => self.emit_option_niche_render(
                receiver,
                receiver_ty,
                niche_idx,
                niche_value,
                RenderStyle::Debug,
            ),
            "to_str" => self.emit_option_niche_render(
                receiver,
                receiver_ty,
                niche_idx,
                niche_value,
                RenderStyle::Printable,
            ),
            "clone" => Some(receiver),
            _ => None,
        }
    }

    /// Shared `debug` / `to_str` rendering for niche-encoded Options.
    /// `style` selects Debug or Printable formatting.
    fn emit_option_niche_render(
        &mut self,
        receiver: ValueId,
        receiver_ty: Idx,
        niche_idx: u32,
        niche_value: u64,
        style: RenderStyle,
    ) -> Option<ValueId> {
        let is_some = self.compute_option_is_some(receiver, niche_idx, niche_value)?;
        let payload = self.builder.extract_value(receiver, 0, "opt.payload")?;
        let TypeInfo::Option { inner } = self.type_info.get(receiver_ty) else {
            return None;
        };
        self.emit_option_debug_branch(is_some, payload, inner, style)
    }

    /// Compute the `is_some` predicate for a niche-encoded Option.
    ///
    /// Loads the niche field, compares it against the sentinel via
    /// [`niche_is_sentinel`](Self::niche_is_sentinel) (which yields
    /// `is_none`), then inverts the polarity. Used by every Option
    /// niche helper that needs the variant predicate.
    fn compute_option_is_some(
        &mut self,
        receiver: ValueId,
        niche_idx: u32,
        niche_value: u64,
    ) -> Option<ValueId> {
        let field = self
            .builder
            .extract_value(receiver, niche_idx, "opt.niche")?;
        let is_niche = self.niche_is_sentinel(field, niche_value, "is_niche");
        let t = self.builder.const_bool(true);
        let f = self.builder.const_bool(false);
        Some(self.builder.select(is_niche, f, t, "is_some"))
    }

    /// Niche-encoded Result dispatch.
    ///
    /// Each operation computes its niche-aware variant predicate before
    /// extracting a payload. Unwrap and expect operations guard the active
    /// variant; payload-producing operations retain new owning references.
    pub(super) fn emit_result_niche(
        &mut self,
        method: &str,
        receiver: ValueId,
        arg_vals: &[ValueId],
        receiver_ty: Idx,
        encoding: &TagEncoding,
    ) -> Option<ValueId> {
        let (niche_idx, niche_value, niche_variant_idx) = encoding.niche_fields()?;
        match method {
            "is_ok" => {
                self.compute_result_is_ok(receiver, niche_idx, niche_value, niche_variant_idx)
            }
            "is_err" => {
                self.compute_result_is_err(receiver, niche_idx, niche_value, niche_variant_idx)
            }
            "unwrap" => {
                let is_ok =
                    self.compute_result_is_ok(receiver, niche_idx, niche_value, niche_variant_idx)?;
                self.emit_unwrap_branch(
                    is_ok,
                    "called `Result.unwrap()` on an `Err` value",
                    "res_unwrap_niche",
                )?;
                // After unwrap branch, guaranteed Ok — retain unconditionally.
                let payload = self.builder.extract_value(receiver, 0, "res.payload")?;
                let TypeInfo::Result { ok: ok_ty, .. } = self.type_info.get(receiver_ty) else {
                    return Some(payload);
                };
                self.inc_value_rc(payload, ok_ty, 1);
                Some(payload)
            }
            "unwrap_err" => {
                let is_err = self.compute_result_is_err(
                    receiver,
                    niche_idx,
                    niche_value,
                    niche_variant_idx,
                )?;
                self.emit_unwrap_branch(
                    is_err,
                    "called `Result.unwrap_err()` on an `Ok` value",
                    "res_unwrap_err_niche",
                )?;
                // After unwrap branch, guaranteed Err — retain unconditionally.
                let payload = self.builder.extract_value(receiver, 0, "res.payload")?;
                let TypeInfo::Result { err: err_ty, .. } = self.type_info.get(receiver_ty) else {
                    return Some(payload);
                };
                self.inc_value_rc(payload, err_ty, 1);
                Some(payload)
            }
            "unwrap_or" if arg_vals.len() >= 2 => {
                let is_ok =
                    self.compute_result_is_ok(receiver, niche_idx, niche_value, niche_variant_idx)?;
                let payload = self.builder.extract_value(receiver, 0, "res.payload")?;

                // RC-retain payload when Ok: the select copies payload bytes.
                if let TypeInfo::Result { ok: ok_ty, .. } = self.type_info.get(receiver_ty) {
                    if !self.classifier.is_scalar(ok_ty) {
                        let inc_bb = self
                            .builder
                            .append_block(self.current_function, "ruor.niche.inc");
                        let merge_bb = self
                            .builder
                            .append_block(self.current_function, "ruor.niche.merge");
                        self.builder.cond_br(is_ok, inc_bb, merge_bb);
                        self.builder.position_at_end(inc_bb);
                        self.inc_value_rc(payload, ok_ty, 1);
                        self.builder.br(merge_bb);
                        self.builder.position_at_end(merge_bb);
                    }
                }

                Some(
                    self.builder
                        .select(is_ok, payload, arg_vals[1], "unwrap_or"),
                )
            }
            "expect" if arg_vals.len() >= 2 => {
                let is_ok =
                    self.compute_result_is_ok(receiver, niche_idx, niche_value, niche_variant_idx)?;
                self.emit_expect_branch(is_ok, arg_vals[1], "res_expect_niche")?;
                // After expect branch, guaranteed Ok — retain unconditionally.
                let payload = self.builder.extract_value(receiver, 0, "res.payload")?;
                let TypeInfo::Result { ok: ok_ty, .. } = self.type_info.get(receiver_ty) else {
                    return Some(payload);
                };
                self.inc_value_rc(payload, ok_ty, 1);
                Some(payload)
            }
            "expect_err" if arg_vals.len() >= 2 => {
                let is_err = self.compute_result_is_err(
                    receiver,
                    niche_idx,
                    niche_value,
                    niche_variant_idx,
                )?;
                self.emit_expect_branch(is_err, arg_vals[1], "res_expect_err_niche")?;
                // After expect branch, guaranteed Err — retain unconditionally.
                let payload = self.builder.extract_value(receiver, 0, "res.payload")?;
                let TypeInfo::Result { err: err_ty, .. } = self.type_info.get(receiver_ty) else {
                    return Some(payload);
                };
                self.inc_value_rc(payload, err_ty, 1);
                Some(payload)
            }
            "clone" => Some(receiver),
            _ => None,
        }
    }

    /// Compute the `is_ok` predicate for a niche-encoded Result.
    ///
    /// Loads the niche field, compares against the sentinel, and inverts based
    /// on whether Ok or Err is the niche variant. When `niche_variant_idx == 0`
    /// (Ok is the niche variant), `is_ok = is_niche`; otherwise `is_ok = !is_niche`.
    fn compute_result_is_ok(
        &mut self,
        receiver: ValueId,
        niche_idx: u32,
        niche_value: u64,
        niche_variant_idx: u32,
    ) -> Option<ValueId> {
        let field = self
            .builder
            .extract_value(receiver, niche_idx, "res.niche")?;
        let is_niche = self.niche_is_sentinel(field, niche_value, "res.is_niche");
        if niche_variant_idx == 0 {
            Some(is_niche)
        } else {
            let t = self.builder.const_bool(true);
            let f = self.builder.const_bool(false);
            Some(self.builder.select(is_niche, f, t, "is_ok"))
        }
    }

    /// Compute the `is_err` predicate for a niche-encoded Result.
    ///
    /// Mirror of [`compute_result_is_ok`](Self::compute_result_is_ok) — when
    /// `niche_variant_idx == 1` (Err is the niche variant), `is_err = is_niche`;
    /// otherwise `is_err = !is_niche`.
    fn compute_result_is_err(
        &mut self,
        receiver: ValueId,
        niche_idx: u32,
        niche_value: u64,
        niche_variant_idx: u32,
    ) -> Option<ValueId> {
        let field = self
            .builder
            .extract_value(receiver, niche_idx, "res.niche")?;
        let is_niche = self.niche_is_sentinel(field, niche_value, "res.is_niche");
        if niche_variant_idx == 1 {
            Some(is_niche)
        } else {
            let t = self.builder.const_bool(true);
            let f = self.builder.const_bool(false);
            Some(self.builder.select(is_niche, f, t, "is_err"))
        }
    }
}
