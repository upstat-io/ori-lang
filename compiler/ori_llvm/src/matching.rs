//! Pattern matching compilation.

use std::collections::HashMap;

use inkwell::values::{BasicValueEnum, FunctionValue};
use ori_ir::ast::patterns::MatchPattern;
use ori_ir::ast::ExprKind;
use ori_ir::{ArmRange, ExprArena, ExprId, Name, TypeId};

use crate::{LLVMCodegen, LoopContext};

impl<'ctx> LLVMCodegen<'ctx> {
    /// Compile a match expression.
    ///
    /// Match expressions are compiled as a series of conditional branches:
    /// 1. Evaluate scrutinee
    /// 2. For each arm: check pattern, if match execute body, else try next arm
    /// 3. Use phi node to merge results from all arms
    pub(crate) fn compile_match(
        &self,
        scrutinee: ExprId,
        arms: ArmRange,
        result_type: TypeId,
        arena: &ExprArena,
        expr_types: &[TypeId],
        locals: &mut HashMap<Name, BasicValueEnum<'ctx>>,
        function: FunctionValue<'ctx>,
        loop_ctx: Option<&LoopContext<'ctx>>,
    ) -> Option<BasicValueEnum<'ctx>> {
        // Compile the scrutinee
        let scrutinee_val = self.compile_expr(scrutinee, arena, expr_types, locals, function, loop_ctx)?;

        // Get the arms
        let arms = arena.get_arms(arms);

        if arms.is_empty() {
            // No arms - return default value
            return if result_type == TypeId::VOID {
                None
            } else {
                Some(self.default_value(result_type))
            };
        }

        // Create merge block for all arms
        let merge_bb = self.context.append_basic_block(function, "match_merge");

        // Track incoming values for the phi node
        let mut incoming: Vec<(BasicValueEnum<'ctx>, inkwell::basic_block::BasicBlock<'ctx>)> = Vec::new();

        // Process each arm
        for (i, arm) in arms.iter().enumerate() {
            let is_last = i == arms.len() - 1;

            // Create blocks for this arm
            let arm_body_bb = self.context.append_basic_block(function, &format!("match_arm_{i}"));
            let next_bb = if is_last {
                merge_bb // Last arm falls through to merge (or unreachable)
            } else {
                self.context.append_basic_block(function, &format!("match_next_{i}"))
            };

            // Check the pattern
            let matches = self.compile_pattern_check(&arm.pattern, scrutinee_val, arena, expr_types);

            if let Some(cond) = matches {
                // Conditional branch based on pattern match
                self.builder.build_conditional_branch(cond, arm_body_bb, next_bb).ok()?;
            } else {
                // Pattern always matches (wildcard, binding)
                self.builder.build_unconditional_branch(arm_body_bb).ok()?;
            }

            // Compile arm body
            self.builder.position_at_end(arm_body_bb);

            // Bind pattern variables
            self.bind_pattern_vars(&arm.pattern, scrutinee_val, locals);

            // Compile guard if present
            if let Some(guard) = arm.guard {
                let guard_val = self.compile_expr(guard, arena, expr_types, locals, function, loop_ctx)?;
                let guard_bool = guard_val.into_int_value();

                // If guard fails, go to next arm
                let guard_pass_bb = self.context.append_basic_block(function, &format!("guard_pass_{i}"));
                self.builder.build_conditional_branch(guard_bool, guard_pass_bb, next_bb).ok()?;
                self.builder.position_at_end(guard_pass_bb);
            }

            // Compile arm body
            let body_val = self.compile_expr(arm.body, arena, expr_types, locals, function, loop_ctx);

            // Jump to merge block
            let arm_exit_bb = self.builder.get_insert_block()?;
            if arm_exit_bb.get_terminator().is_none() {
                self.builder.build_unconditional_branch(merge_bb).ok()?;
            }

            // Track incoming value for phi
            if let Some(val) = body_val {
                incoming.push((val, arm_exit_bb));
            }

            // Position at next arm's check block
            if !is_last {
                self.builder.position_at_end(next_bb);
            }
        }

        // Build merge block
        self.builder.position_at_end(merge_bb);

        // Create phi node if we have values
        if incoming.is_empty() {
            None
        } else if incoming.len() == 1 {
            // Single arm - just use the value
            Some(incoming[0].0)
        } else {
            // Multiple arms - need phi node
            let phi = self.build_phi(result_type, &incoming)?;
            Some(phi.as_basic_value())
        }
    }

    /// Check if a pattern matches the scrutinee.
    /// Returns Some(condition) if a runtime check is needed, None if always matches.
    fn compile_pattern_check(
        &self,
        pattern: &MatchPattern,
        scrutinee: BasicValueEnum<'ctx>,
        arena: &ExprArena,
        _expr_types: &[TypeId],
    ) -> Option<inkwell::values::IntValue<'ctx>> {
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
                        let expected = self.context.i64_type().const_int(*n as u64, true);
                        let actual = scrutinee.into_int_value();
                        Some(self.builder.build_int_compare(
                            inkwell::IntPredicate::EQ,
                            actual,
                            expected,
                            "lit_match",
                        ).ok()?)
                    }
                    ExprKind::Bool(b) => {
                        let expected = self.context.bool_type().const_int(u64::from(*b), false);
                        let actual = scrutinee.into_int_value();
                        Some(self.builder.build_int_compare(
                            inkwell::IntPredicate::EQ,
                            actual,
                            expected,
                            "bool_match",
                        ).ok()?)
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
                let struct_val = match scrutinee {
                    BasicValueEnum::StructValue(sv) => sv,
                    _ => return None, // Can't match variant on non-struct
                };

                // Extract tag
                let tag = self.builder.build_extract_value(struct_val, 0, "tag").ok()?;
                let tag_int = tag.into_int_value();

                // Get expected tag based on variant name
                let variant_name = self.interner.lookup(*name);
                let expected_tag = match variant_name {
                    "None" => 0,
                    "Some" => 1,
                    "Ok" => 0,
                    "Err" => 1,
                    _ => 0, // Unknown variant - assume tag 0
                };

                let expected = self.context.i8_type().const_int(expected_tag, false);
                Some(self.builder.build_int_compare(
                    inkwell::IntPredicate::EQ,
                    tag_int,
                    expected,
                    "variant_match",
                ).ok()?)
            }

            // Other patterns - treat as always match for now
            _ => None,
        }
    }

    /// Bind pattern variables to the scrutinee value.
    fn bind_pattern_vars(
        &self,
        pattern: &MatchPattern,
        scrutinee: BasicValueEnum<'ctx>,
        locals: &mut HashMap<Name, BasicValueEnum<'ctx>>,
    ) {
        match pattern {
            MatchPattern::Binding(name) => {
                locals.insert(*name, scrutinee);
            }

            MatchPattern::Variant { name: _, inner } => {
                // For variants like Some(x), extract the inner value and bind it
                if let Some(inner_pattern) = inner {
                    // Extract the payload from the tagged union
                    if let BasicValueEnum::StructValue(struct_val) = scrutinee {
                        if let Ok(payload) = self.builder.build_extract_value(struct_val, 1, "payload") {
                            self.bind_pattern_vars(inner_pattern, payload, locals);
                        }
                    }
                }
            }

            MatchPattern::At { name, pattern } => {
                // Bind the whole value to name, then process inner pattern
                locals.insert(*name, scrutinee);
                self.bind_pattern_vars(pattern, scrutinee, locals);
            }

            MatchPattern::Tuple(patterns) => {
                // Bind each tuple element
                if let BasicValueEnum::StructValue(struct_val) = scrutinee {
                    for (i, pat) in patterns.iter().enumerate() {
                        if let Ok(elem) = self.builder.build_extract_value(struct_val, i as u32, &format!("tuple_{i}")) {
                            self.bind_pattern_vars(pat, elem, locals);
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
