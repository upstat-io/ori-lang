//! Pattern matching compilation.

use inkwell::values::{BasicValueEnum, FunctionValue};
use ori_ir::ast::patterns::MatchPattern;
use ori_ir::ast::ExprKind;
use ori_ir::{ArmRange, ExprArena, ExprId};
use ori_types::{Idx, PatternKey, PatternResolution};
use tracing::{debug, instrument, trace, warn};

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

        // Remember range start for PatternKey computation
        let arm_range_start = arms.start;

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

        debug!(
            scrutinee_type = ?scrutinee_val.get_type(),
            arm_count = arms.len(),
            "matching scrutinee"
        );

        // Process each arm
        for (i, arm) in arms.iter().enumerate() {
            let is_last = i == arms.len() - 1;
            trace!(arm = i, pattern = ?arm.pattern, "compiling match arm");

            // Create blocks for this arm
            let arm_body_bb = self.append_block(function, &format!("match_arm_{i}"));
            // For non-last arms, create a next block; for last arm, go to unreachable
            // (exhaustive matches should never reach this path)
            let next_bb = if is_last {
                unreachable_bb
            } else {
                self.append_block(function, &format!("match_next_{i}"))
            };

            // Compute pattern key for this arm (matches type checker's PatternKey::Arm)
            let arm_key = PatternKey::Arm(arm_range_start + i as u32);

            // Check the pattern
            let matches = self.compile_pattern_check(
                &arm.pattern,
                scrutinee_val,
                arena,
                expr_types,
                Some(arm_key),
            );

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
            self.bind_match_pattern_vars(&arm.pattern, scrutinee_val, arena, locals, Some(arm_key));

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

            // Track incoming value for phi, ensuring type consistency.
            // Match arms can produce values with different LLVM types when the type
            // checker leaves unresolved type variables (Tag::Var) that map to i64,
            // while the actual compiled value has a different type (e.g., str struct).
            // Coerce to the expected result type to maintain phi node type consistency.
            if let Some(val) = body_val {
                let expected_type = self.cx().llvm_type(result_type);
                let coerced = if val.get_type() == expected_type {
                    val
                } else {
                    self.coerce_value_to_type(val, expected_type)
                };
                incoming.push((coerced, arm_exit_bb));
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
    ///
    /// `arm_key` is the pattern key for looking up type-checker resolutions.
    #[instrument(
        skip(self, pattern, scrutinee, arena, _expr_types, arm_key),
        level = "trace"
    )]
    fn compile_pattern_check(
        &self,
        pattern: &MatchPattern,
        scrutinee: BasicValueEnum<'ll>,
        arena: &ExprArena,
        _expr_types: &[Idx],
        arm_key: Option<PatternKey>,
    ) -> Option<inkwell::values::IntValue<'ll>> {
        match pattern {
            MatchPattern::Binding(_) => {
                // Check if the type checker resolved this binding as a unit variant
                if let Some(key) = arm_key {
                    if let Some(PatternResolution::UnitVariant { variant_index, .. }) =
                        self.cx().resolve_pattern(key)
                    {
                        // This Binding was resolved to a unit variant — compare tags
                        let BasicValueEnum::StructValue(struct_val) = scrutinee else {
                            return None;
                        };
                        let tag = self.extract_value(struct_val, 0, "tag")?;
                        let BasicValueEnum::IntValue(tag_int) = tag else {
                            return None;
                        };
                        let expected = self
                            .cx()
                            .scx
                            .type_i8()
                            .const_int(u64::from(variant_index), false);
                        return Some(self.icmp(
                            inkwell::IntPredicate::EQ,
                            tag_int,
                            expected,
                            "variant_match",
                        ));
                    }
                }
                // Normal binding — always matches
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
                // Unified tag lookup via SumTypeLayout — works for both built-in
                // (Option/Result) and user-defined sum types.
                let BasicValueEnum::StructValue(struct_val) = scrutinee else {
                    return None;
                };

                let tag = self.extract_value(struct_val, 0, "tag")?;
                let BasicValueEnum::IntValue(tag_int) = tag else {
                    return None;
                };

                // Look up tag from SumTypeLayout (registered at init for builtins,
                // during type registration for user-defined sum types)
                let expected_tag =
                    if let Some((_, variant)) = self.cx().lookup_variant_constructor(*name) {
                        u64::from(variant.tag)
                    } else {
                        // Fallback for unregistered variants (shouldn't happen in well-typed code)
                        let variant_name = self.cx().interner.lookup(*name);
                        warn!(
                            variant = variant_name,
                            "unregistered variant in pattern match"
                        );
                        0
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
    /// Match pattern bindings are always immutable. `arm_key` is used to look up
    /// type-checker resolutions for ambiguous Binding patterns.
    #[instrument(
        skip(self, pattern, scrutinee, arena, locals, arm_key),
        level = "trace"
    )]
    fn bind_match_pattern_vars(
        &self,
        pattern: &MatchPattern,
        scrutinee: BasicValueEnum<'ll>,
        arena: &ExprArena,
        locals: &mut Locals<'ll>,
        arm_key: Option<PatternKey>,
    ) {
        match pattern {
            MatchPattern::Binding(name) => {
                // If resolved as a unit variant, do NOT bind — it's a constructor, not a variable
                if let Some(key) = arm_key {
                    if matches!(
                        self.cx().resolve_pattern(key),
                        Some(PatternResolution::UnitVariant { .. })
                    ) {
                        return;
                    }
                }
                locals.bind_immutable(*name, scrutinee);
            }

            MatchPattern::Variant { name, inner } => {
                // For variants like Some(x) or Click(x, y), extract and bind inner values
                let inner_ids = arena.get_match_pattern_list(*inner);
                if inner_ids.is_empty() {
                    return;
                }

                let BasicValueEnum::StructValue(struct_val) = scrutinee else {
                    return;
                };

                // Check if this is a user-defined sum type (payload_i64_count > 0)
                // vs a built-in (Option/Result) with direct payload extraction
                let is_user_defined = self
                    .cx()
                    .lookup_variant_constructor(*name)
                    .and_then(|(type_name, _)| self.cx().get_sum_type_layout(type_name))
                    .is_some_and(|layout| layout.payload_i64_count > 0);

                if is_user_defined {
                    // User-defined sum type: payload is [M x i64] array at field 1.
                    // Extract fields via alloca + byte-addressed GEP.
                    self.bind_user_sum_type_payload(*name, struct_val, inner_ids, arena, locals);
                } else {
                    // Built-in sum type (Option/Result): payload is directly at field 1.
                    self.bind_builtin_variant_payload(struct_val, inner_ids, arena, locals);
                }
            }

            MatchPattern::At { name, pattern } => {
                // Bind the whole value to name, then process inner pattern
                locals.bind_immutable(*name, scrutinee);
                let inner = arena.get_match_pattern(*pattern);
                self.bind_match_pattern_vars(inner, scrutinee, arena, locals, None);
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
                            self.bind_match_pattern_vars(pat, elem, arena, locals, None);
                        }
                    }
                }
            }

            _ => {
                // Other patterns don't bind variables (Wildcard, Literal, etc.)
            }
        }
    }

    /// Bind inner patterns for built-in variant types (Option/Result).
    ///
    /// These have layout `{ i8 tag, T payload }` — payload is directly at field 1.
    fn bind_builtin_variant_payload(
        &self,
        struct_val: inkwell::values::StructValue<'ll>,
        inner_ids: &[ori_ir::MatchPatternId],
        arena: &ExprArena,
        locals: &mut Locals<'ll>,
    ) {
        if inner_ids.len() == 1 {
            if let Some(payload) = self.extract_value(struct_val, 1, "payload") {
                let inner_pattern = arena.get_match_pattern(inner_ids[0]);
                self.bind_match_pattern_vars(inner_pattern, payload, arena, locals, None);
            }
        } else {
            // Multi-field: payload is a tuple struct
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
                        if let Some(elem) =
                            self.extract_value(tuple_val, idx, &format!("variant_field_{i}"))
                        {
                            let inner_pattern = arena.get_match_pattern(*pat_id);
                            self.bind_match_pattern_vars(inner_pattern, elem, arena, locals, None);
                        }
                    }
                }
            }
        }
    }

    /// Bind inner patterns for user-defined sum types.
    ///
    /// These have layout `{ i8 tag, [M x i64] payload }`. Fields are extracted
    /// via alloca + byte-addressed GEP into the payload array.
    fn bind_user_sum_type_payload(
        &self,
        variant_name: ori_ir::Name,
        struct_val: inkwell::values::StructValue<'ll>,
        inner_ids: &[ori_ir::MatchPatternId],
        arena: &ExprArena,
        locals: &mut Locals<'ll>,
    ) {
        let Some((type_name, variant)) = self.cx().lookup_variant_constructor(variant_name) else {
            return;
        };
        let Some(_layout) = self.cx().get_sum_type_layout(type_name) else {
            return;
        };
        let Some(struct_ty) = self.cx().get_struct_type(type_name) else {
            return;
        };

        // Store the struct value to an alloca so we can GEP into the payload
        let function = self.get_current_function();
        let alloca = self.create_entry_alloca(function, "match_val", struct_ty.into());
        self.store(struct_val.into(), alloca);

        // GEP to payload field (index 1)
        let payload_ptr = self.struct_gep(struct_ty, alloca, 1, "payload_ptr");

        // Extract each field at byte offsets
        let mut byte_offset: u32 = 0;
        for (i, pat_id) in inner_ids.iter().enumerate() {
            let field_ty = variant.field_types.get(i).copied().unwrap_or(Idx::INT);
            let field_size = crate::module::field_byte_size(field_ty);

            // GEP to byte offset
            let i8_ty = self.cx().scx.type_i8();
            let offset = i8_ty.const_int(u64::from(byte_offset), false);
            let field_ptr = self.gep(
                i8_ty.into(),
                payload_ptr,
                &[offset],
                &format!("field_{i}_ptr"),
            );

            // Load the field value with its LLVM type
            let llvm_ty = self.cx().llvm_type(field_ty);
            let field_val = self.load(llvm_ty, field_ptr, &format!("field_{i}"));

            let inner_pattern = arena.get_match_pattern(*pat_id);
            self.bind_match_pattern_vars(inner_pattern, field_val, arena, locals, None);

            byte_offset += field_size;
        }
    }
}
