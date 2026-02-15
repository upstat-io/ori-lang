//! Constant and decision tree pools for the canonical IR.
//!
//! [`ConstantPool`] interns compile-time-folded values (integers, strings, etc.)
//! and [`DecisionTreePool`] stores pre-compiled pattern match decision trees.
//! Both are indexed by lightweight ID newtypes ([`ConstantId`], [`DecisionTreeId`]).

use std::fmt;

use crate::arena::to_u32;
use crate::Name;

use super::expr::ConstValue;
use super::tree::DecisionTree;

/// Index into a [`ConstantPool`]. References a compile-time-folded value.
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
#[repr(transparent)]
pub struct ConstantId(u32);

impl ConstantId {
    #[inline]
    pub const fn new(index: u32) -> Self {
        Self(index)
    }

    #[inline]
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    #[inline]
    pub const fn raw(self) -> u32 {
        self.0
    }
}

impl fmt::Debug for ConstantId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ConstantId({})", self.0)
    }
}

/// Index into a [`DecisionTreePool`]. References a pre-compiled decision tree.
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
#[repr(transparent)]
pub struct DecisionTreeId(u32);

impl DecisionTreeId {
    #[inline]
    pub const fn new(index: u32) -> Self {
        Self(index)
    }

    #[inline]
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    #[inline]
    pub const fn raw(self) -> u32 {
        self.0
    }
}

impl fmt::Debug for DecisionTreeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DecisionTreeId({})", self.0)
    }
}

/// Pool of compile-time constant values, indexed by [`ConstantId`].
///
/// Constants are interned: duplicate values share the same ID.
/// Pre-interns common sentinels (unit, true, false, 0, 1, empty string)
/// for O(1) access.
#[derive(Clone, Debug)]
pub struct ConstantPool {
    values: Vec<ConstValue>,
    /// Content-hash dedup: maps value hash to index for O(1) lookup.
    dedup: rustc_hash::FxHashMap<ConstValue, ConstantId>,
}

impl ConstantPool {
    // Pre-interned sentinel IDs.
    pub const UNIT: ConstantId = ConstantId(0);
    pub const TRUE: ConstantId = ConstantId(1);
    pub const FALSE: ConstantId = ConstantId(2);
    pub const ZERO: ConstantId = ConstantId(3);
    pub const ONE: ConstantId = ConstantId(4);
    pub const EMPTY_STR: ConstantId = ConstantId(5);

    /// Create a new constant pool with pre-interned sentinels.
    pub fn new() -> Self {
        let sentinels = vec![
            ConstValue::Unit,
            ConstValue::Bool(true),
            ConstValue::Bool(false),
            ConstValue::Int(0),
            ConstValue::Int(1),
            ConstValue::Str(Name::EMPTY),
        ];

        let mut dedup = rustc_hash::FxHashMap::default();
        for (i, v) in sentinels.iter().enumerate() {
            dedup.insert(v.clone(), ConstantId::new(to_u32(i, "constant sentinels")));
        }

        Self {
            values: sentinels,
            dedup,
        }
    }

    /// Intern a constant value. Returns the existing ID if already interned.
    pub fn intern(&mut self, value: ConstValue) -> ConstantId {
        if let Some(&id) = self.dedup.get(&value) {
            return id;
        }
        let id = ConstantId::new(to_u32(self.values.len(), "constants"));
        self.dedup.insert(value.clone(), id);
        self.values.push(value);
        id
    }

    /// Get a constant value by ID.
    pub fn get(&self, id: ConstantId) -> &ConstValue {
        &self.values[id.index()]
    }

    /// Number of interned constants.
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Returns `true` if only sentinels are present.
    pub fn is_empty(&self) -> bool {
        self.values.len() <= 6 // sentinels only
    }
}

impl Default for ConstantPool {
    fn default() -> Self {
        Self::new()
    }
}

impl PartialEq for ConstantPool {
    fn eq(&self, other: &Self) -> bool {
        self.values == other.values
    }
}

impl Eq for ConstantPool {}

/// Shared decision tree — `Arc<DecisionTree>` for O(1) cloning.
///
/// Decision trees are immutable after construction and may be cloned
/// by both `ori_eval` (to release a borrow on `self`) and `ori_arc`
/// (same pattern). Arc sharing avoids deep-copying the recursive structure.
#[expect(
    clippy::disallowed_types,
    reason = "Arc enables O(1) clone for immutable decision trees shared across eval/codegen"
)]
pub type SharedDecisionTree = std::sync::Arc<DecisionTree>;

/// Pool of compiled decision trees, indexed by [`DecisionTreeId`].
///
/// Decision trees are produced during pattern compilation (Section 03)
/// and consumed by both `ori_eval` and `ori_arc`. Trees are wrapped in
/// `Arc` so consumers can cheaply clone a reference (O(1)) instead of
/// deep-cloning the recursive tree structure.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DecisionTreePool {
    trees: Vec<SharedDecisionTree>,
}

impl DecisionTreePool {
    /// Create an empty pool.
    pub fn new() -> Self {
        Self { trees: Vec::new() }
    }

    /// Store a decision tree and return its ID.
    pub fn push(&mut self, tree: DecisionTree) -> DecisionTreeId {
        let id = DecisionTreeId::new(to_u32(self.trees.len(), "decision trees"));
        self.trees.push(SharedDecisionTree::new(tree));
        id
    }

    /// Get a decision tree by ID.
    pub fn get(&self, id: DecisionTreeId) -> &DecisionTree {
        &self.trees[id.index()]
    }

    /// Get a shared reference to a decision tree for O(1) cloning.
    ///
    /// Use this instead of `get().clone()` when you need to own a copy
    /// of the tree (e.g., to release a borrow on `self`). The Arc clone
    /// is O(1) vs O(n) for deep-cloning the recursive tree structure.
    pub fn get_shared(&self, id: DecisionTreeId) -> SharedDecisionTree {
        SharedDecisionTree::clone(&self.trees[id.index()])
    }

    /// Number of stored trees.
    pub fn len(&self) -> usize {
        self.trees.len()
    }

    /// Returns `true` if no trees are stored.
    pub fn is_empty(&self) -> bool {
        self.trees.is_empty()
    }
}
