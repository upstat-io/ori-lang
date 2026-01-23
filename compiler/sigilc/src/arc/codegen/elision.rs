// Reference Count Elision for ARC Memory Management
//
// Analyzes code to find opportunities where reference counting can be
// elided (skipped) without affecting correctness. This is a key optimization
// that can significantly reduce runtime overhead.
//
// Elision opportunities include:
// - Unique ownership: Only one reference exists, so no counting needed
// - Immediate consumption: Value is created and consumed in same expression
// - Move semantics: Value is moved, not copied, so transfer is free
// - Copy-on-write: Unique references can be mutated in place

use std::collections::{HashMap, HashSet};

use crate::ir::{TExpr, TExprKind, TFunction, TStmt, Type};

use super::super::analysis::DefaultTypeClassifier;
use super::super::ids::LocalId;
use super::super::traits::{ElisionOpportunity, ElisionReason, TypeClassifier};

/// Liveness state of a local variable
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LivenessState {
    /// Variable has not been defined yet
    Undefined,

    /// Variable is live (may be used)
    Live,

    /// Variable is dead (will not be used again)
    Dead,

    /// Variable may or may not be live (unknown control flow)
    Unknown,
}

/// Use information for a local variable
#[derive(Debug, Clone)]
pub struct UseInfo {
    /// Number of times the variable is used
    pub use_count: u32,

    /// Whether the variable is ever borrowed (reference taken)
    pub is_borrowed: bool,

    /// Whether the variable is ever mutated after initialization
    pub is_mutated: bool,

    /// Whether the variable escapes the current scope
    pub escapes: bool,

    /// Whether the variable is captured by a closure
    pub is_captured: bool,
}

impl Default for UseInfo {
    fn default() -> Self {
        UseInfo {
            use_count: 0,
            is_borrowed: false,
            is_mutated: false,
            escapes: false,
            is_captured: false,
        }
    }
}

/// Elision analyzer using liveness and use analysis
pub struct ElisionAnalyzer {
    /// Type classifier
    classifier: DefaultTypeClassifier,

    /// Use information for each local
    use_info: HashMap<LocalId, UseInfo>,

    /// Set of locals that are known to be unique (single owner)
    unique_locals: HashSet<LocalId>,
}

impl Default for ElisionAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl ElisionAnalyzer {
    /// Create a new elision analyzer
    pub fn new() -> Self {
        ElisionAnalyzer {
            classifier: DefaultTypeClassifier::new(),
            use_info: HashMap::new(),
            unique_locals: HashSet::new(),
        }
    }

    /// Analyze a function and find elision opportunities
    pub fn analyze(&mut self, func: &TFunction) -> Vec<ElisionOpportunity> {
        // Clear previous analysis
        self.use_info.clear();
        self.unique_locals.clear();

        // First pass: collect use information
        self.collect_use_info(&func.body);

        // Second pass: identify elision opportunities
        let mut opportunities = Vec::new();

        for (local_id, info) in &self.use_info {
            // Check for unique ownership
            if info.use_count == 1 && !info.is_borrowed && !info.escapes && !info.is_captured {
                // This variable is used exactly once and never shared
                if let Some(ty) = self.get_local_type(*local_id, func) {
                    if self.classifier.requires_destruction(&ty) {
                        opportunities.push(ElisionOpportunity {
                            local_id: *local_id,
                            ty,
                            reason: ElisionReason::UniqueOwnership,
                            benefit: 2, // Save one retain and one release
                        });
                        self.unique_locals.insert(*local_id);
                    }
                }
            }

            // Check for move semantics
            if info.use_count == 1 && !info.is_mutated && !info.is_borrowed {
                if let Some(ty) = self.get_local_type(*local_id, func) {
                    if self.classifier.requires_destruction(&ty) && !self.unique_locals.contains(local_id) {
                        opportunities.push(ElisionOpportunity {
                            local_id: *local_id,
                            ty,
                            reason: ElisionReason::Move,
                            benefit: 1, // Save one retain
                        });
                    }
                }
            }
        }

        // Third pass: identify immediate consumption patterns
        self.find_immediate_consumption(&func.body, &mut opportunities);

        opportunities
    }

    /// Collect use information for all locals
    fn collect_use_info(&mut self, expr: &TExpr) {
        match &expr.kind {
            TExprKind::Local(id) => {
                let local_id = LocalId::new(id.0);
                let info = self.use_info.entry(local_id).or_default();
                info.use_count += 1;
            }

            TExprKind::Lambda { captures, body, .. } => {
                // Mark captured variables
                for local_id in captures {
                    let id = LocalId::new(local_id.0);
                    let info = self.use_info.entry(id).or_default();
                    info.is_captured = true;
                    info.use_count += 1;
                }
                self.collect_use_info(body);
            }

            TExprKind::Call { args, .. } => {
                for arg in args {
                    self.collect_use_info(arg);
                }
            }

            TExprKind::MethodCall { receiver, args, .. } => {
                self.collect_use_info(receiver);
                for arg in args {
                    self.collect_use_info(arg);
                }
            }

            TExprKind::Binary { left, right, .. } => {
                self.collect_use_info(left);
                self.collect_use_info(right);
            }

            TExprKind::Unary { operand, .. } => {
                self.collect_use_info(operand);
            }

            TExprKind::If { cond, then_branch, else_branch } => {
                self.collect_use_info(cond);
                self.collect_use_info(then_branch);
                self.collect_use_info(else_branch);
            }

            TExprKind::Block(stmts, result) => {
                for stmt in stmts {
                    match stmt {
                        TStmt::Expr(e) => self.collect_use_info(e),
                        TStmt::Let { value, .. } => self.collect_use_info(value),
                    }
                }
                self.collect_use_info(result);
            }

            TExprKind::Match(match_expr) => {
                self.collect_use_info(&match_expr.scrutinee);
                for arm in &match_expr.arms {
                    self.collect_use_info(&arm.body);
                }
            }

            TExprKind::For { iter, body, .. } => {
                self.collect_use_info(iter);
                self.collect_use_info(body);
            }

            TExprKind::Index(expr, index) => {
                self.collect_use_info(expr);
                self.collect_use_info(index);
            }

            TExprKind::Field(expr, _) => {
                self.collect_use_info(expr);
            }

            TExprKind::List(elements) => {
                for elem in elements {
                    self.collect_use_info(elem);
                }
            }

            TExprKind::MapLiteral(entries) => {
                for (k, v) in entries {
                    self.collect_use_info(k);
                    self.collect_use_info(v);
                }
            }

            TExprKind::Tuple(elements) => {
                for elem in elements {
                    self.collect_use_info(elem);
                }
            }

            TExprKind::Struct { fields, .. } => {
                for (_, value) in fields {
                    self.collect_use_info(value);
                }
            }

            TExprKind::Range { start, end } => {
                self.collect_use_info(start);
                self.collect_use_info(end);
            }

            TExprKind::Ok(inner)
            | TExprKind::Err(inner)
            | TExprKind::Some(inner)
            | TExprKind::Unwrap(inner)
            | TExprKind::LengthOf(inner) => {
                self.collect_use_info(inner);
            }

            TExprKind::Coalesce { value, default } => {
                self.collect_use_info(value);
                self.collect_use_info(default);
            }

            TExprKind::Assign { value, .. } => {
                self.collect_use_info(value);
            }

            TExprKind::With { implementation, body, .. } => {
                self.collect_use_info(implementation);
                self.collect_use_info(body);
            }

            // Terminals
            _ => {}
        }
    }

    /// Mark locals that escape (returned, stored in collections, etc.)
    #[allow(dead_code)]
    fn mark_escaping(&mut self, expr: &TExpr) {
        match &expr.kind {
            TExprKind::Local(id) => {
                let local_id = LocalId::new(id.0);
                if let Some(info) = self.use_info.get_mut(&local_id) {
                    info.escapes = true;
                }
            }
            TExprKind::Field(expr, _) => {
                self.mark_escaping(expr);
            }
            _ => {}
        }
    }

    /// Find immediate consumption patterns (value created and immediately consumed)
    fn find_immediate_consumption(&self, expr: &TExpr, opportunities: &mut Vec<ElisionOpportunity>) {
        match &expr.kind {
            // Recurse
            TExprKind::Block(stmts, result) => {
                for stmt in stmts {
                    match stmt {
                        TStmt::Expr(e) => self.find_immediate_consumption(e, opportunities),
                        TStmt::Let { value, .. } => {
                            // Check if the value is immediately consumed
                            if let TExprKind::Call { .. } = &value.kind {
                                // Function calls that return reference types
                                if self.classifier.requires_destruction(&value.ty) {
                                    // Could be an immediate consumption if the let binding
                                    // is only used once in the next expression
                                    // This requires more sophisticated analysis
                                }
                            }
                            self.find_immediate_consumption(value, opportunities);
                        }
                    }
                }
                self.find_immediate_consumption(result, opportunities);
            }

            TExprKind::If { cond, then_branch, else_branch } => {
                self.find_immediate_consumption(cond, opportunities);
                self.find_immediate_consumption(then_branch, opportunities);
                self.find_immediate_consumption(else_branch, opportunities);
            }

            TExprKind::Match(match_expr) => {
                self.find_immediate_consumption(&match_expr.scrutinee, opportunities);
                for arm in &match_expr.arms {
                    self.find_immediate_consumption(&arm.body, opportunities);
                }
            }

            TExprKind::For { iter, body, .. } => {
                self.find_immediate_consumption(iter, opportunities);
                self.find_immediate_consumption(body, opportunities);
            }

            _ => {}
        }
    }

    /// Get the type of a local variable (helper)
    fn get_local_type(&self, local_id: LocalId, func: &TFunction) -> Option<Type> {
        func.locals
            .get(crate::ir::LocalId(local_id.0))
            .map(|info| info.ty.clone())
    }

    /// Check if a local has unique ownership
    pub fn is_unique(&self, local_id: LocalId) -> bool {
        self.unique_locals.contains(&local_id)
    }

    /// Get use info for a local
    pub fn get_use_info(&self, local_id: LocalId) -> Option<&UseInfo> {
        self.use_info.get(&local_id)
    }
}

/// Determines if Copy-on-Write optimization is applicable
pub fn can_apply_cow(_ty: &Type, use_info: &UseInfo) -> bool {
    // COW is applicable when:
    // 1. The type is a reference type (string, list, etc.)
    // 2. The reference count is 1 (unique)
    // 3. A mutation is requested

    // At compile time, we can only statically determine COW applicability
    // in limited cases. The runtime will need to check refcount == 1.

    // We can identify potential COW sites where mutation happens
    use_info.is_mutated && !use_info.is_borrowed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_use_info_default() {
        let info = UseInfo::default();
        assert_eq!(info.use_count, 0);
        assert!(!info.is_borrowed);
        assert!(!info.is_mutated);
        assert!(!info.escapes);
        assert!(!info.is_captured);
    }

    #[test]
    fn test_elision_analyzer_creation() {
        let analyzer = ElisionAnalyzer::new();
        assert!(analyzer.use_info.is_empty());
        assert!(analyzer.unique_locals.is_empty());
    }

    #[test]
    fn test_cow_applicability() {
        let mut info = UseInfo::default();
        info.is_mutated = true;

        assert!(can_apply_cow(&Type::Str, &info));

        info.is_borrowed = true;
        assert!(!can_apply_cow(&Type::Str, &info));
    }
}
