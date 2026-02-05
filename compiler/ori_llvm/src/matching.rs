//! Pattern matching compilation.

use inkwell::values::{BasicValueEnum, FunctionValue};
use ori_ir::ast::patterns::MatchPattern;
use ori_ir::ast::ExprKind;
use ori_ir::{ArmRange, ExprArena, ExprId};
use ori_types::Idx;
use tracing::instrument;

use crate::builder::{Builder, Locals};
use crate::LoopContext;

impl<'ll> Builder<'_, 'll, '_> {
    /// Compile a match expression.
    ///
    /// Match expressions are compiled as a series of conditional branches:
    /// 1. Evaluate scrutinee
    /// 2. For each arm: check pattern, if match execute body, else try next arm
    /// 3. Use phi node to merge results from all arms
    #[instrument(
        skip(self, arena, expr_types, locals, function, loop_ctx),
        level = "debug"
    )]
    pub(crate) fn compile_match(
        &self,
        scrutinee: ExprId,
        arms: ArmRange,
        result_type: Idx,
        arena: &ExprArena,
        expr_types: &[Idx],
        locals: &mut Locals<'ll>,
        function: FunctionValue<'ll>,
        loop_ctx: Option<&LoopContext<'ll>>,
    ) -> Option<BasicValueEnum<'ll>> {
        // Compile the scrutinee
        let scrutinee_val =
            self.compile_expr(scrutinee, arena, expr_types, locals, function, loop_ctx)?;

        // Get the arms
        let arms = arena.get_arms(arms);

        if arms.is_empty() {
            // No arms - return default value
            return if result_type == Idx::UNIT {
                None
            } else {
                Some(self.cx().default_value(result_type))
            };
        }

        // Create merge block for all arms
        let merge_bb = self.append_block(function, "match_merge");

        // Create unreachable block for match exhaustiveness (should never be reached)
        let unreachable_bb = self.append_block(function, "match_unreachable");

        // Track incoming values for the phi node
        let mut incoming: Vec<(BasicValueEnum<'ll>, inkwell::basic_block::BasicBlock<'ll>)> =
            Vec::new();

        // Process each arm
        for (i, arm) in arms.iter().enumerate() {
            let is_last = i == arms.len() - 1;

            // Create blocks for this arm
            let arm_body_bb = self.append_block(function, &format!("match_arm_{i}"));
            // For non-last arms, create a next block; for last arm, go to unreachable
            // (exhaustive matches should never reach this path)
            let next_bb = if is_last {
                unreachable_bb
            } else {
                self.append_block(function, &format!("match_next_{i}"))
            };

            // Check the pattern
            let matches =
                self.compile_pattern_check(&arm.pattern, scrutinee_val, arena, expr_types);

            if let Some(cond) = matches {
                // Conditional branch based on pattern match
                self.cond_br(cond, arm_body_bb, next_bb);
            } else {
                // Pattern always matches (wildcard, binding)
                self.br(arm_body_bb);
            }

            // Compile arm body
            self.position_at_end(arm_body_bb);

            // Bind pattern variables
            self.bind_match_pattern_vars(&arm.pattern, scrutinee_val, arena, locals);

            // Compile guard if present
            if let Some(guard) = arm.guard {
                let guard_val =
                    self.compile_expr(guard, arena, expr_types, locals, function, loop_ctx)?;
                let guard_bool = guard_val.into_int_value();

                // If guard fails, go to next arm
                let guard_pass_bb = self.append_block(function, &format!("guard_pass_{i}"));
                self.cond_br(guard_bool, guard_pass_bb, next_bb);
                self.position_at_end(guard_pass_bb);
            }

            // Compile arm body
            let body_val =
                self.compile_expr(arm.body, arena, expr_types, locals, function, loop_ctx);

            // Jump to merge block
            let arm_exit_bb = self.current_block()?;
            if arm_exit_bb.get_terminator().is_none() {
                self.br(merge_bb);
            }

            // Track incoming value for phi
            if let Some(val) = body_val {
                incoming.push((val, arm_exit_bb));
            }

            // Position at next arm's check block
            if !is_last {
                self.position_at_end(next_bb);
            }
        }

        // Build unreachable block (for exhaustiveness - should never be reached)
        self.position_at_end(unreachable_bb);
        self.unreachable();

        // Build merge block
        self.position_at_end(merge_bb);

        // Create phi node if we have values
        // build_phi_from_incoming handles single-value and multi-value cases
        self.build_phi_from_incoming(result_type, &incoming)
    }

    /// Check if a pattern matches the scrutinee.
    /// Returns Some(condition) if a runtime check is needed, None if always matches.
    #[instrument(skip(self, pattern, scrutinee, arena, _expr_types), level = "trace")]
    fn compile_pattern_check(
        &self,
        pattern: &MatchPattern,
        scrutinee: BasicValueEnum<'ll>,
        arena: &ExprArena,
        _expr_types: &[Idx],
    ) -> Option<inkwell::values::IntValue<'ll>> {
        match pattern {
            MatchPattern::Wildcard | MatchPattern::Binding(_) => {
                // Always matches
                None
            }

            MatchPattern::Literal(expr_id) => {
                // Compare with literal value
                let literal_expr = arena.get_expr(*expr_id);
                match &literal_expr.kind {
                    ExprKind::Int(n) => {
                        let expected = self.cx().scx.type_i64().const_int(*n as u64, true);
                        let actual = scrutinee.into_int_value();
                        Some(self.icmp(inkwell::IntPredicate::EQ, actual, expected, "lit_match"))
                    }
                    ExprKind::Bool(b) => {
                        let expected = self.cx().scx.type_i1().const_int(u64::from(*b), false);
                        let actual = scrutinee.into_int_value();
                        Some(self.icmp(inkwell::IntPredicate::EQ, actual, expected, "bool_match"))
                    }
                    _ => {
                        // Unsupported literal type - treat as always match for now
                        None
                    }
                }
            }

            MatchPattern::Variant { name, inner: _ } => {
                // For Option/Result, check the tag
                // Scrutinee should be a struct { i8 tag, T value }
                let BasicValueEnum::StructValue(struct_val) = scrutinee else {
                    return None; // Can't match variant on non-struct
                };

                // Extract tag - must be the first field and must be an integer
                let tag = self.extract_value(struct_val, 0, "tag")?;
                let BasicValueEnum::IntValue(tag_int) = tag else {
                    // Tag is not an integer - malformed Option/Result struct
                    // This can happen if types are mismatched; treat as no match
                    return None;
                };

                // Get expected tag based on variant name
                let variant_name = self.cx().interner.lookup(*name);
                let expected_tag = match variant_name {
                    "Some" | "Err" => 1,
                    // None, Ok, and unknown variants use tag 0
                    _ => 0,
                };

                let expected = self.cx().scx.type_i8().const_int(expected_tag, false);
                Some(self.icmp(
                    inkwell::IntPredicate::EQ,
                    tag_int,
                    expected,
                    "variant_match",
                ))
            }

            // Other patterns - treat as always match for now
            _ => None,
        }
    }

    /// Bind pattern variables to the scrutinee value.
    ///
    /// Match pattern bindings are always immutable.
    #[instrument(skip(self, pattern, scrutinee, arena, locals), level = "trace")]
    fn bind_match_pattern_vars(
        &self,
        pattern: &MatchPattern,
        scrutinee: BasicValueEnum<'ll>,
        arena: &ExprArena,
        locals: &mut Locals<'ll>,
    ) {
        match pattern {
            MatchPattern::Binding(name) => {
                locals.bind_immutable(*name, scrutinee);
            }

            MatchPattern::Variant { name: _, inner } => {
                // For variants like Some(x) or Click(x, y), extract and bind inner values
                let inner_ids = arena.get_match_pattern_list(*inner);
                if !inner_ids.is_empty() {
                    if let BasicValueEnum::StructValue(struct_val) = scrutinee {
                        if inner_ids.len() == 1 {
                            // Single field variant: payload is the value directly
                            if let Some(payload) = self.extract_value(struct_val, 1, "payload") {
                                let inner_pattern = arena.get_match_pattern(inner_ids[0]);
                                self.bind_match_pattern_vars(inner_pattern, payload, arena, locals);
                            }
                        } else {
                            // Multi-field variant: payload is a tuple, extract each element
                            #[expect(
                                clippy::collapsible_match,
                                reason = "Separate if-lets for clarity: first extract, then type-check"
                            )]
                            if let Some(payload) = self.extract_value(struct_val, 1, "payload") {
                                if let BasicValueEnum::StructValue(tuple_val) = payload {
                                    for (i, pat_id) in inner_ids.iter().enumerate() {
                                        #[expect(
                                            clippy::cast_possible_truncation,
                                            reason = "variant field count fits in u32"
                                        )]
                                        let idx = i as u32;
                                        if let Some(elem) = self.extract_value(
                                            tuple_val,
                                            idx,
                                            &format!("variant_field_{i}"),
                                        ) {
                                            let inner_pattern = arena.get_match_pattern(*pat_id);
                                            self.bind_match_pattern_vars(
                                                inner_pattern,
                                                elem,
                                                arena,
                                                locals,
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            MatchPattern::At { name, pattern } => {
                // Bind the whole value to name, then process inner pattern
                locals.bind_immutable(*name, scrutinee);
                let inner = arena.get_match_pattern(*pattern);
                self.bind_match_pattern_vars(inner, scrutinee, arena, locals);
            }

            MatchPattern::Tuple(patterns) => {
                // Bind each tuple element
                let pattern_ids = arena.get_match_pattern_list(*patterns);
                if let BasicValueEnum::StructValue(struct_val) = scrutinee {
                    for (i, pat_id) in pattern_ids.iter().enumerate() {
                        #[expect(
                            clippy::cast_possible_truncation,
                            reason = "tuple element count fits in u32"
                        )]
                        let idx = i as u32;
                        if let Some(elem) =
                            self.extract_value(struct_val, idx, &format!("tuple_{i}"))
                        {
                            let pat = arena.get_match_pattern(*pat_id);
                            self.bind_match_pattern_vars(pat, elem, arena, locals);
                        }
                    }
                }
            }

            _ => {
                // Other patterns don't bind variables (Wildcard, Literal, etc.)
            }
        }
    }
}
