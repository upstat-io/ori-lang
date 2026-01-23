// Retain/Release Insertion for ARC Memory Management
//
// Identifies points in the code where retain (increment refcount) and
// release (decrement refcount) operations need to be inserted.

use crate::ir::{TExpr, TExprKind, TFunction, TStmt, Type};

use super::super::analysis::DefaultTypeClassifier;
use super::super::ids::{LocalId, ScopeId};
use super::super::traits::{
    ElisionOpportunity, RefCountAnalyzer, ReleasePoint, RetainPoint,
    RetainReason, TypeClassifier,
};
use super::scope_tracker::{ScopeKind, ScopeTracker};

/// Default implementation of RefCountAnalyzer
pub struct DefaultRefCountAnalyzer {
    /// Type classifier for determining which types need ARC
    classifier: DefaultTypeClassifier,
}

impl Default for DefaultRefCountAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl DefaultRefCountAnalyzer {
    /// Create a new analyzer
    pub fn new() -> Self {
        DefaultRefCountAnalyzer {
            classifier: DefaultTypeClassifier::new(),
        }
    }

    /// Create an analyzer with a custom classifier
    pub fn with_classifier(classifier: DefaultTypeClassifier) -> Self {
        DefaultRefCountAnalyzer { classifier }
    }

    /// Check if a type needs reference counting
    fn needs_arc(&self, ty: &Type) -> bool {
        self.classifier.requires_destruction(ty)
    }

    /// Analyze an expression for retain points
    fn analyze_expr_retains(
        &self,
        expr: &TExpr,
        scope_id: ScopeId,
        retains: &mut Vec<RetainPoint>,
    ) {
        match &expr.kind {
            // Function calls may need retains for arguments
            TExprKind::Call { args, .. } => {
                for arg in args {
                    if self.needs_arc(&arg.ty) {
                        retains.push(RetainPoint {
                            scope_id,
                            local_id: LocalId::new(0),
                            ty: arg.ty.clone(),
                            reason: RetainReason::FunctionArg,
                        });
                    }
                    self.analyze_expr_retains(arg, scope_id, retains);
                }
            }

            // Method calls similarly need retains
            TExprKind::MethodCall { receiver, args, .. } => {
                self.analyze_expr_retains(receiver, scope_id, retains);
                for arg in args {
                    if self.needs_arc(&arg.ty) {
                        retains.push(RetainPoint {
                            scope_id,
                            local_id: LocalId::new(0),
                            ty: arg.ty.clone(),
                            reason: RetainReason::FunctionArg,
                        });
                    }
                    self.analyze_expr_retains(arg, scope_id, retains);
                }
            }

            // List literals may need retains for elements
            TExprKind::List(elements) => {
                for elem in elements {
                    if self.needs_arc(&elem.ty) {
                        retains.push(RetainPoint {
                            scope_id,
                            local_id: LocalId::new(0),
                            ty: elem.ty.clone(),
                            reason: RetainReason::CollectionInsert,
                        });
                    }
                    self.analyze_expr_retains(elem, scope_id, retains);
                }
            }

            // Map literals
            TExprKind::MapLiteral(entries) => {
                for (key, value) in entries {
                    if self.needs_arc(&key.ty) {
                        retains.push(RetainPoint {
                            scope_id,
                            local_id: LocalId::new(0),
                            ty: key.ty.clone(),
                            reason: RetainReason::CollectionInsert,
                        });
                    }
                    if self.needs_arc(&value.ty) {
                        retains.push(RetainPoint {
                            scope_id,
                            local_id: LocalId::new(0),
                            ty: value.ty.clone(),
                            reason: RetainReason::CollectionInsert,
                        });
                    }
                    self.analyze_expr_retains(key, scope_id, retains);
                    self.analyze_expr_retains(value, scope_id, retains);
                }
            }

            // Lambda captures need retains
            TExprKind::Lambda { captures, body, .. } => {
                // Captures are LocalIds - we'd need to look up their types
                for local_id in captures {
                    retains.push(RetainPoint {
                        scope_id,
                        local_id: LocalId::new(local_id.0),
                        ty: Type::Any, // Would need type lookup
                        reason: RetainReason::ClosureCapture,
                    });
                }
                self.analyze_expr_retains(body, scope_id, retains);
            }

            // Struct construction
            TExprKind::Struct { fields, .. } => {
                for (_, value) in fields {
                    if self.needs_arc(&value.ty) {
                        retains.push(RetainPoint {
                            scope_id,
                            local_id: LocalId::new(0),
                            ty: value.ty.clone(),
                            reason: RetainReason::Binding,
                        });
                    }
                    self.analyze_expr_retains(value, scope_id, retains);
                }
            }

            // Recursively analyze other expressions
            TExprKind::Binary { left, right, .. } => {
                self.analyze_expr_retains(left, scope_id, retains);
                self.analyze_expr_retains(right, scope_id, retains);
            }

            TExprKind::Unary { operand, .. } => {
                self.analyze_expr_retains(operand, scope_id, retains);
            }

            TExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                self.analyze_expr_retains(cond, scope_id, retains);
                self.analyze_expr_retains(then_branch, scope_id, retains);
                self.analyze_expr_retains(else_branch, scope_id, retains);
            }

            TExprKind::Block(stmts, result) => {
                for stmt in stmts {
                    match stmt {
                        TStmt::Expr(e) => self.analyze_expr_retains(e, scope_id, retains),
                        TStmt::Let { value, .. } => {
                            if self.needs_arc(&value.ty) {
                                retains.push(RetainPoint {
                                    scope_id,
                                    local_id: LocalId::new(0),
                                    ty: value.ty.clone(),
                                    reason: RetainReason::Binding,
                                });
                            }
                            self.analyze_expr_retains(value, scope_id, retains);
                        }
                    }
                }
                self.analyze_expr_retains(result, scope_id, retains);
            }

            TExprKind::Match(match_expr) => {
                self.analyze_expr_retains(&match_expr.scrutinee, scope_id, retains);
                for arm in &match_expr.arms {
                    self.analyze_expr_retains(&arm.body, scope_id, retains);
                }
            }

            TExprKind::For { iter, body, .. } => {
                self.analyze_expr_retains(iter, scope_id, retains);
                self.analyze_expr_retains(body, scope_id, retains);
            }

            TExprKind::Index(expr, index) => {
                self.analyze_expr_retains(expr, scope_id, retains);
                self.analyze_expr_retains(index, scope_id, retains);
            }

            TExprKind::Field(expr, _) => {
                self.analyze_expr_retains(expr, scope_id, retains);
            }

            TExprKind::Tuple(elements) => {
                for elem in elements {
                    self.analyze_expr_retains(elem, scope_id, retains);
                }
            }

            TExprKind::Range { start, end } => {
                self.analyze_expr_retains(start, scope_id, retains);
                self.analyze_expr_retains(end, scope_id, retains);
            }

            TExprKind::Ok(inner)
            | TExprKind::Err(inner)
            | TExprKind::Some(inner)
            | TExprKind::Unwrap(inner)
            | TExprKind::LengthOf(inner) => {
                self.analyze_expr_retains(inner, scope_id, retains);
            }

            TExprKind::Coalesce { value, default } => {
                self.analyze_expr_retains(value, scope_id, retains);
                self.analyze_expr_retains(default, scope_id, retains);
            }

            TExprKind::Assign { value, .. } => {
                self.analyze_expr_retains(value, scope_id, retains);
            }

            TExprKind::With { implementation, body, .. } => {
                self.analyze_expr_retains(implementation, scope_id, retains);
                self.analyze_expr_retains(body, scope_id, retains);
            }

            // Terminals - no recursion needed
            TExprKind::Int(_)
            | TExprKind::Float(_)
            | TExprKind::Bool(_)
            | TExprKind::String(_)
            | TExprKind::Local(_)
            | TExprKind::Param(_)
            | TExprKind::Config(_)
            | TExprKind::Nil
            | TExprKind::None_
            | TExprKind::Pattern(_) => {}
        }
    }

    /// Analyze a function for release points using scope tracking
    fn analyze_releases_with_tracker(&self, func: &TFunction) -> Vec<ReleasePoint> {
        let mut tracker = ScopeTracker::new();
        let mut releases = Vec::new();

        // Enter function scope
        tracker.enter(ScopeKind::Function);

        // Record parameters
        for (i, param) in func.params.iter().enumerate() {
            if self.needs_arc(&param.ty) {
                tracker.record_allocation(
                    LocalId::new(i as u32),
                    param.ty.clone(),
                    param.name.clone(),
                    true,
                );
            }
        }

        // Analyze body
        self.analyze_expr_releases(&func.body, &mut tracker, &mut releases);

        // Exit function scope (adds remaining releases)
        releases.extend(tracker.exit());

        releases
    }

    /// Recursively analyze expression for releases
    fn analyze_expr_releases(
        &self,
        expr: &TExpr,
        tracker: &mut ScopeTracker,
        releases: &mut Vec<ReleasePoint>,
    ) {
        match &expr.kind {
            TExprKind::Block(stmts, result) => {
                let _scope_id = tracker.enter(ScopeKind::Block);
                for stmt in stmts {
                    match stmt {
                        TStmt::Expr(e) => self.analyze_expr_releases(e, tracker, releases),
                        TStmt::Let { local, value } => {
                            self.analyze_expr_releases(value, tracker, releases);
                            if self.needs_arc(&value.ty) {
                                tracker.record_allocation(
                                    LocalId::new(local.0),
                                    value.ty.clone(),
                                    format!("local_{}", local.0),
                                    true,
                                );
                            }
                        }
                    }
                }
                self.analyze_expr_releases(result, tracker, releases);
                releases.extend(tracker.exit());
            }

            TExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                self.analyze_expr_releases(cond, tracker, releases);

                tracker.enter(ScopeKind::Conditional);
                self.analyze_expr_releases(then_branch, tracker, releases);
                releases.extend(tracker.exit());

                tracker.enter(ScopeKind::Conditional);
                self.analyze_expr_releases(else_branch, tracker, releases);
                releases.extend(tracker.exit());
            }

            TExprKind::For {
                binding,
                iter,
                body,
            } => {
                self.analyze_expr_releases(iter, tracker, releases);

                tracker.enter(ScopeKind::Loop);
                // Loop binding
                if self.needs_arc(&body.ty) {
                    tracker.record_allocation(
                        LocalId::new(binding.0),
                        body.ty.clone(),
                        format!("for_binding_{}", binding.0),
                        true,
                    );
                }
                self.analyze_expr_releases(body, tracker, releases);
                releases.extend(tracker.exit());
            }

            TExprKind::Match(match_expr) => {
                self.analyze_expr_releases(&match_expr.scrutinee, tracker, releases);

                for arm in &match_expr.arms {
                    tracker.enter(ScopeKind::MatchArm);
                    self.analyze_expr_releases(&arm.body, tracker, releases);
                    releases.extend(tracker.exit());
                }
            }

            TExprKind::Lambda { body, .. } => {
                tracker.enter(ScopeKind::Lambda);
                self.analyze_expr_releases(body, tracker, releases);
                releases.extend(tracker.exit());
            }

            // Recurse into subexpressions
            TExprKind::Binary { left, right, .. } => {
                self.analyze_expr_releases(left, tracker, releases);
                self.analyze_expr_releases(right, tracker, releases);
            }

            TExprKind::Unary { operand, .. } => {
                self.analyze_expr_releases(operand, tracker, releases);
            }

            TExprKind::Call { args, .. } => {
                for arg in args {
                    self.analyze_expr_releases(arg, tracker, releases);
                }
            }

            TExprKind::MethodCall { receiver, args, .. } => {
                self.analyze_expr_releases(receiver, tracker, releases);
                for arg in args {
                    self.analyze_expr_releases(arg, tracker, releases);
                }
            }

            TExprKind::Index(expr, index) => {
                self.analyze_expr_releases(expr, tracker, releases);
                self.analyze_expr_releases(index, tracker, releases);
            }

            TExprKind::Field(expr, _) => {
                self.analyze_expr_releases(expr, tracker, releases);
            }

            TExprKind::List(elements) => {
                for elem in elements {
                    self.analyze_expr_releases(elem, tracker, releases);
                }
            }

            TExprKind::MapLiteral(entries) => {
                for (k, v) in entries {
                    self.analyze_expr_releases(k, tracker, releases);
                    self.analyze_expr_releases(v, tracker, releases);
                }
            }

            TExprKind::Tuple(elements) => {
                for elem in elements {
                    self.analyze_expr_releases(elem, tracker, releases);
                }
            }

            TExprKind::Struct { fields, .. } => {
                for (_, value) in fields {
                    self.analyze_expr_releases(value, tracker, releases);
                }
            }

            TExprKind::Range { start, end } => {
                self.analyze_expr_releases(start, tracker, releases);
                self.analyze_expr_releases(end, tracker, releases);
            }

            TExprKind::Ok(inner)
            | TExprKind::Err(inner)
            | TExprKind::Some(inner)
            | TExprKind::Unwrap(inner)
            | TExprKind::LengthOf(inner) => {
                self.analyze_expr_releases(inner, tracker, releases);
            }

            TExprKind::Coalesce { value, default } => {
                self.analyze_expr_releases(value, tracker, releases);
                self.analyze_expr_releases(default, tracker, releases);
            }

            TExprKind::Assign { value, .. } => {
                self.analyze_expr_releases(value, tracker, releases);
            }

            TExprKind::With { implementation, body, .. } => {
                self.analyze_expr_releases(implementation, tracker, releases);
                self.analyze_expr_releases(body, tracker, releases);
            }

            // Terminals
            _ => {}
        }
    }
}

impl RefCountAnalyzer for DefaultRefCountAnalyzer {
    fn retains_needed(&self, func: &TFunction) -> Vec<RetainPoint> {
        let mut retains = Vec::new();
        let scope_id = ScopeId::new(0); // Function scope
        self.analyze_expr_retains(&func.body, scope_id, &mut retains);
        retains
    }

    fn releases_needed(&self, func: &TFunction) -> Vec<ReleasePoint> {
        self.analyze_releases_with_tracker(func)
    }

    fn elision_opportunities(&self, _func: &TFunction) -> Vec<ElisionOpportunity> {
        // This will be implemented in the elision module
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analyzer_creation() {
        let analyzer = DefaultRefCountAnalyzer::new();
        // Just verify it can be created
        assert!(true);
    }

    #[test]
    fn test_needs_arc() {
        let analyzer = DefaultRefCountAnalyzer::new();

        assert!(!analyzer.needs_arc(&Type::Int));
        assert!(!analyzer.needs_arc(&Type::Bool));
        assert!(analyzer.needs_arc(&Type::Str));
        assert!(analyzer.needs_arc(&Type::List(Box::new(Type::Int))));
    }
}
