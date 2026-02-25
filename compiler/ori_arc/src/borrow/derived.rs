//! Per-variable derived ownership inference.
//!
//! Unlike parameter-level borrow inference (which classifies only function
//! parameters), this module classifies **every** variable via a single
//! forward SSA pass.

use rustc_hash::FxHashMap;

use ori_ir::Name;

use crate::ir::{ArcFunction, ArcInstr};
use crate::ownership::{AnnotatedSig, DerivedOwnership, Ownership};

/// Infer per-variable ownership from SSA data flow.
///
/// Unlike [`super::infer_borrows_scc`] which classifies only function parameters via
/// SCC-based inference, this function classifies **every** variable in a
/// single forward pass (no fixed-point needed — SSA guarantees each variable
/// is defined exactly once).
///
/// The result is a `Vec<DerivedOwnership>` indexed by `ArcVarId::raw()`,
/// enabling RC insertion to skip `RcInc`/`RcDec` for:
/// - Variables borrowed from a still-live owner (`BorrowedFrom`)
/// - Freshly constructed values with refcount = 1 (`Fresh`)
///
/// # Arguments
///
/// * `func` — the ARC IR function to analyze.
/// * `sigs` — annotated signatures from borrow inference (for callee param ownership).
#[expect(clippy::implicit_hasher, reason = "FxHashMap is the canonical hasher")]
pub fn infer_derived_ownership(
    func: &ArcFunction,
    sigs: &FxHashMap<Name, AnnotatedSig>,
) -> Vec<DerivedOwnership> {
    let num_vars = func.var_types.len();
    let mut ownership = vec![DerivedOwnership::Owned; num_vars];

    // Function parameters: inherit from AnnotatedSig.
    if let Some(sig) = sigs.get(&func.name) {
        for (i, param) in func.params.iter().enumerate() {
            let idx = param.var.index();
            if idx < num_vars {
                ownership[idx] = match sig.params.get(i).map(|p| p.ownership) {
                    Some(Ownership::Borrowed) => DerivedOwnership::BorrowedFrom(param.var),
                    _ => DerivedOwnership::Owned,
                };
            }
        }
    }

    // Forward pass over all blocks in order.
    // SSA form: each variable is defined exactly once, so a single forward
    // pass is sufficient (no iteration needed).
    for block in &func.blocks {
        // Block parameters receive values via jump args — they're owned
        // (the caller transfers ownership through the jump).
        for &(param_var, _ty) in &block.params {
            let idx = param_var.index();
            if idx < num_vars {
                ownership[idx] = DerivedOwnership::Owned;
            }
        }

        for instr in &block.body {
            match instr {
                ArcInstr::Project { dst, value, .. } => {
                    // A projection borrows from the source variable.
                    let dst_idx = dst.index();
                    if dst_idx < num_vars {
                        let source_idx = value.index();
                        ownership[dst_idx] = if source_idx < num_vars {
                            // Transitively resolve: if `value` borrows from X,
                            // the projection also borrows from X.
                            match ownership[source_idx] {
                                DerivedOwnership::BorrowedFrom(root) => {
                                    DerivedOwnership::BorrowedFrom(root)
                                }
                                _ => DerivedOwnership::BorrowedFrom(*value),
                            }
                        } else {
                            DerivedOwnership::BorrowedFrom(*value)
                        };
                    }
                }

                ArcInstr::Let { dst, value, .. } => {
                    let dst_idx = dst.index();
                    if dst_idx < num_vars {
                        ownership[dst_idx] = match value {
                            // Var alias inherits from source.
                            crate::ir::ArcValue::Var(src) => {
                                let src_idx = src.index();
                                if src_idx < num_vars {
                                    ownership[src_idx]
                                } else {
                                    DerivedOwnership::Owned
                                }
                            }
                            // Literals and PrimOps produce owned values.
                            crate::ir::ArcValue::Literal(_)
                            | crate::ir::ArcValue::PrimOp { .. } => DerivedOwnership::Owned,
                        };
                    }
                }

                ArcInstr::Construct { dst, .. } => {
                    // A newly constructed value has refcount = 1.
                    let dst_idx = dst.index();
                    if dst_idx < num_vars {
                        ownership[dst_idx] = DerivedOwnership::Fresh;
                    }
                }

                ArcInstr::PartialApply { dst, .. } => {
                    // A new closure has refcount = 1.
                    let dst_idx = dst.index();
                    if dst_idx < num_vars {
                        ownership[dst_idx] = DerivedOwnership::Fresh;
                    }
                }

                ArcInstr::Apply { dst, .. } | ArcInstr::ApplyIndirect { dst, .. } => {
                    // Call results are owned (callee returns an owned value).
                    let dst_idx = dst.index();
                    if dst_idx < num_vars {
                        ownership[dst_idx] = DerivedOwnership::Owned;
                    }
                }

                // RC/reuse ops don't define new variables (or their dst
                // is a token which is always Owned).
                ArcInstr::RcInc { .. }
                | ArcInstr::RcDec { .. }
                | ArcInstr::IsShared { .. }
                | ArcInstr::Set { .. }
                | ArcInstr::SetTag { .. }
                | ArcInstr::Reset { .. }
                | ArcInstr::Reuse { .. } => {}
            }
        }
    }

    ownership
}
