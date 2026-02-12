//! Reset/Reuse detection for ARC IR (Section 07.6).
//!
//! After RC insertion (§07.2), identifies opportunities for in-place
//! constructor reuse: when an `RcDec` is immediately followed by a
//! `Construct` of the same type, the memory can be reused instead of
//! freed and reallocated.
//!
//! This pass replaces:
//! ```text
//! RcDec { var: x }
//! Construct { dst: y, ty: T, ctor, args }
//! ```
//! with:
//! ```text
//! Reset { var: x, token: t }
//! Reuse { token: t, dst: y, ty: T, ctor, args }
//! ```
//!
//! where `t` is a fresh reuse token. The `Reset`/`Reuse` pair is later
//! expanded by Section 09 into a conditional: if `x` is uniquely owned
//! (RC == 1), reuse the memory in-place; otherwise allocate fresh.
//!
//! # Constraints
//!
//! A `RcDec`/`Construct` pair is only valid for reset/reuse if:
//!
//! 1. The types match: `typeof(x) == ty` of the `Construct`.
//! 2. No use of `x` between the `RcDec` and `Construct` (no aliasing).
//! 3. The type needs RC (is heap-allocated).
//!
//! # References
//!
//! - Lean 4: `src/Lean/Compiler/IR/ExpandResetReuse.lean`
//! - Lean 4: `src/Lean/Compiler/IR/ResetReuse.lean`
//! - Koka: Perceus paper §4 (reuse analysis)

use ori_types::Idx;
use rustc_hash::FxHashSet;

use crate::graph::DominatorTree;
use crate::ir::{ArcBlockId, ArcFunction, ArcInstr, ArcVarId};
use crate::liveness::RefinedLiveness;
use crate::ArcClassification;

/// Detect and replace `RcDec`/`Construct` pairs with `Reset`/`Reuse`.
///
/// Scans each block forward for matching pairs. Only intra-block matches
/// are considered (cross-block reuse would require more complex analysis).
///
/// # Arguments
///
/// * `func` — the ARC IR function to transform (mutated in place).
/// * `classifier` — type classifier for `needs_rc()` checks.
pub(crate) fn detect_reset_reuse(func: &mut ArcFunction, classifier: &dyn ArcClassification) {
    // Precondition: detection creates Reset/Reuse — none should exist yet.
    debug_assert!(
        !func
            .blocks
            .iter()
            .flat_map(|b| b.body.iter())
            .any(|i| matches!(i, ArcInstr::Reset { .. } | ArcInstr::Reuse { .. })),
        "detect_reset_reuse: IR already contains Reset/Reuse — pipeline ordering error"
    );

    tracing::debug!(
        function = func.name.raw(),
        "detecting reset/reuse opportunities"
    );

    let num_blocks = func.blocks.len();

    for block_idx in 0..num_blocks {
        detect_in_block(func, block_idx, classifier);
    }
}

/// Detect reset/reuse pairs within a single block.
///
/// Uses a forward scan. When we find an `RcDec`, we look ahead for a
/// matching `Construct`. If found and constraints are satisfied, replace
/// both instructions.
fn detect_in_block(func: &mut ArcFunction, block_idx: usize, classifier: &dyn ArcClassification) {
    // Track which RcDec indices have been paired, so we don't pair twice.
    let mut paired_decs: FxHashSet<usize> = FxHashSet::default();
    // Track which Construct indices have been paired.
    let mut paired_constructs: FxHashSet<usize> = FxHashSet::default();

    // Phase 1: Scan — collect matched (dec_idx, construct_idx, dec_ty)
    // triples. Token allocation is deferred to after the scan to avoid
    // a borrow conflict (body borrows func.blocks immutably, fresh_var
    // borrows func mutably).
    let mut matched: Vec<(usize, usize, Idx)> = Vec::new();

    let body = &func.blocks[block_idx].body;

    for i in 0..body.len() {
        if paired_decs.contains(&i) {
            continue;
        }

        // Look for RcDec instructions.
        let dec_var = match &body[i] {
            ArcInstr::RcDec { var } => *var,
            _ => continue,
        };

        // Check that the type needs RC (skip scalars).
        let dec_ty = func.var_type(dec_var);
        if !classifier.needs_rc(dec_ty) {
            continue;
        }

        // Scan forward for a matching Construct.
        for (j, candidate) in body.iter().enumerate().skip(i + 1) {
            if paired_constructs.contains(&j) {
                continue;
            }

            // Check constraint: no use of dec_var between i and j.
            if candidate.uses_var(dec_var) && !matches!(candidate, ArcInstr::Construct { .. }) {
                // dec_var is used before we find a Construct → cannot reuse.
                break;
            }

            match candidate {
                ArcInstr::Construct { ty, .. } if *ty == dec_ty => {
                    // Check that dec_var is NOT used in the Construct's args.
                    // (If it is, there's an alias and reuse is unsafe.)
                    if candidate.uses_var(dec_var) {
                        // dec_var appears in args → skip this Construct.
                        continue;
                    }

                    matched.push((i, j, dec_ty));
                    paired_decs.insert(i);
                    paired_constructs.insert(j);
                    break;
                }
                _ => {
                    // Check if this instruction uses dec_var → constraint violation.
                    if candidate.uses_var(dec_var) {
                        break;
                    }
                }
            }
        }
    }

    // Phase 2: Allocate fresh token variables (body borrow is released).
    let pairs: Vec<(usize, usize, ArcVarId)> = matched
        .into_iter()
        .map(|(dec_idx, construct_idx, dec_ty)| {
            let token = func.fresh_var(dec_ty);
            (dec_idx, construct_idx, token)
        })
        .collect();

    // Apply replacements (in reverse order to preserve indices).
    // Since we're replacing in-place at fixed indices, order doesn't matter
    // for correctness, but we process pairs as collected.
    let body = &mut func.blocks[block_idx].body;
    for (dec_idx, construct_idx, token) in pairs {
        // Extract Construct details before replacing.
        let (dst, ty, ctor, args) = match &body[construct_idx] {
            ArcInstr::Construct {
                dst,
                ty,
                ctor,
                args,
            } => (*dst, *ty, *ctor, args.clone()),
            _ => unreachable!("paired construct index must be a Construct"),
        };

        let dec_var = match &body[dec_idx] {
            ArcInstr::RcDec { var } => *var,
            _ => unreachable!("paired dec index must be an RcDec"),
        };

        // Replace RcDec → Reset.
        body[dec_idx] = ArcInstr::Reset {
            var: dec_var,
            token,
        };

        // Replace Construct → Reuse.
        body[construct_idx] = ArcInstr::Reuse {
            token,
            dst,
            ty,
            ctor,
            args,
        };
    }
}

/// Cross-block reset/reuse detection using dominator tree and refined liveness.
///
/// Extends intra-block detection to find reuse opportunities across basic
/// blocks. The canonical case is linked-list `map`:
///
/// ```text
/// B0: RcDec(node)          ← unpaired after intra-block detection
/// B1: ...                   ← dominated by B0
/// B2: new = Construct(Node) ← allocation in dominated block
/// ```
///
/// If `node` is only live-for-drop (not read as operand) in B1, then we can
/// replace `RcDec(node)` → `Reset(node, token)` in B0 and `Construct` →
/// `Reuse(token, ...)` in B2.
///
/// # Safety
///
/// This transformation is valid because:
/// 1. B0 dominates B2 → the token is always available at B2
/// 2. `node` is not live-for-use in any block between B0 and B2 → no aliasing
/// 3. The types match → memory layout is compatible for reuse
///
/// # Arguments
///
/// * `func` — the ARC IR function (mutated in place).
/// * `classifier` — type classifier for `needs_rc()` checks.
/// * `dom_tree` — precomputed dominator tree.
/// * `refined` — precomputed refined liveness per block.
pub fn detect_reset_reuse_cfg(
    func: &mut ArcFunction,
    classifier: &dyn ArcClassification,
    dom_tree: &DominatorTree,
    refined: &[RefinedLiveness],
) {
    // Step 1: Run intra-block detection first (fast path).
    detect_reset_reuse(func, classifier);

    // Step 2: Collect unpaired RcDec instructions.
    // After intra-block detection, remaining RcDec instructions are candidates
    // for cross-block pairing.
    let mut unpaired_decs: Vec<(usize, usize, ArcVarId, Idx)> = Vec::new();
    for (block_idx, block) in func.blocks.iter().enumerate() {
        for (instr_idx, instr) in block.body.iter().enumerate() {
            if let ArcInstr::RcDec { var } = instr {
                let ty = func.var_type(*var);
                if classifier.needs_rc(ty) {
                    unpaired_decs.push((block_idx, instr_idx, *var, ty));
                }
            }
        }
    }

    if unpaired_decs.is_empty() {
        return;
    }

    tracing::debug!(
        unpaired = unpaired_decs.len(),
        "cross-block reset/reuse: scanning dominated blocks"
    );

    // Step 3: For each unpaired RcDec, walk dominated blocks to find a matching Construct.
    let num_blocks = func.blocks.len();
    let mut paired_constructs: FxHashSet<(usize, usize)> = FxHashSet::default();
    let mut matches: Vec<CrossBlockMatch> = Vec::new();

    for &(dec_block_idx, dec_instr_idx, dec_var, dec_ty) in &unpaired_decs {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "ARC IR block counts fit in u32"
        )]
        let dec_block_id = ArcBlockId::new(dec_block_idx as u32);
        let dominated = dom_tree.dominated_preorder(dec_block_id, num_blocks);

        let mut found = false;
        for &target_block_id in &dominated {
            let target_idx = target_block_id.index();

            // Skip the dec's own block (intra-block already handled).
            if target_idx == dec_block_idx {
                continue;
            }

            // Check aliasing: if dec_var is live-for-use in this block,
            // it might be read, so we can't safely reset it.
            if target_idx < refined.len() && refined[target_idx].live_for_use.contains(&dec_var) {
                // Variable is read in this subtree — cannot pair.
                break;
            }

            // Scan block for an unpaired Construct of matching type.
            let target_body = &func.blocks[target_idx].body;
            for (ci, instr) in target_body.iter().enumerate() {
                if paired_constructs.contains(&(target_idx, ci)) {
                    continue;
                }
                if let ArcInstr::Construct { ty, .. } = instr {
                    if *ty == dec_ty && !instr.uses_var(dec_var) {
                        matches.push(CrossBlockMatch {
                            dec_block: dec_block_idx,
                            dec_instr: dec_instr_idx,
                            dec_var,
                            construct_block: target_idx,
                            construct_instr: ci,
                        });
                        paired_constructs.insert((target_idx, ci));
                        found = true;
                        break;
                    }
                }
            }

            if found {
                break;
            }
        }
    }

    if matches.is_empty() {
        return;
    }

    tracing::debug!(
        cross_block_pairs = matches.len(),
        "cross-block reset/reuse: applying transformations"
    );

    // Step 4: Apply cross-block replacements.
    for m in matches {
        let token = func.fresh_var(func.var_type(m.dec_var));

        // Extract Construct details.
        let (dst, ty, ctor, args) = match &func.blocks[m.construct_block].body[m.construct_instr] {
            ArcInstr::Construct {
                dst,
                ty,
                ctor,
                args,
            } => (*dst, *ty, *ctor, args.clone()),
            _ => unreachable!("paired construct must be a Construct"),
        };

        // Replace RcDec → Reset in the dec's block.
        func.blocks[m.dec_block].body[m.dec_instr] = ArcInstr::Reset {
            var: m.dec_var,
            token,
        };

        // Replace Construct → Reuse in the target block.
        func.blocks[m.construct_block].body[m.construct_instr] = ArcInstr::Reuse {
            token,
            dst,
            ty,
            ctor,
            args,
        };
    }
}

/// A matched cross-block RcDec/Construct pair.
struct CrossBlockMatch {
    dec_block: usize,
    dec_instr: usize,
    dec_var: ArcVarId,
    construct_block: usize,
    construct_instr: usize,
}

#[cfg(test)]
mod tests {
    use ori_ir::Name;
    use ori_types::{Idx, Pool};

    use crate::ir::{ArcBlock, ArcFunction, ArcInstr, ArcTerminator, ArcValue, CtorKind, LitValue};
    use crate::test_helpers::{b, make_func, owned_param, v};
    use crate::ArcClassifier;

    use super::detect_reset_reuse;

    // Helpers

    fn run_detect(mut func: ArcFunction) -> ArcFunction {
        let pool = Pool::new();
        let classifier = ArcClassifier::new(&pool);
        detect_reset_reuse(&mut func, &classifier);
        func
    }

    // Tests

    /// Test 1: Basic pair — RcDec{x}; Construct{ty==typeof(x)} → Reset/Reuse.
    #[test]
    fn basic_pair() {
        // v0: str (param), v1: str (construct result)
        // Body: RcDec{v0}; Construct{dst:v1, ty:STR, ...}
        let func = make_func(
            vec![owned_param(0, Idx::STR)],
            Idx::STR,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![
                    ArcInstr::RcDec { var: v(0) },
                    ArcInstr::Construct {
                        dst: v(1),
                        ty: Idx::STR,
                        ctor: CtorKind::Struct(Name::from_raw(10)),
                        args: vec![],
                    },
                ],
                terminator: ArcTerminator::Return { value: v(1) },
            }],
            vec![Idx::STR, Idx::STR],
        );

        let result = run_detect(func);
        let body = &result.blocks[0].body;

        // Should be Reset/Reuse, not RcDec/Construct.
        assert!(
            matches!(&body[0], ArcInstr::Reset { var, token } if *var == v(0) && token.raw() == 2),
            "expected Reset, got {:?}",
            body[0]
        );
        assert!(
            matches!(&body[1], ArcInstr::Reuse { token, dst, ty, .. } if token.raw() == 2 && *dst == v(1) && *ty == Idx::STR),
            "expected Reuse, got {:?}",
            body[1]
        );
    }

    /// Test 2: Different type — no reuse.
    #[test]
    fn different_type_no_reuse() {
        // v0: STR, construct type: INT (different).
        // Use a type that needs_rc for the construct. Since INT is scalar,
        // use two different ref types. We'll use STR for dec and UNIT placeholder
        // for construct (UNIT is scalar, so this won't match).
        // Actually, to test properly, both need to be RC types but different.
        // Let's just check that STR dec + INT construct doesn't match.
        let func = make_func(
            vec![owned_param(0, Idx::STR)],
            Idx::INT,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![
                    ArcInstr::RcDec { var: v(0) },
                    ArcInstr::Construct {
                        dst: v(1),
                        ty: Idx::INT, // Different type (and scalar — won't match STR)
                        ctor: CtorKind::Tuple,
                        args: vec![],
                    },
                ],
                terminator: ArcTerminator::Return { value: v(1) },
            }],
            vec![Idx::STR, Idx::INT],
        );

        let result = run_detect(func);
        let body = &result.blocks[0].body;

        // Should remain RcDec/Construct (no match).
        assert!(
            matches!(&body[0], ArcInstr::RcDec { .. }),
            "expected RcDec, got {:?}",
            body[0]
        );
        assert!(
            matches!(&body[1], ArcInstr::Construct { .. }),
            "expected Construct, got {:?}",
            body[1]
        );
    }

    /// Test 3: Aliased — use of dec'd var between Dec and Construct → no reuse.
    #[test]
    fn aliased_no_reuse() {
        // RcDec{v0}; Apply{args:[v0]}; Construct{ty==typeof(v0)}
        // v0 is used in the Apply between Dec and Construct → unsafe to reuse.
        let func = make_func(
            vec![owned_param(0, Idx::STR)],
            Idx::STR,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![
                    ArcInstr::RcDec { var: v(0) },
                    ArcInstr::Apply {
                        dst: v(1),
                        ty: Idx::INT,
                        func: Name::from_raw(99),
                        args: vec![v(0)],
                    },
                    ArcInstr::Construct {
                        dst: v(2),
                        ty: Idx::STR,
                        ctor: CtorKind::Struct(Name::from_raw(10)),
                        args: vec![],
                    },
                ],
                terminator: ArcTerminator::Return { value: v(2) },
            }],
            vec![Idx::STR, Idx::INT, Idx::STR],
        );

        let result = run_detect(func);
        let body = &result.blocks[0].body;

        // Should remain unchanged.
        assert!(matches!(&body[0], ArcInstr::RcDec { .. }));
        assert!(matches!(&body[1], ArcInstr::Apply { .. }));
        assert!(matches!(&body[2], ArcInstr::Construct { .. }));
    }

    /// Test 4: Intervening non-aliasing instruction — reuse is OK.
    #[test]
    fn intervening_ok() {
        // RcDec{v0}; Let{v2: int = 42}; Construct{ty==typeof(v0)}
        // The Let doesn't use v0 → safe to reuse.
        let func = make_func(
            vec![owned_param(0, Idx::STR)],
            Idx::STR,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![
                    ArcInstr::RcDec { var: v(0) },
                    ArcInstr::Let {
                        dst: v(1),
                        ty: Idx::INT,
                        value: ArcValue::Literal(LitValue::Int(42)),
                    },
                    ArcInstr::Construct {
                        dst: v(2),
                        ty: Idx::STR,
                        ctor: CtorKind::Struct(Name::from_raw(10)),
                        args: vec![],
                    },
                ],
                terminator: ArcTerminator::Return { value: v(2) },
            }],
            vec![Idx::STR, Idx::INT, Idx::STR],
        );

        let result = run_detect(func);
        let body = &result.blocks[0].body;

        // Reset at index 0, Let at index 1, Reuse at index 2.
        assert!(
            matches!(&body[0], ArcInstr::Reset { var, .. } if *var == v(0)),
            "expected Reset, got {:?}",
            body[0]
        );
        assert!(matches!(&body[1], ArcInstr::Let { .. }));
        assert!(
            matches!(&body[2], ArcInstr::Reuse { dst, ty, .. } if *dst == v(2) && *ty == Idx::STR),
            "expected Reuse, got {:?}",
            body[2]
        );
    }

    /// Test 5: First Construct wins — two Constructs after Dec, only first paired.
    #[test]
    fn first_construct_wins() {
        // RcDec{v0}; Construct{v1:STR}; Construct{v2:STR}
        // Only the first Construct should be paired.
        let func = make_func(
            vec![owned_param(0, Idx::STR)],
            Idx::STR,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![
                    ArcInstr::RcDec { var: v(0) },
                    ArcInstr::Construct {
                        dst: v(1),
                        ty: Idx::STR,
                        ctor: CtorKind::Struct(Name::from_raw(10)),
                        args: vec![],
                    },
                    ArcInstr::Construct {
                        dst: v(2),
                        ty: Idx::STR,
                        ctor: CtorKind::Struct(Name::from_raw(10)),
                        args: vec![],
                    },
                ],
                terminator: ArcTerminator::Return { value: v(2) },
            }],
            vec![Idx::STR, Idx::STR, Idx::STR],
        );

        let result = run_detect(func);
        let body = &result.blocks[0].body;

        // First pair: Reset/Reuse.
        assert!(matches!(&body[0], ArcInstr::Reset { .. }));
        assert!(matches!(&body[1], ArcInstr::Reuse { .. }));
        // Second Construct: unchanged.
        assert!(
            matches!(&body[2], ArcInstr::Construct { .. }),
            "expected Construct, got {:?}",
            body[2]
        );
    }

    /// Test 6: Multiple pairs — two Dec/Construct pairs, both replaced.
    #[test]
    fn multiple_pairs() {
        // RcDec{v0}; Construct{v2:STR}; RcDec{v1}; Construct{v3:STR}
        let func = make_func(
            vec![owned_param(0, Idx::STR), owned_param(1, Idx::STR)],
            Idx::STR,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![
                    ArcInstr::RcDec { var: v(0) },
                    ArcInstr::Construct {
                        dst: v(2),
                        ty: Idx::STR,
                        ctor: CtorKind::Struct(Name::from_raw(10)),
                        args: vec![],
                    },
                    ArcInstr::RcDec { var: v(1) },
                    ArcInstr::Construct {
                        dst: v(3),
                        ty: Idx::STR,
                        ctor: CtorKind::Struct(Name::from_raw(10)),
                        args: vec![],
                    },
                ],
                terminator: ArcTerminator::Return { value: v(3) },
            }],
            vec![Idx::STR, Idx::STR, Idx::STR, Idx::STR],
        );

        let result = run_detect(func);
        let body = &result.blocks[0].body;

        // Both pairs should be replaced.
        assert!(
            matches!(&body[0], ArcInstr::Reset { var, .. } if *var == v(0)),
            "expected Reset(v0), got {:?}",
            body[0]
        );
        assert!(
            matches!(&body[1], ArcInstr::Reuse { dst, .. } if *dst == v(2)),
            "expected Reuse(v2), got {:?}",
            body[1]
        );
        assert!(
            matches!(&body[2], ArcInstr::Reset { var, .. } if *var == v(1)),
            "expected Reset(v1), got {:?}",
            body[2]
        );
        assert!(
            matches!(&body[3], ArcInstr::Reuse { dst, .. } if *dst == v(3)),
            "expected Reuse(v3), got {:?}",
            body[3]
        );
    }

    /// Test 7: Fresh token ID doesn't collide with existing vars.
    #[test]
    fn fresh_token_id() {
        // var_types has 3 entries (v0, v1, v2). Token should be v3.
        let func = make_func(
            vec![owned_param(0, Idx::STR)],
            Idx::STR,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![
                    ArcInstr::Let {
                        dst: v(1),
                        ty: Idx::INT,
                        value: ArcValue::Literal(LitValue::Int(0)),
                    },
                    ArcInstr::RcDec { var: v(0) },
                    ArcInstr::Construct {
                        dst: v(2),
                        ty: Idx::STR,
                        ctor: CtorKind::Struct(Name::from_raw(10)),
                        args: vec![],
                    },
                ],
                terminator: ArcTerminator::Return { value: v(2) },
            }],
            vec![Idx::STR, Idx::INT, Idx::STR],
        );

        let result = run_detect(func);

        // Token should be ArcVarId(3) — next after v2.
        let body = &result.blocks[0].body;
        match &body[1] {
            ArcInstr::Reset { token, .. } => {
                assert_eq!(token.raw(), 3, "token should be v3");
                // And it should be in var_types.
                assert_eq!(result.var_types.len(), 4);
                assert_eq!(result.var_types[3], Idx::STR);
            }
            other => panic!("expected Reset, got {other:?}"),
        }
    }

    // ── Cross-block reset/reuse tests ──────────────────────────

    use crate::graph::DominatorTree;
    use crate::liveness::compute_refined_liveness;

    use super::detect_reset_reuse_cfg;

    fn run_detect_cfg(mut func: ArcFunction) -> ArcFunction {
        let pool = Pool::new();
        let classifier = ArcClassifier::new(&pool);
        let dom = DominatorTree::build(&func);
        let (refined, _) = compute_refined_liveness(&func, &classifier);
        detect_reset_reuse_cfg(&mut func, &classifier, &dom, &refined);
        func
    }

    /// Cross-block: `RcDec` in entry, Construct in dominated block.
    /// The canonical linked-list `map` pattern.
    #[test]
    fn cross_block_basic() {
        // Block 0:
        //   v1 = apply f(v0)     -- transforms element
        //   RcDec(v0)            -- decrement original node
        //   jump b1
        // Block 1 (dominated by b0):
        //   v2 = Construct(Struct, [v1])  -- allocate new node
        //   return v2
        //
        // After cross-block detection:
        //   Block 0: Reset(v0, token), jump b1
        //   Block 1: Reuse(token, ...), return v2
        let func = make_func(
            vec![owned_param(0, Idx::STR)],
            Idx::STR,
            vec![
                ArcBlock {
                    id: b(0),
                    params: vec![],
                    body: vec![
                        ArcInstr::Apply {
                            dst: v(1),
                            ty: Idx::STR,
                            func: Name::from_raw(99),
                            args: vec![v(0)],
                        },
                        ArcInstr::RcDec { var: v(0) },
                    ],
                    terminator: ArcTerminator::Jump {
                        target: b(1),
                        args: vec![],
                    },
                },
                ArcBlock {
                    id: b(1),
                    params: vec![],
                    body: vec![ArcInstr::Construct {
                        dst: v(2),
                        ty: Idx::STR,
                        ctor: CtorKind::Struct(Name::from_raw(10)),
                        args: vec![v(1)],
                    }],
                    terminator: ArcTerminator::Return { value: v(2) },
                },
            ],
            vec![Idx::STR, Idx::STR, Idx::STR],
        );

        let result = run_detect_cfg(func);

        // Block 0: RcDec should be replaced with Reset.
        let has_reset = result.blocks[0]
            .body
            .iter()
            .any(|i| matches!(i, ArcInstr::Reset { .. }));
        assert!(
            has_reset,
            "block 0 should have Reset after cross-block detection"
        );

        // Block 1: Construct should be replaced with Reuse.
        let has_reuse = result.blocks[1]
            .body
            .iter()
            .any(|i| matches!(i, ArcInstr::Reuse { .. }));
        assert!(
            has_reuse,
            "block 1 should have Reuse after cross-block detection"
        );

        // No RcDec should remain (it was replaced by Reset).
        let has_rc_dec = result.blocks[0]
            .body
            .iter()
            .any(|i| matches!(i, ArcInstr::RcDec { .. }));
        assert!(!has_rc_dec, "block 0 should not have RcDec after pairing");
    }

    /// Cross-block: aliasing prevents pairing (var is live-for-use in target).
    #[test]
    fn cross_block_aliasing_prevents() {
        // Block 0:
        //   RcDec(v0)
        //   jump b1
        // Block 1:
        //   v1 = apply f(v0)     -- v0 is read here → live-for-use
        //   v2 = Construct(Struct, [v1])
        //   return v2
        //
        // v0 is used in b1 → cannot reset in b0.
        let func = make_func(
            vec![owned_param(0, Idx::STR)],
            Idx::STR,
            vec![
                ArcBlock {
                    id: b(0),
                    params: vec![],
                    body: vec![ArcInstr::RcDec { var: v(0) }],
                    terminator: ArcTerminator::Jump {
                        target: b(1),
                        args: vec![],
                    },
                },
                ArcBlock {
                    id: b(1),
                    params: vec![],
                    body: vec![
                        ArcInstr::Apply {
                            dst: v(1),
                            ty: Idx::STR,
                            func: Name::from_raw(99),
                            args: vec![v(0)],
                        },
                        ArcInstr::Construct {
                            dst: v(2),
                            ty: Idx::STR,
                            ctor: CtorKind::Struct(Name::from_raw(10)),
                            args: vec![v(1)],
                        },
                    ],
                    terminator: ArcTerminator::Return { value: v(2) },
                },
            ],
            vec![Idx::STR, Idx::STR, Idx::STR],
        );

        let result = run_detect_cfg(func);

        // Pairing should NOT happen — v0 is used in b1.
        let has_reset = result
            .blocks
            .iter()
            .flat_map(|bl| bl.body.iter())
            .any(|i| matches!(i, ArcInstr::Reset { .. }));
        assert!(!has_reset, "should not pair when aliasing exists");
    }

    /// Cross-block: all existing intra-block tests still pass after cfg detection.
    #[test]
    fn cross_block_preserves_intra_block() {
        // Exact same setup as basic_pair test — intra-block pair should still work.
        let func = make_func(
            vec![owned_param(0, Idx::STR)],
            Idx::STR,
            vec![ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![
                    ArcInstr::RcDec { var: v(0) },
                    ArcInstr::Construct {
                        dst: v(1),
                        ty: Idx::STR,
                        ctor: CtorKind::Struct(Name::from_raw(10)),
                        args: vec![],
                    },
                ],
                terminator: ArcTerminator::Return { value: v(1) },
            }],
            vec![Idx::STR, Idx::STR],
        );

        let result = run_detect_cfg(func);

        // Should pair intra-block.
        let has_reset = result.blocks[0]
            .body
            .iter()
            .any(|i| matches!(i, ArcInstr::Reset { .. }));
        let has_reuse = result.blocks[0]
            .body
            .iter()
            .any(|i| matches!(i, ArcInstr::Reuse { .. }));
        assert!(has_reset, "intra-block Reset should still be detected");
        assert!(has_reuse, "intra-block Reuse should still be detected");
    }
}
