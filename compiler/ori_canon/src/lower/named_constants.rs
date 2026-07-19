//! Dependency-ordered evaluation of module-level `$name` declarations.

use rustc_hash::FxHashMap;

use ori_ir::canon::{ConstEvalProblem, ConstEvalProblemKind, NamedConstValue};
use ori_ir::visitor::{walk_expr, Visitor};
use ori_ir::{Expr, ExprArena, ExprKind, Module, Name};

use super::Lowerer;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum VisitState {
    Pending,
    Visiting,
    Done,
    Failed,
}

/// Seed selected imports, evaluate local declarations, and return the local
/// module's consumer-independent value artifact.
pub(super) fn lower_named_constants(
    lowerer: &mut Lowerer<'_>,
    module: &Module,
    imported: &[NamedConstValue],
) -> Vec<NamedConstValue> {
    lowerer.named_constants.extend(
        imported
            .iter()
            .map(|constant| (constant.name, constant.value.clone())),
    );

    let local_indices: FxHashMap<Name, usize> = module
        .consts
        .iter()
        .enumerate()
        .map(|(index, constant)| (constant.name, index))
        .collect();

    // A local declaration shadows a same-named import even when its own
    // initializer later fails. Removing all local names before evaluation
    // prevents a self-reference from accidentally reading the imported value.
    for name in local_indices.keys() {
        lowerer.named_constants.remove(name);
    }

    let dependencies: Vec<Vec<usize>> = module
        .consts
        .iter()
        .map(|constant| {
            let mut collector = ConstReferenceCollector::default();
            collector.visit_expr_id(constant.value, lowerer.src);
            collector
                .references
                .into_iter()
                .filter_map(|name| local_indices.get(&name).copied())
                .collect()
        })
        .collect();
    let mut states = vec![VisitState::Pending; module.consts.len()];

    for index in 0..module.consts.len() {
        evaluate_local(index, lowerer, module, &dependencies, &mut states);
    }

    module
        .consts
        .iter()
        .filter_map(|constant| {
            lowerer
                .named_constants
                .get(&constant.name)
                .cloned()
                .map(|value| NamedConstValue {
                    name: constant.name,
                    value,
                })
        })
        .collect()
}

fn evaluate_local(
    index: usize,
    lowerer: &mut Lowerer<'_>,
    module: &Module,
    dependencies: &[Vec<usize>],
    states: &mut [VisitState],
) -> bool {
    match states[index] {
        VisitState::Done => return true,
        VisitState::Failed => return false,
        VisitState::Visiting => {
            let constant = &module.consts[index];
            if !lowerer.const_problems.iter().any(|problem| {
                problem.name == constant.name
                    && matches!(
                        problem.kind,
                        ConstEvalProblemKind::CircularDependency { .. }
                    )
            }) {
                lowerer.const_problems.push(ConstEvalProblem {
                    name: constant.name,
                    span: constant.span,
                    kind: ConstEvalProblemKind::CircularDependency {
                        dependency: constant.name,
                    },
                });
            }
            return false;
        }
        VisitState::Pending => {}
    }

    states[index] = VisitState::Visiting;
    for &dependency in &dependencies[index] {
        if !evaluate_local(dependency, lowerer, module, dependencies, states) {
            let constant = &module.consts[index];
            if !lowerer
                .const_problems
                .iter()
                .any(|problem| problem.name == constant.name)
            {
                lowerer.const_problems.push(ConstEvalProblem {
                    name: constant.name,
                    span: constant.span,
                    kind: ConstEvalProblemKind::UnresolvedReference {
                        reference: module.consts[dependency].name,
                    },
                });
            }
            states[index] = VisitState::Failed;
            return false;
        }
    }

    let constant = &module.consts[index];
    let root = lowerer.lower_expr(constant.value);
    match crate::const_fold::evaluate_constant(&lowerer.arena, &lowerer.constants, root) {
        Ok(value) => {
            lowerer.named_constants.insert(constant.name, value);
            states[index] = VisitState::Done;
            true
        }
        Err(kind) => {
            lowerer.const_problems.push(ConstEvalProblem {
                name: constant.name,
                span: constant.span,
                kind,
            });
            states[index] = VisitState::Failed;
            false
        }
    }
}

#[derive(Default)]
struct ConstReferenceCollector {
    references: Vec<Name>,
}

impl<'ast> Visitor<'ast> for ConstReferenceCollector {
    fn visit_expr(&mut self, expr: &Expr, arena: &'ast ExprArena) {
        if let ExprKind::Const(name) = expr.kind {
            self.references.push(name);
        }
        walk_expr(self, expr, arena);
    }
}
