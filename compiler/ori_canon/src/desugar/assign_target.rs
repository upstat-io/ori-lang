//! Type-directed assignment-chain desugaring.
//!
//! Indexed and field assignments become pure root reassignments before AIMS.
//! Chains wrap right-to-left using the type checker's [`ori_types::AssignDesugar`]
//! plan. Potentially effectful index expressions are hoisted and reused, so
//! compound assignment evaluates each index exactly once.

use ori_ir::canon::{CanExpr, CanField, CanId};
use ori_ir::{AccessStep, AccessStepRange, ExprId, ExprKind, Mutability, Name, Span, TypeId};

use crate::lower::Lowerer;

/// Key strategy for one index step of an assignment-target chain.
#[derive(Copy, Clone)]
enum IndexKey {
    /// Trivially-pure index re-lowered directly at each use.
    Direct(ExprId),
    /// Side-effecting index hoisted to a `let $__assign_idx_N` temp ident.
    Temp(CanId),
    /// A field step — carries no index key.
    None,
}

struct ReadChainPlan<'a> {
    root_name: Name,
    steps: &'a [AccessStep],
    step_routes: &'a [ori_types::AssignStepRoute],
    index_keys: &'a [IndexKey],
    span: Span,
    level_types: &'a [TypeId],
}

impl Lowerer<'_> {
    /// Desugar one `Assign { target: AssignTarget{root, steps}, value }` into
    /// the pure-reassignment form. `assign_target_id` keys the
    /// [`ori_types::AssignDesugar`] plan; `outer_ty` is the assignment
    /// expression's own type (`void`/unit — the reassignment result).
    pub(crate) fn desugar_assign_target(
        &mut self,
        assign_target_id: ExprId,
        root: ExprId,
        steps: AccessStepRange,
        value: ExprId,
        span: Span,
        outer_ty: TypeId,
    ) -> CanId {
        let ExprKind::Ident(root_name) = *self.src.expr_kind(root) else {
            // Preserve the fallback diagnostic for an invalid root shape.
            return self.lower_assign_chain_fallback(root, steps, value, span, outer_ty);
        };

        let (level_types, step_routes): (Vec<TypeId>, Vec<ori_types::AssignStepRoute>) =
            match self.typed.resolve_assign_desugar(assign_target_id) {
                Some(plan) => (
                    plan.level_types
                        .iter()
                        .map(|idx| TypeId::from_raw(idx.raw()))
                        .collect(),
                    plan.step_routes.clone(),
                ),
                None => {
                    return self.lower_assign_chain_fallback(root, steps, value, span, outer_ty);
                }
            };

        let step_list: Vec<AccessStep> = self.src.get_access_steps(steps).to_vec();

        // `level_types[k]` is the type of reading `root` plus the first `k`
        // steps. A well-formed plan carries `steps + 1` entries.
        let routes_match = step_list.iter().zip(&step_routes).all(|(step, route)| {
            matches!(
                (step, route),
                (AccessStep::Field(_), ori_types::AssignStepRoute::Field)
                    | (AccessStep::Index(_), ori_types::AssignStepRoute::Index(_))
            )
        });
        if level_types.len() != step_list.len() + 1
            || step_routes.len() != step_list.len()
            || !routes_match
        {
            return self.lower_assign_chain_fallback(root, steps, value, span, outer_ty);
        }

        // Plan each index once so compound-assignment reads reuse its key.
        let (temp_lets, index_keys) = self.plan_index_keys(&step_list, span);
        let read_chain = ReadChainPlan {
            root_name,
            steps: &step_list,
            step_routes: &step_routes,
            index_keys: &index_keys,
            span,
            level_types: &level_types,
        };

        // Active temp overrides also apply to the parser-shared read copy.
        let value_can = self.lower_expr(value);
        let inner = self.build_assign_value(&read_chain, value_can);

        // Zero steps defensively collapse to assigning `value_can` directly.
        let target = self.push(CanExpr::Ident(root_name), span, level_types[0]);
        let assign = self.push(
            CanExpr::Assign {
                target,
                value: inner,
            },
            span,
            outer_ty,
        );

        // Nested assignments populate their own overrides.
        self.index_temp_overrides.clear();

        if temp_lets.is_empty() {
            return assign;
        }

        let stmts = self.arena.push_expr_list(&temp_lets);
        self.push(
            CanExpr::Block {
                stmts,
                result: assign,
            },
            span,
            outer_ty,
        )
    }

    fn build_assign_value(&mut self, plan: &ReadChainPlan<'_>, value: CanId) -> CanId {
        let mut inner = value;
        for k in (0..plan.steps.len()).rev() {
            let receiver = self.synth_read_chain(plan, k);
            inner = match plan.steps[k] {
                AccessStep::Index(_) => {
                    let key = self.lower_index_key(plan.index_keys[k], plan.span);
                    let args = self.arena.push_expr_list(&[key, inner]);
                    self.push(
                        CanExpr::MethodCall {
                            receiver,
                            method: self.name_updated,
                            args,
                        },
                        plan.span,
                        plan.level_types[k],
                    )
                }
                AccessStep::Field(field) => {
                    self.synth_struct_update(receiver, plan.level_types[k], field, inner, plan.span)
                }
            };
        }
        inner
    }

    /// Plan a key strategy per index step: a trivially-pure index re-lowers
    /// directly at each use; a possibly-side-effecting index hoists to a single
    /// temp `let`. Returns the hoisting `let` bindings + the per-step
    /// `IndexKey`s, and records the source-ExprId -> temp override so the
    /// parser-shared read-copy inside `value` reuses the temp.
    fn plan_index_keys(
        &mut self,
        step_list: &[AccessStep],
        span: Span,
    ) -> (Vec<CanId>, Vec<IndexKey>) {
        let mut temp_lets: Vec<CanId> = Vec::new();
        let mut index_keys: Vec<IndexKey> = Vec::with_capacity(step_list.len());
        for (i, step) in step_list.iter().enumerate() {
            match step {
                AccessStep::Index(idx_expr) => {
                    let idx_expr = *idx_expr;
                    if self.is_trivial_index(idx_expr) {
                        index_keys.push(IndexKey::Direct(idx_expr));
                        continue;
                    }
                    let idx_ty = self.expr_type(idx_expr);
                    let idx_can = self.lower_expr(idx_expr);
                    let temp_name = self.intern_assign_idx_temp(i);
                    let pattern =
                        self.arena
                            .push_binding_pattern(ori_ir::canon::CanBindingPattern::Name {
                                name: temp_name,
                                mutable: Mutability::Immutable,
                            });
                    let let_node = self.push(
                        CanExpr::Let {
                            pattern,
                            init: idx_can,
                            mutable: Mutability::Immutable,
                        },
                        span,
                        TypeId::UNIT,
                    );
                    temp_lets.push(let_node);
                    let temp_ident = self.push(CanExpr::Ident(temp_name), span, idx_ty);
                    self.index_temp_overrides.insert(idx_expr, temp_ident);
                    index_keys.push(IndexKey::Temp(temp_ident));
                }
                AccessStep::Field(_) => index_keys.push(IndexKey::None),
            }
        }
        (temp_lets, index_keys)
    }

    /// Synthesize the read-chain `root.step0..step(k-1)` as canonical
    /// `Index`/`Field` read nodes. `level_types[j]` types the read after the
    /// first `j` steps; index keys reuse the hoisted temps.
    fn synth_read_chain(&mut self, plan: &ReadChainPlan<'_>, k: usize) -> CanId {
        let mut node = self.push(
            CanExpr::Ident(plan.root_name),
            plan.span,
            plan.level_types[0],
        );
        for j in 0..k {
            node = match plan.steps[j] {
                AccessStep::Index(_) => {
                    let key = self.lower_index_key(plan.index_keys[j], plan.span);
                    let ori_types::AssignStepRoute::Index(dispatch) = plan.step_routes[j] else {
                        unreachable!("validated index step must carry an index dispatch route");
                    };
                    self.push(
                        CanExpr::Index {
                            receiver: node,
                            index: key,
                            dispatch,
                        },
                        plan.span,
                        plan.level_types[j + 1],
                    )
                }
                AccessStep::Field(field) => self.push(
                    CanExpr::Field {
                        receiver: node,
                        field,
                    },
                    plan.span,
                    plan.level_types[j + 1],
                ),
            };
        }
        node
    }

    /// Synthesize `{ ...receiver, field: new_value }` as a fully-resolved
    /// `CanExpr::Struct` (mirrors `desugar_struct_with_spread`): the named
    /// field gets `new_value`; every other field reads `receiver.field`.
    ///
    /// `struct_ty` is the receiver's resolved struct type; its `Named` /
    /// `Applied` name resolves the field roster from the type registry.
    fn synth_struct_update(
        &mut self,
        receiver: CanId,
        struct_ty: TypeId,
        field: Name,
        new_value: CanId,
        span: Span,
    ) -> CanId {
        let idx = ori_types::Idx::from_raw(struct_ty.raw());
        let struct_name = match self.pool.tag(idx) {
            ori_types::Tag::Named => self.pool.named_name(idx),
            ori_types::Tag::Applied => self.pool.applied_name(idx),
            // Unknown receiver shape — emit Error (the type checker should have
            // rejected this).
            _ => return self.push(CanExpr::Error, span, struct_ty),
        };

        let Some(field_defs) = self.resolve_struct_fields(struct_name, struct_ty) else {
            return self.push(CanExpr::Error, span, struct_ty);
        };

        let can_fields: Vec<CanField> = field_defs
            .iter()
            .map(|&(field_name, field_ty)| {
                let value = if field_name == field {
                    new_value
                } else {
                    self.push(
                        CanExpr::Field {
                            receiver,
                            field: field_name,
                        },
                        span,
                        field_ty,
                    )
                };
                CanField {
                    name: field_name,
                    value,
                }
            })
            .collect();

        let fields = self.arena.push_fields(&can_fields);
        self.push(
            CanExpr::Struct {
                name: struct_name,
                fields,
            },
            span,
            struct_ty,
        )
    }

    /// Intern a unique-per-position temp name for a hoisted index expression.
    fn intern_assign_idx_temp(&self, step_index: usize) -> Name {
        self.interner.intern(&format!("$__assign_idx_{step_index}"))
    }

    /// Lower an index step's key per its strategy: re-lower the source index
    /// directly (trivially-pure), or read the hoisted temp (side-effecting).
    /// `IndexKey::None` (a field step) never reaches here.
    fn lower_index_key(&mut self, key: IndexKey, span: Span) -> CanId {
        match key {
            IndexKey::Direct(idx_expr) => self.lower_expr(idx_expr),
            IndexKey::Temp(temp) => temp,
            IndexKey::None => self.push(CanExpr::Error, span, TypeId::ERROR),
        }
    }

    /// A trivially-pure index expression (literal / identifier / const / `self`
    /// / `#`) is re-evaluable for free — no hoist temp needed. Anything else
    /// (`f()`, arithmetic, field/index reads) is possibly side-effecting and is
    /// hoisted to a single temporary for the eval-once guarantee.
    fn is_trivial_index(&self, idx_expr: ExprId) -> bool {
        matches!(
            self.src.expr_kind(idx_expr),
            ExprKind::Int(_)
                | ExprKind::Bool(_)
                | ExprKind::Char(_)
                | ExprKind::String(_)
                | ExprKind::Ident(_)
                | ExprKind::Const(_)
                | ExprKind::SelfRef
                | ExprKind::HashLength
                | ExprKind::Unit
        )
    }

    /// Fallback for an `AssignTarget` with no recorded desugar plan (or a
    /// non-identifier root): lower the chain as the un-desugared
    /// `Index`/`Field` write-target so the evaluator surfaces its clean
    /// "not supported" diagnostic. Valid programs never reach this path once
    /// the type checker has planned the desugar.
    fn lower_assign_chain_fallback(
        &mut self,
        root: ExprId,
        steps: AccessStepRange,
        value: ExprId,
        span: Span,
        outer_ty: TypeId,
    ) -> CanId {
        let node = self.lower_raw_access_chain(root, steps, span, outer_ty);
        let value = self.lower_expr(value);
        self.push(
            CanExpr::Assign {
                target: node,
                value,
            },
            span,
            outer_ty,
        )
    }

    /// Lower `root` plus a raw `AccessStepRange` into a left-associated
    /// `Index`/`Field` read chain, re-lowering each index directly and tagging
    /// every node with `ty`. The single canonical builder for an un-desugared
    /// access chain for fallback write targets and bare `ExprKind::AssignTarget`.
    pub(crate) fn lower_raw_access_chain(
        &mut self,
        root: ExprId,
        steps: AccessStepRange,
        span: Span,
        ty: TypeId,
    ) -> CanId {
        let mut node = self.lower_expr(root);
        let step_list = self.src.get_access_steps(steps).to_vec();
        for step in step_list {
            node = match step {
                AccessStep::Field(field) => self.push(
                    CanExpr::Field {
                        receiver: node,
                        field,
                    },
                    span,
                    ty,
                ),
                AccessStep::Index(index) => {
                    let index = self.lower_expr(index);
                    self.push(
                        CanExpr::Index {
                            receiver: node,
                            index,
                            dispatch: ori_ir::canon::IndexDispatch::Error,
                        },
                        span,
                        ty,
                    )
                }
            };
        }
        node
    }
}
