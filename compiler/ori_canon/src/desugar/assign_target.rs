//! Type-directed index/field-assignment desugar.
//!
//! Rewrites `ExprKind::Assign { target: AssignTarget, value }` into the
//! pure-reassignment form so AIMS (which runs after canonicalization) never
//! sees a `CanExpr::Assign { target: Index/Field }`:
//!
//! | Source | Desugared |
//! |--------|-----------|
//! | `xs[i] = v` | `xs = xs.updated(key: i, value: v)` |
//! | `m[k] = v` | `m = m.updated(key: k, value: v)` (`{K: V}` `IndexSet`) |
//! | `s.f = v` | `s = { ...s, f: v }` |
//! | `grid[x][y] = v` | `grid = grid.updated(key: x, value: grid[x].updated(key: y, value: v))` |
//! | `s.items[i] = v` | `s = { ...s, items: s.items.updated(key: i, value: v) }` |
//!
//! Chains wrap right-to-left. The per-level resolved types come from the type
//! checker's [`ori_types::AssignDesugar`] plan (the type-direction decision);
//! synthesis happens here because the source arena is immutable during type
//! checking.
//!
//! Side-effect hoisting: each index expression is bound once to a synthetic
//! `let $__assign_idx_N` temporary and reused everywhere the chain reads it,
//! so `arr[f()] += 1` (whose parser compound-desugar shares the index
//! `ExprId` between the read-copy and the write-copy) runs `f()` exactly once.

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
            // Root is not a plain identifier — the only assignable root shape.
            // Fall back to the un-desugared chain (the evaluator surfaces the
            // clean diagnostic). Valid programs never reach here: the type
            // checker rejects non-identifier / immutable roots before planning.
            return self.lower_assign_chain_fallback(root, steps, value, span, outer_ty);
        };

        let level_types: Vec<TypeId> = match self.typed.resolve_assign_desugar(assign_target_id) {
            Some(plan) => plan
                .level_types
                .iter()
                .map(|idx| TypeId::from_raw(idx.raw()))
                .collect(),
            None => {
                return self.lower_assign_chain_fallback(root, steps, value, span, outer_ty);
            }
        };

        let step_list: Vec<AccessStep> = self.src.get_access_steps(steps).to_vec();

        // `level_types[k]` is the type of reading `root` plus the first `k`
        // steps. A well-formed plan carries `steps + 1` entries.
        if level_types.len() != step_list.len() + 1 {
            return self.lower_assign_chain_fallback(root, steps, value, span, outer_ty);
        }

        // Per index step, decide a key strategy (hoisted temp vs direct
        // re-lower) and record the hoisting `let` bindings + the source-ExprId
        // -> temp override so the parser-shared read-copy inside `value` reuses
        // the temp.
        let (temp_lets, index_keys) = self.plan_index_keys(&step_list, span);

        // Lower the assigned value with the index-temp overrides active so any
        // shared index read inside `value` (the parser's compound-assign
        // read-copy) reuses the hoisted temp instead of re-evaluating it.
        let value_can = self.lower_expr(value);

        // Wrap right-to-left: each step k applies `updated` (index) or struct
        // spread (field) to the read-chain `root.step0..step(k-1)`.
        let mut inner = value_can;
        for k in (0..step_list.len()).rev() {
            let receiver =
                self.synth_read_chain(root_name, &step_list, &index_keys, k, span, &level_types);
            inner = match step_list[k] {
                AccessStep::Index(_) => {
                    let key = self.lower_index_key(index_keys[k], span);
                    let args = self.arena.push_expr_list(&[key, inner]);
                    self.push(
                        CanExpr::MethodCall {
                            receiver,
                            method: self.name_updated,
                            args,
                        },
                        span,
                        level_types[k],
                    )
                }
                AccessStep::Field(field) => {
                    self.synth_struct_update(receiver, level_types[k], field, inner, span)
                }
            };
        }

        // The final reassignment: `root = <wrapped>`. `inner` collapses to
        // `value_can` when there are zero steps (defensive — the parser only
        // builds an AssignTarget when at least one step exists).
        let target = self.push(CanExpr::Ident(root_name), span, level_types[0]);
        let assign = self.push(
            CanExpr::Assign {
                target,
                value: inner,
            },
            span,
            outer_ty,
        );

        // Clear the per-desugar overrides before returning; nested assignments
        // re-populate their own.
        self.index_temp_overrides.clear();

        if temp_lets.is_empty() {
            return assign;
        }

        // Wrap the temp bindings + reassignment in a block so the temps scope
        // to this assignment only.
        let start = self.arena.start_expr_list();
        for let_node in temp_lets {
            self.arena.push_expr_list_item(let_node);
        }
        let stmts = self.arena.finish_expr_list(start);
        self.push(
            CanExpr::Block {
                stmts,
                result: assign,
            },
            span,
            outer_ty,
        )
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
    fn synth_read_chain(
        &mut self,
        root_name: Name,
        steps: &[AccessStep],
        index_keys: &[IndexKey],
        k: usize,
        span: Span,
        level_types: &[TypeId],
    ) -> CanId {
        let mut node = self.push(CanExpr::Ident(root_name), span, level_types[0]);
        for j in 0..k {
            node = match steps[j] {
                AccessStep::Index(_) => {
                    let key = self.lower_index_key(index_keys[j], span);
                    self.push(
                        CanExpr::Index {
                            receiver: node,
                            index: key,
                        },
                        span,
                        level_types[j + 1],
                    )
                }
                AccessStep::Field(field) => self.push(
                    CanExpr::Field {
                        receiver: node,
                        field,
                    },
                    span,
                    level_types[j + 1],
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

        let Some(field_names) = self.resolve_struct_fields(struct_name) else {
            return self.push(CanExpr::Error, span, struct_ty);
        };

        let can_fields: Vec<CanField> = field_names
            .iter()
            .map(|&fname| {
                let value = if fname == field {
                    new_value
                } else {
                    self.push(
                        CanExpr::Field {
                            receiver,
                            field: fname,
                        },
                        span,
                        TypeId::ERROR,
                    )
                };
                CanField { name: fname, value }
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
                    outer_ty,
                ),
                AccessStep::Index(index) => {
                    let index = self.lower_expr(index);
                    self.push(
                        CanExpr::Index {
                            receiver: node,
                            index,
                        },
                        span,
                        outer_ty,
                    )
                }
            };
        }
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
}
