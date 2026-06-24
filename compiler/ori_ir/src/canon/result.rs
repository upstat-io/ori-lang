//! Canonicalization result types.
//!
//! [`CanonResult`] bundles the [`CanArena`] with constant/decision-tree pools
//! and root expressions, forming the complete output of the canonicalization
//! pass. [`CanonRoot`] / [`MethodRoot`] carry the per-function and per-method
//! canonical bodies; [`MonoInstanceId`] is the abstract monomorphization
//! dispatch handle; [`SharedCanonResult`] is the thread-safe shared wrapper.

use crate::Name;

use super::arena::CanArena;
use super::expr::PatternProblem;
use super::ids::CanId;
use super::pools::{ConstantPool, DecisionTreePool};

/// A canonicalized function root — body + canonical default expressions.
///
/// Carries canonical default expressions so the evaluator resolves default
/// parameter values via `eval_can(CanId)` rather than `eval(ExprId)`.
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

/// A canonicalized method root — `(type_name, method_name, body)` in canonical IR.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MethodRoot {
    /// Type that owns the method (e.g., `Point`, `list`).
    pub type_name: Name,
    /// Method name.
    pub method_name: Name,
    /// Canonical body expression.
    pub body: CanId,
}

/// Typed index into the typeck-side `Vec<MonoInstance>` — the abstract dispatch
/// key on the canonical-IR side ([`CanonResult::mono_dispatch_map_can`]) and the
/// ARC + LLVM + eval dispatch paths. The mangled LLVM symbol name is computed in
/// `ori_llvm` via `mangle_mono_name`; typeck and canon produce only this index.
///
/// Lives in `ori_ir` (leaf crate; re-exported by `ori_types`) so every consuming
/// crate shares the handle without a cross-crate cycle. Equality / hashing match
/// index identity; construct via `MonoInstanceId(idx)`, read the index via `id.0`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "cache", derive(serde::Serialize, serde::Deserialize))]
pub struct MonoInstanceId(pub u32);

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
    /// Map from canonical `CanId` of a generic call site to its resolved
    /// monomorphic instance index.
    ///
    /// Populated during canon lowering: when `lower_expr` constructs a
    /// `CanExpr::Call` or `CanExpr::MethodCall` for an AST `ExprId` whose
    /// typeck-side `TypedModule.mono_dispatch_map` carries an entry, the
    /// lowerer translates that entry's `(ExprId, MonoInstanceId)` pair to
    /// `(CanId, MonoInstanceId)` and appends here.
    ///
    /// The ARC carriers (`ArcInstr::Apply` / `ArcTerminator::Invoke`)
    /// read this side-table to transfer the abstract index onto the ARC-level
    /// call. LLVM + eval dispatch then look up
    /// `TypedModule.mono_instances[id.0]` and call `mangle_mono_name` locally
    /// inside `ori_llvm` — preserving phase purity (canon
    /// produces canonical IR + side-tables; no LLVM strings).
    ///
    /// Stored as `Vec<(CanId, MonoInstanceId)>` (NOT `FxHashMap`) for Salsa
    /// compatibility — `CanonResult` derives
    /// `Eq + PartialEq` and `FxHashMap` cannot satisfy them. Mirrors the
    /// shape of `TypedModule.mono_dispatch_map: Vec<(ExprId, MonoInstanceId)>`.
    /// Sorted by `CanId.raw()` for binary-search lookup; deduplication is
    /// not required because each `CanId` is allocated exactly once.
    ///
    /// Empty for non-generic call sites.
    pub mono_dispatch_map_can: Vec<(CanId, MonoInstanceId)>,
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
            mono_dispatch_map_can: Vec::new(),
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
