// Scope Tracker for ARC Memory Management
//
// Tracks scope boundaries and local variable allocations to determine
// where release operations need to be inserted. Locals are released
// in reverse creation order when exiting a scope.

use std::collections::HashMap;

use crate::ir::Type;

use super::super::ids::{LocalId, ScopeId};
use super::super::traits::{ReleasePoint, ReleaseReason};

/// Kind of scope
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeKind {
    /// Function body scope
    Function,

    /// Block scope (run pattern, etc.)
    Block,

    /// Loop body scope (for/loop)
    Loop,

    /// Conditional branch (if/else)
    Conditional,

    /// Match arm
    MatchArm,

    /// Lambda/closure scope
    Lambda,
}

/// Information about a local variable allocation
#[derive(Debug, Clone)]
pub struct LocalAllocation {
    /// The local variable ID
    pub local_id: LocalId,

    /// Type of the value
    pub ty: Type,

    /// Name of the variable (for debugging)
    pub name: String,

    /// Order of allocation within the scope
    pub order: u32,

    /// Whether this local requires destruction
    pub requires_destruction: bool,
}

/// Information about a scope
#[derive(Debug, Clone)]
pub struct ScopeInfo {
    /// Unique ID of this scope
    pub id: ScopeId,

    /// Kind of scope
    pub kind: ScopeKind,

    /// Parent scope (None for root)
    pub parent: Option<ScopeId>,

    /// Local variables allocated in this scope (in creation order)
    pub locals: Vec<LocalAllocation>,

    /// Whether this scope has early exits (break/continue/return)
    pub has_early_exit: bool,
}

impl ScopeInfo {
    fn new(id: ScopeId, kind: ScopeKind, parent: Option<ScopeId>) -> Self {
        ScopeInfo {
            id,
            kind,
            parent,
            locals: Vec::new(),
            has_early_exit: false,
        }
    }
}

/// Tracker for scope boundaries and local allocations
pub struct ScopeTracker {
    /// All scopes (indexed by ScopeId)
    scopes: Vec<ScopeInfo>,

    /// Current scope stack
    scope_stack: Vec<ScopeId>,

    /// Next scope ID to allocate
    next_scope_id: u32,

    /// Map from LocalId to its allocation info
    local_info: HashMap<LocalId, (ScopeId, usize)>, // (scope, index in scope.locals)
}

impl Default for ScopeTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl ScopeTracker {
    /// Create a new scope tracker
    pub fn new() -> Self {
        ScopeTracker {
            scopes: Vec::new(),
            scope_stack: Vec::new(),
            next_scope_id: 0,
            local_info: HashMap::new(),
        }
    }

    /// Enter a new scope
    pub fn enter(&mut self, kind: ScopeKind) -> ScopeId {
        let id = ScopeId::new(self.next_scope_id);
        self.next_scope_id += 1;

        let parent = self.scope_stack.last().copied();
        let scope = ScopeInfo::new(id, kind, parent);

        self.scopes.push(scope);
        self.scope_stack.push(id);

        id
    }

    /// Record a local variable allocation in the current scope
    pub fn record_allocation(
        &mut self,
        local_id: LocalId,
        ty: Type,
        name: String,
        requires_destruction: bool,
    ) {
        let scope_id = *self.scope_stack.last().expect("no scope to allocate in");
        let scope = &mut self.scopes[scope_id.index()];

        let order = scope.locals.len() as u32;
        scope.locals.push(LocalAllocation {
            local_id,
            ty,
            name,
            order,
            requires_destruction,
        });

        self.local_info.insert(local_id, (scope_id, order as usize));
    }

    /// Mark current scope as having an early exit
    pub fn mark_early_exit(&mut self) {
        if let Some(&scope_id) = self.scope_stack.last() {
            self.scopes[scope_id.index()].has_early_exit = true;
        }
    }

    /// Exit the current scope and return release points
    ///
    /// Returns locals in reverse creation order (LIFO)
    pub fn exit(&mut self) -> Vec<ReleasePoint> {
        let scope_id = self.scope_stack.pop().expect("no scope to exit");
        let scope = &self.scopes[scope_id.index()];

        // Generate release points in reverse order
        let releases: Vec<ReleasePoint> = scope
            .locals
            .iter()
            .rev()
            .filter(|local| local.requires_destruction)
            .enumerate()
            .map(|(i, local)| ReleasePoint {
                scope_id,
                local_id: local.local_id,
                ty: local.ty.clone(),
                reason: ReleaseReason::ScopeExit,
                order: i as u32,
            })
            .collect();

        releases
    }

    /// Get releases needed for an early exit (break/continue/return)
    ///
    /// Returns releases for all scopes up to and including the target scope
    pub fn releases_for_early_exit(&self, target_scope: ScopeId) -> Vec<ReleasePoint> {
        let mut releases = Vec::new();
        let mut order = 0;

        // Walk up the scope stack from current to target
        for &scope_id in self.scope_stack.iter().rev() {
            let scope = &self.scopes[scope_id.index()];

            // Add releases for this scope (in reverse order)
            for local in scope.locals.iter().rev() {
                if local.requires_destruction {
                    releases.push(ReleasePoint {
                        scope_id,
                        local_id: local.local_id,
                        ty: local.ty.clone(),
                        reason: ReleaseReason::ControlFlow,
                        order,
                    });
                    order += 1;
                }
            }

            if scope_id == target_scope {
                break;
            }
        }

        releases
    }

    /// Get releases needed for a return statement
    ///
    /// Returns releases for all scopes up to and including the function scope
    pub fn releases_for_return(&self) -> Vec<ReleasePoint> {
        let mut releases = Vec::new();
        let mut order = 0;

        // Walk up to the function scope
        for &scope_id in self.scope_stack.iter().rev() {
            let scope = &self.scopes[scope_id.index()];

            // Add releases for this scope
            for local in scope.locals.iter().rev() {
                if local.requires_destruction {
                    releases.push(ReleasePoint {
                        scope_id,
                        local_id: local.local_id,
                        ty: local.ty.clone(),
                        reason: ReleaseReason::EarlyReturn,
                        order,
                    });
                    order += 1;
                }
            }

            if scope.kind == ScopeKind::Function {
                break;
            }
        }

        releases
    }

    /// Get the current scope ID
    pub fn current_scope(&self) -> Option<ScopeId> {
        self.scope_stack.last().copied()
    }

    /// Get scope info by ID
    pub fn get_scope(&self, id: ScopeId) -> Option<&ScopeInfo> {
        self.scopes.get(id.index())
    }

    /// Get local allocation info
    pub fn get_local_info(&self, local_id: LocalId) -> Option<&LocalAllocation> {
        self.local_info.get(&local_id).and_then(|&(scope_id, idx)| {
            self.scopes
                .get(scope_id.index())
                .and_then(|s| s.locals.get(idx))
        })
    }

    /// Find the enclosing loop scope (for break/continue)
    pub fn find_loop_scope(&self) -> Option<ScopeId> {
        for &scope_id in self.scope_stack.iter().rev() {
            if self.scopes[scope_id.index()].kind == ScopeKind::Loop {
                return Some(scope_id);
            }
        }
        None
    }

    /// Find the enclosing function scope (for return)
    pub fn find_function_scope(&self) -> Option<ScopeId> {
        for &scope_id in self.scope_stack.iter().rev() {
            if self.scopes[scope_id.index()].kind == ScopeKind::Function {
                return Some(scope_id);
            }
        }
        None
    }

    /// Get depth of current scope stack
    pub fn depth(&self) -> usize {
        self.scope_stack.len()
    }

    /// Check if we're inside a loop
    pub fn in_loop(&self) -> bool {
        self.find_loop_scope().is_some()
    }

    /// Reset the tracker for a new function
    pub fn reset(&mut self) {
        self.scopes.clear();
        self.scope_stack.clear();
        self.next_scope_id = 0;
        self.local_info.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_scope() {
        let mut tracker = ScopeTracker::new();

        let scope_id = tracker.enter(ScopeKind::Function);
        assert_eq!(scope_id.index(), 0);
        assert_eq!(tracker.depth(), 1);

        tracker.record_allocation(LocalId::new(0), Type::Int, "x".to_string(), false);
        tracker.record_allocation(LocalId::new(1), Type::Str, "s".to_string(), true);

        let releases = tracker.exit();
        assert_eq!(releases.len(), 1); // Only 's' requires destruction
        assert_eq!(releases[0].local_id, LocalId::new(1));
    }

    #[test]
    fn test_nested_scopes() {
        let mut tracker = ScopeTracker::new();

        let func_scope = tracker.enter(ScopeKind::Function);
        tracker.record_allocation(LocalId::new(0), Type::Str, "a".to_string(), true);

        let block_scope = tracker.enter(ScopeKind::Block);
        tracker.record_allocation(LocalId::new(1), Type::Str, "b".to_string(), true);

        assert_eq!(tracker.depth(), 2);

        // Exit inner scope
        let inner_releases = tracker.exit();
        assert_eq!(inner_releases.len(), 1);
        assert_eq!(inner_releases[0].local_id, LocalId::new(1));

        // Exit outer scope
        let outer_releases = tracker.exit();
        assert_eq!(outer_releases.len(), 1);
        assert_eq!(outer_releases[0].local_id, LocalId::new(0));
    }

    #[test]
    fn test_reverse_order() {
        let mut tracker = ScopeTracker::new();

        tracker.enter(ScopeKind::Function);
        tracker.record_allocation(LocalId::new(0), Type::Str, "a".to_string(), true);
        tracker.record_allocation(LocalId::new(1), Type::Str, "b".to_string(), true);
        tracker.record_allocation(LocalId::new(2), Type::Str, "c".to_string(), true);

        let releases = tracker.exit();

        // Should be in reverse order: c, b, a
        assert_eq!(releases.len(), 3);
        assert_eq!(releases[0].local_id, LocalId::new(2));
        assert_eq!(releases[1].local_id, LocalId::new(1));
        assert_eq!(releases[2].local_id, LocalId::new(0));
    }

    #[test]
    fn test_early_exit() {
        let mut tracker = ScopeTracker::new();

        let func_scope = tracker.enter(ScopeKind::Function);
        tracker.record_allocation(LocalId::new(0), Type::Str, "outer".to_string(), true);

        let loop_scope = tracker.enter(ScopeKind::Loop);
        tracker.record_allocation(LocalId::new(1), Type::Str, "inner".to_string(), true);

        // Simulate break - need to release both inner and outer up to loop
        let releases = tracker.releases_for_early_exit(loop_scope);

        // Should only include inner scope locals
        assert_eq!(releases.len(), 1);
        assert_eq!(releases[0].local_id, LocalId::new(1));
    }

    #[test]
    fn test_return_releases() {
        let mut tracker = ScopeTracker::new();

        tracker.enter(ScopeKind::Function);
        tracker.record_allocation(LocalId::new(0), Type::Str, "a".to_string(), true);

        tracker.enter(ScopeKind::Block);
        tracker.record_allocation(LocalId::new(1), Type::Str, "b".to_string(), true);

        tracker.enter(ScopeKind::Block);
        tracker.record_allocation(LocalId::new(2), Type::Str, "c".to_string(), true);

        let releases = tracker.releases_for_return();

        // Should include all locals from all scopes
        assert_eq!(releases.len(), 3);
        // In reverse order from innermost to outermost
        assert_eq!(releases[0].local_id, LocalId::new(2));
        assert_eq!(releases[1].local_id, LocalId::new(1));
        assert_eq!(releases[2].local_id, LocalId::new(0));
    }

    #[test]
    fn test_find_loop_scope() {
        let mut tracker = ScopeTracker::new();

        tracker.enter(ScopeKind::Function);
        assert!(tracker.find_loop_scope().is_none());

        let loop_scope = tracker.enter(ScopeKind::Loop);
        assert_eq!(tracker.find_loop_scope(), Some(loop_scope));

        tracker.enter(ScopeKind::Block);
        assert_eq!(tracker.find_loop_scope(), Some(loop_scope));
    }

    #[test]
    fn test_reset() {
        let mut tracker = ScopeTracker::new();

        tracker.enter(ScopeKind::Function);
        tracker.record_allocation(LocalId::new(0), Type::Str, "x".to_string(), true);

        tracker.reset();

        assert_eq!(tracker.depth(), 0);
        assert!(tracker.current_scope().is_none());
    }
}
