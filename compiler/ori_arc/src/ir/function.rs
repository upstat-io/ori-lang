//! [`ArcFunction`] methods — variable management, block allocation, and
//! type/representation lookups.

use ori_ir::Name;
use ori_types::Idx;

use super::{ArcBlock, ArcBlockId, ArcFunction, ArcTerminator, ArcVarId, ValueRepr};

/// Minimal empty function for test construction.
///
/// Tests use struct update syntax to override only the fields they care about:
/// ```ignore
/// ArcFunction { name: ..., blocks: ..., ..Default::default() }
/// ```
///
/// The default has a single empty block with an `Unreachable` terminator,
/// which is the minimal valid structure for most analysis passes.
impl Default for ArcFunction {
    fn default() -> Self {
        ArcFunction {
            name: Name::from_raw(0),
            params: Vec::new(),
            return_type: Idx::from_raw(0),
            blocks: vec![ArcBlock {
                id: ArcBlockId::new(0),
                params: Vec::new(),
                body: Vec::new(),
                terminator: ArcTerminator::Unreachable,
            }],
            entry: ArcBlockId::new(0),
            var_types: Vec::new(),
            var_reprs: Vec::new(),
            spans: vec![Vec::new()],
            is_fbip: false,
            num_captures: 0,
            cow_annotations: crate::uniqueness::CowAnnotations::default(),
            drop_hints: crate::uniqueness::DropHints::default(),
            tail_calls: Vec::new(),
        }
    }
}

impl super::ArcFunction {
    /// Look up the type of a variable.
    ///
    /// # Panics
    ///
    /// Debug-panics if `var` is out of bounds.
    #[inline]
    pub fn var_type(&self, var: ArcVarId) -> Idx {
        debug_assert!(
            var.index() < self.var_types.len(),
            "ArcVarId {} out of bounds (have {} vars)",
            var.raw(),
            self.var_types.len(),
        );
        self.var_types[var.index()]
    }

    /// Look up the machine representation of a variable.
    ///
    /// Only valid after [`compute_var_reprs`](super::compute_var_reprs) has
    /// been called (i.e., during or after the ARC pipeline). Returns `None`
    /// if `var_reprs` is empty (pre-pipeline).
    #[inline]
    pub fn var_repr(&self, var: ArcVarId) -> Option<ValueRepr> {
        if self.var_reprs.is_empty() {
            return None;
        }
        debug_assert!(
            var.index() < self.var_reprs.len(),
            "ArcVarId {} out of bounds for var_reprs (have {} entries)",
            var.raw(),
            self.var_reprs.len(),
        );
        Some(self.var_reprs[var.index()])
    }

    /// Allocate a fresh variable with the given type.
    ///
    /// Returns a new [`ArcVarId`] that does not collide with any existing
    /// variable in this function. The variable's type is recorded in
    /// [`var_types`](Self::var_types).
    ///
    /// Used by ARC passes that introduce synthetic variables (e.g., the
    /// `IsShared` result in constructor reuse expansion, reuse tokens in
    /// reset/reuse detection).
    pub fn fresh_var(&mut self, ty: Idx) -> ArcVarId {
        let id = u32::try_from(self.var_types.len())
            .unwrap_or_else(|_| panic!("variable count exceeds u32::MAX"));
        self.var_types.push(ty);
        // Keep var_reprs in sync if it has been populated.
        // Uses Scalar as a placeholder — callers that know the correct repr
        // should use `fresh_var_repr` instead.
        if !self.var_reprs.is_empty() {
            self.var_reprs.push(ValueRepr::Scalar);
        }
        ArcVarId::new(id)
    }

    /// Allocate a fresh variable with an explicit [`ValueRepr`].
    ///
    /// Like [`fresh_var`](Self::fresh_var), but stores the provided `repr`
    /// instead of a `Scalar` placeholder. Used by ARC passes that create
    /// variables of known representation (e.g., reuse tokens, merge params).
    pub fn fresh_var_repr(&mut self, ty: Idx, repr: ValueRepr) -> ArcVarId {
        let id = u32::try_from(self.var_types.len())
            .unwrap_or_else(|_| panic!("variable count exceeds u32::MAX"));
        self.var_types.push(ty);
        if !self.var_reprs.is_empty() {
            self.var_reprs.push(repr);
        }
        ArcVarId::new(id)
    }

    /// Append a new basic block to this function.
    ///
    /// The block's `id` must equal the next sequential block index
    /// (`self.blocks.len()`). Span entries are initialized to `None` for
    /// each instruction in the block body.
    ///
    /// # Panics
    ///
    /// Debug-panics if `block.id` does not match the expected index.
    pub fn push_block(&mut self, block: ArcBlock) {
        let expected = ArcBlockId::new(
            u32::try_from(self.blocks.len())
                .unwrap_or_else(|_| panic!("block count exceeds u32::MAX")),
        );
        assert_eq!(
            block.id,
            expected,
            "block ID {} does not match expected index {}",
            block.id.raw(),
            expected.raw(),
        );
        self.spans.push(vec![None; block.body.len()]);
        self.blocks.push(block);
    }

    /// Return the [`ArcBlockId`] that the next [`push_block`](Self::push_block)
    /// call will use.
    pub fn next_block_id(&self) -> ArcBlockId {
        ArcBlockId::new(
            u32::try_from(self.blocks.len())
                .unwrap_or_else(|_| panic!("block count exceeds u32::MAX")),
        )
    }
}
