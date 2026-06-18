//! Canonicalization result types ([`CanonResult`], [`CanonRoot`], [`MethodRoot`], [`SharedCanonResult`]).

use crate::Name;

use super::arena::CanArena;
use super::expr::PatternProblem;
use super::ids::CanId;
use super::pools::{ConstantPool, DecisionTreePool};

/// A canonicalized function root — body + defaults in canonical IR.
///
/// Replaces the previous `(Name, CanId)` tuple in `CanonResult.roots`,
/// adding canonical default expressions so that the evaluator can use
/// `eval_can(CanId)` instead of `eval(ExprId)` for default parameter values.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanonRoot {
    /// Function or test name.
    pub name: Name,
    /// Canonical body expression.
    pub body: CanId,
    /// Canonical default expressions, parallel to the function's parameter list.
    /// `defaults[i]` is `Some(can_id)` if parameter `i` has a default value,
    /// `None` if the parameter is required.
    pub defaults: Vec<Option<CanId>>,
    /// For multi-clause functions: the canonical parameter names from the
    /// `FunctionSig`. The evaluator must use these names (not the first clause's
    /// parser names) because the canonical scrutinee Idents use them.
    /// Empty for single-clause functions and tests.
    pub param_names: Vec<Name>,
}

/// A canonicalized method root — body in canonical IR.
///
/// Replaces the previous `(Name, Name, CanId)` tuple in `CanonResult.method_roots`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MethodRoot {
    /// Type that owns the method (e.g., `Point`, `list`).
    pub type_name: Name,
    /// Method name.
    pub method_name: Name,
    /// Canonical body expression.
    pub body: CanId,
}

/// Output of the canonicalization pass.
///
/// Contains everything needed by both backends: the canonical expression
/// arena, constant pool, decision trees, and the root expression.
///
/// # Salsa Compatibility
///
/// Implements Clone, Debug for Salsa query return types.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanonResult {
    /// The canonical expression arena.
    pub arena: CanArena,
    /// Pool of compile-time constant values.
    pub constants: ConstantPool,
    /// Pool of compiled decision trees.
    pub decision_trees: DecisionTreePool,
    /// The root expression (entry point for single-expression lowering).
    pub root: CanId,
    /// Named roots for module-level lowering (one per function/test).
    pub roots: Vec<CanonRoot>,
    /// Method roots for `impl`/`extend`/`def_impl` blocks.
    pub method_roots: Vec<MethodRoot>,
    /// Pattern problems detected during exhaustiveness checking.
    pub problems: Vec<PatternProblem>,
}

impl CanonResult {
    /// Create an empty result (for error recovery).
    pub fn empty() -> Self {
        Self {
            arena: CanArena::new(),
            constants: ConstantPool::new(),
            decision_trees: DecisionTreePool::new(),
            root: CanId::INVALID,
            roots: Vec::new(),
            method_roots: Vec::new(),
            problems: Vec::new(),
        }
    }

    /// Look up a named root by function name.
    pub fn root_for(&self, name: Name) -> Option<CanId> {
        self.roots.iter().find(|r| r.name == name).map(|r| r.body)
    }

    /// Look up a canon root by function name (includes defaults).
    pub fn canon_root_for(&self, name: Name) -> Option<&CanonRoot> {
        self.roots.iter().find(|r| r.name == name)
    }

    /// Look up a method root by type name and method name.
    pub fn method_root_for(&self, type_name: Name, method_name: Name) -> Option<CanId> {
        self.method_roots
            .iter()
            .find(|r| r.type_name == type_name && r.method_name == method_name)
            .map(|r| r.body)
    }

    /// Look up the Nth method root for a `(type_name, method_name)` pair.
    ///
    /// When multiple impls define the same method on the same type
    /// (e.g., `impl Index<int, V>` and `impl Index<str, V>`), each produces
    /// a separate `MethodRoot` entry. This method selects the Nth one,
    /// matching the sequential order in which impls appear in the module.
    pub fn method_root_for_nth(
        &self,
        type_name: Name,
        method_name: Name,
        n: usize,
    ) -> Option<CanId> {
        self.method_roots
            .iter()
            .filter(|r| r.type_name == type_name && r.method_name == method_name)
            .nth(n)
            .map(|r| r.body)
    }
}

/// Thread-safe shared reference to a `CanonResult`.
///
/// Analogous to `SharedArena` but for canonical IR. Functions carry this
/// to resolve `CanId` values in their body during evaluation.
#[derive(Clone, Debug)]
#[expect(
    clippy::disallowed_types,
    reason = "Arc is the implementation of SharedCanonResult"
)]
pub struct SharedCanonResult(std::sync::Arc<CanonResult>);

#[expect(
    clippy::disallowed_types,
    reason = "Arc is the implementation of SharedCanonResult"
)]
impl SharedCanonResult {
    /// Create a new shared canon result.
    pub fn new(result: CanonResult) -> Self {
        Self(std::sync::Arc::new(result))
    }
}

impl std::ops::Deref for SharedCanonResult {
    type Target = CanonResult;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
