//! Equality chain and range check emitters.
//!
//! LLVM doesn't support `switch` on strings/floats, so we emit sequential
//! `Branch` terminators comparing the scrutinee to each value. Range checks
//! use a similar chain with `lo <= value && value <= hi` tests.

use ori_ir::Span;
use ori_types::Idx;

use crate::ir::{ArcValue, ArcVarId, LitValue, PrimOp};

use super::super::emit::{emit_tree, EmitContext};
use super::super::{DecisionTree, TestValue};

/// Emit an if-else chain for string/float equality dispatch.
pub(in crate::decision_tree) fn emit_str_chain(
    lowerer: &mut crate::lower::ArcLowerer<'_>,
    scrutinee: ArcVarId,
    edges: &[(TestValue, DecisionTree)],
    default: Option<&DecisionTree>,
    ctx: &mut EmitContext,
) {
    for (tv, subtree) in edges {
        // Emit the comparison.
        let expected = emit_test_value_literal(lowerer, tv, ctx.span);
        let cmp = lowerer.builder.emit_let(
            Idx::BOOL,
            ArcValue::PrimOp {
                op: PrimOp::Binary(ori_ir::BinaryOp::Eq),
                args: vec![scrutinee, expected],
            },
            Some(ctx.span),
        );

        let match_block = lowerer.builder.new_block();
        let next_block = lowerer.builder.new_block();
        lowerer
            .builder
            .terminate_branch(cmp, match_block, next_block);

        // Match block: emit the subtree.
        lowerer.builder.position_at(match_block);
        emit_tree(lowerer, subtree, ctx);

        // Continue at next_block for the next comparison.
        lowerer.builder.position_at(next_block);
    }

    // After all comparisons: emit default or unreachable.
    if let Some(default_tree) = default {
        emit_tree(lowerer, default_tree, ctx);
    } else {
        lowerer.builder.terminate_unreachable();
    }
}

/// Emit a comparison chain for int dispatch that mixes exact literals and
/// ranges (`match n { 0, 1..10, 10..100, _ }`).
///
/// Each edge becomes a `bool` test in decision-tree order — an exact `Int`
/// edge tests `scrutinee == v`, an `IntRange` edge tests
/// `lo <= scrutinee && scrutinee </<= hi`. A pure-int `Switch` cannot encode a
/// range (each range would collapse to a single duplicate case), so any column
/// containing a range routes here (`infer_test_kind`).
pub(in crate::decision_tree) fn emit_range_chain(
    lowerer: &mut crate::lower::ArcLowerer<'_>,
    scrutinee: ArcVarId,
    edges: &[(TestValue, DecisionTree)],
    default: Option<&DecisionTree>,
    ctx: &mut EmitContext,
) {
    for (tv, subtree) in edges {
        let matches = match tv {
            TestValue::Int(v) => {
                let expected =
                    lowerer
                        .builder
                        .emit_let(Idx::INT, ArcValue::Literal(LitValue::Int(*v)), None);
                lowerer.builder.emit_let(
                    Idx::BOOL,
                    ArcValue::PrimOp {
                        op: PrimOp::Binary(ori_ir::BinaryOp::Eq),
                        args: vec![scrutinee, expected],
                    },
                    Some(ctx.span),
                )
            }
            TestValue::IntRange { lo, hi, inclusive } => {
                // lo <= scrutinee
                let lo_val =
                    lowerer
                        .builder
                        .emit_let(Idx::INT, ArcValue::Literal(LitValue::Int(*lo)), None);
                let lo_ok = lowerer.builder.emit_let(
                    Idx::BOOL,
                    ArcValue::PrimOp {
                        op: PrimOp::Binary(ori_ir::BinaryOp::GtEq),
                        args: vec![scrutinee, lo_val],
                    },
                    Some(ctx.span),
                );

                // scrutinee <= hi (inclusive) or scrutinee < hi (exclusive)
                let hi_val =
                    lowerer
                        .builder
                        .emit_let(Idx::INT, ArcValue::Literal(LitValue::Int(*hi)), None);
                let hi_op = if *inclusive {
                    PrimOp::Binary(ori_ir::BinaryOp::LtEq)
                } else {
                    PrimOp::Binary(ori_ir::BinaryOp::Lt)
                };
                let hi_ok = lowerer.builder.emit_let(
                    Idx::BOOL,
                    ArcValue::PrimOp {
                        op: hi_op,
                        args: vec![scrutinee, hi_val],
                    },
                    Some(ctx.span),
                );

                // lo_ok && hi_ok
                lowerer.builder.emit_let(
                    Idx::BOOL,
                    ArcValue::PrimOp {
                        op: PrimOp::Binary(ori_ir::BinaryOp::And),
                        args: vec![lo_ok, hi_ok],
                    },
                    Some(ctx.span),
                )
            }
            // Other kinds never reach the range chain (infer_test_kind routes
            // them elsewhere); skip defensively rather than mis-compile.
            _ => continue,
        };

        let match_block = lowerer.builder.new_block();
        let next_block = lowerer.builder.new_block();
        lowerer
            .builder
            .terminate_branch(matches, match_block, next_block);

        lowerer.builder.position_at(match_block);
        emit_tree(lowerer, subtree, ctx);

        lowerer.builder.position_at(next_block);
    }

    if let Some(default_tree) = default {
        emit_tree(lowerer, default_tree, ctx);
    } else {
        lowerer.builder.terminate_unreachable();
    }
}

/// Emit an if-else chain for list-length dispatch.
///
/// A list pattern tests the list's LENGTH, but the test is not pure equality:
/// an exact-length pattern (`[a, b]`) matches `len == N`, while a rest pattern
/// (`[head, ..tail]`) matches `len >= N`. A `Switch` cannot express `>=`, and an
/// exact arm and a rest arm of the same minimum length collide as duplicate
/// switch cases — so list-length dispatch is a comparison chain, like ranges.
///
/// `scrutinee` is the extracted list length (an `int`), in decision-tree
/// (first-match) order: each exact edge tests `len == N`, each rest edge tests
/// `len >= N`.
pub(in crate::decision_tree) fn emit_list_len_chain(
    lowerer: &mut crate::lower::ArcLowerer<'_>,
    scrutinee: ArcVarId,
    edges: &[(TestValue, DecisionTree)],
    default: Option<&DecisionTree>,
    ctx: &mut EmitContext,
) {
    for (tv, subtree) in edges {
        if let TestValue::ListLen { len, is_exact } = tv {
            let len_val = lowerer.builder.emit_let(
                Idx::INT,
                ArcValue::Literal(LitValue::Int(i64::from(*len))),
                None,
            );
            // Exact length: `len == N`. Rest pattern: `len >= N`.
            let op = if *is_exact {
                PrimOp::Binary(ori_ir::BinaryOp::Eq)
            } else {
                PrimOp::Binary(ori_ir::BinaryOp::GtEq)
            };
            let matches = lowerer.builder.emit_let(
                Idx::BOOL,
                ArcValue::PrimOp {
                    op,
                    args: vec![scrutinee, len_val],
                },
                Some(ctx.span),
            );

            let match_block = lowerer.builder.new_block();
            let next_block = lowerer.builder.new_block();
            lowerer
                .builder
                .terminate_branch(matches, match_block, next_block);

            lowerer.builder.position_at(match_block);
            emit_tree(lowerer, subtree, ctx);

            lowerer.builder.position_at(next_block);
        }
    }

    if let Some(default_tree) = default {
        emit_tree(lowerer, default_tree, ctx);
    } else {
        lowerer.builder.terminate_unreachable();
    }
}

/// Emit a literal value corresponding to a `TestValue`.
fn emit_test_value_literal(
    lowerer: &mut crate::lower::ArcLowerer<'_>,
    tv: &TestValue,
    span: Span,
) -> ArcVarId {
    match tv {
        TestValue::Str(name) => lowerer.builder.emit_let(
            Idx::STR,
            ArcValue::Literal(LitValue::String(*name)),
            Some(span),
        ),
        TestValue::Float(bits) => lowerer.builder.emit_let(
            Idx::FLOAT,
            ArcValue::Literal(LitValue::Int((*bits).cast_signed())),
            Some(span),
        ),
        TestValue::Char(c) => lowerer.builder.emit_let(
            Idx::INT,
            ArcValue::Literal(LitValue::Int(i64::from(u32::from(*c)))),
            Some(span),
        ),
        // Int, Bool, Tag, IntRange, ListLen use emit_int_switch/emit_tag_switch, not this chain.
        _ => {
            tracing::warn!(
                ?tv,
                "unexpected TestValue in emit_test_value_literal; emitting 0"
            );
            lowerer
                .builder
                .emit_let(Idx::INT, ArcValue::Literal(LitValue::Int(0)), Some(span))
        }
    }
}
