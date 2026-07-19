//! [`ArcFunction`] methods — variable management, block allocation, and
//! type/representation lookups.

use ori_ir::Name;
use ori_types::Idx;

use super::{
    ArcBlock, ArcBlockId, ArcFunction, ArcTerminator, ArcVarId, RcStrategy, ValueRepr,
    VariableMetadataState,
};

/// Minimal empty function for test construction.
///
/// Tests use struct update syntax to override only the fields they care about:
/// ```text
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
            var_rc_strategies: Vec::new(),
            var_metadata_state: VariableMetadataState::Unrealized,
            spans: vec![Vec::new()],
            is_fbip: false,
            num_captures: 0,
            cow_annotations: crate::uniqueness::CowAnnotations::default(),
            primitive_facts: super::PrimitiveFacts::default(),
            drop_hints: crate::uniqueness::DropHints::default(),
            tail_calls: Vec::new(),
            burden_emitted: Vec::new(),
            reassign_deaths: Vec::new(),
            catch_scoped_checked_ops: Vec::new(),
            method_call_facts: Vec::new(),
            operator_call_facts: Vec::new(),
            direct_call_facts: Vec::new(),
            class_ledger_emission: false,
        }
    }
}

impl super::ArcFunction {
    /// Return the exact method-call provenance for a stable result register.
    #[must_use]
    pub fn method_call_fact(&self, destination: ArcVarId) -> Option<super::MethodCallFact> {
        self.method_call_facts
            .iter()
            .find(|fact| fact.destination == destination)
            .cloned()
    }

    /// Return exact generated free-call provenance for one result register.
    #[must_use]
    pub fn direct_call_fact(&self, destination: ArcVarId) -> Option<super::DirectCallFact> {
        self.direct_call_facts
            .iter()
            .find(|fact| fact.destination == destination)
            .cloned()
    }

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

    /// Look up the backend-neutral ownership-relevant shape of a variable.
    ///
    /// Only valid after [`compute_var_reprs`](super::compute_var_reprs) has
    /// been called. Returns `None` only while representation metadata is
    /// explicitly unrealized; table emptiness is not a readiness signal.
    #[inline]
    pub fn var_repr(&self, var: ArcVarId) -> Option<ValueRepr> {
        match self.var_metadata_state {
            VariableMetadataState::Unrealized => return None,
            VariableMetadataState::RepresentationsReady | VariableMetadataState::Realized => {}
        }
        assert_eq!(
            self.var_reprs.len(),
            self.var_types.len(),
            "ready var_reprs must be parallel to var_types"
        );
        assert!(
            var.index() < self.var_reprs.len(),
            "ArcVarId {} out of bounds for var_reprs (have {} entries)",
            var.raw(),
            self.var_reprs.len(),
        );
        Some(self.var_reprs[var.index()])
    }

    /// Allocate a fresh variable before representation metadata is realized.
    ///
    /// Returns a new [`ArcVarId`] that does not collide with any existing
    /// variable in this function. The variable's type is recorded in
    /// [`var_types`](Self::var_types).
    ///
    /// The method name makes metadata readiness a caller-owned part of the
    /// allocation contract. Later rewrites must use
    /// [`fresh_scalar_var`](Self::fresh_scalar_var) or
    /// [`fresh_var_like`](Self::fresh_var_like) so metadata stays exact,
    /// including for a realized function that starts with zero variables.
    ///
    /// # Panics
    ///
    /// Panics if realized variable metadata is already populated.
    pub fn fresh_unrealized_var(&mut self, ty: Idx) -> ArcVarId {
        self.assert_unrealized_metadata();
        self.allocate_var(ty)
    }

    /// Allocate a fresh variable whose value is proven scalar by its caller.
    ///
    /// The allocation advances every metadata table owned by the function's
    /// current lifecycle state: types only while unrealized, types plus
    /// representations while representation-ready, or all three tables when
    /// fully realized. Zero-variable states remain unambiguous.
    pub fn fresh_scalar_var(&mut self, ty: Idx) -> ArcVarId {
        match self.var_metadata_state {
            VariableMetadataState::Unrealized => {
                self.assert_unrealized_metadata();
                self.allocate_var(ty)
            }
            VariableMetadataState::RepresentationsReady => {
                self.assert_representations_ready();
                let var = self.allocate_var(ty);
                self.var_reprs.push(ValueRepr::Scalar);
                var
            }
            VariableMetadataState::Realized => {
                self.assert_realized_metadata();
                let var = self.allocate_var(ty);
                self.var_reprs.push(ValueRepr::Scalar);
                self.var_rc_strategies.push(None);
                var
            }
        }
    }

    /// Allocate a fresh variable with the exact metadata of `source`.
    ///
    /// This is the metadata-preserving variable API for aliases, block params,
    /// and renamed definitions. It copies every derived table that is ready in
    /// the current lifecycle state as one indivisible parallel-table operation.
    pub fn fresh_var_like(&mut self, source: ArcVarId) -> ArcVarId {
        let ty = self.var_type(source);
        match self.var_metadata_state {
            VariableMetadataState::Unrealized => {
                self.assert_unrealized_metadata();
                self.allocate_var(ty)
            }
            VariableMetadataState::RepresentationsReady => {
                self.assert_representations_ready();
                let repr = self.var_reprs[source.index()];
                let var = self.allocate_var(ty);
                self.var_reprs.push(repr);
                var
            }
            VariableMetadataState::Realized => {
                self.assert_realized_metadata();
                let repr = self.var_reprs[source.index()];
                let strategy = self.var_rc_strategies[source.index()];
                let var = self.allocate_var(ty);
                self.var_reprs.push(repr);
                self.var_rc_strategies.push(strategy);
                var
            }
        }
    }

    /// Allocate a metadata-preserving alias and release-check its IR type.
    ///
    /// Rewrites that also emit an instruction or block-parameter type must use
    /// this API so the source variable and emitted type cannot become separate
    /// authorities in optimized builds.
    pub fn fresh_var_like_typed(&mut self, source: ArcVarId, expected_ty: Idx) -> ArcVarId {
        let source_ty = self.var_type(source);
        assert_eq!(
            source_ty, expected_ty,
            "metadata-preserving alias type must match its source variable"
        );
        self.fresh_var_like(source)
    }

    /// Replace the representation table and mark it ready without claiming
    /// that RC strategies have been derived.
    pub(crate) fn replace_variable_representations(&mut self, representations: Vec<ValueRepr>) {
        assert_ne!(
            self.var_metadata_state,
            VariableMetadataState::Realized,
            "fully realized metadata cannot be downgraded to representation-ready"
        );
        match self.var_metadata_state {
            VariableMetadataState::Unrealized => assert!(
                self.var_reprs.is_empty() && self.var_rc_strategies.is_empty(),
                "unrealized metadata must be absent before representations become ready"
            ),
            VariableMetadataState::RepresentationsReady => {
                self.assert_representations_ready();
            }
            VariableMetadataState::Realized => unreachable!("guarded above"),
        }
        assert_eq!(
            representations.len(),
            self.var_types.len(),
            "ready representations must be parallel to var_types"
        );
        assert!(
            self.var_rc_strategies.is_empty(),
            "representation readiness cannot discard RC strategies"
        );
        self.var_reprs = representations;
        self.var_metadata_state = VariableMetadataState::RepresentationsReady;
    }

    /// Invalidate derived variable metadata before a sanctioned type rewrite.
    ///
    /// Pre-AIMS lambda specialization is allowed to replace unresolved type
    /// variables after lowering has derived representation metadata. The type
    /// rewrite must explicitly move the function back to `Unrealized` before
    /// changing any types, so the AIMS owner derives both metadata tables once
    /// from the final specialized types. Fully realized metadata is immutable.
    pub(crate) fn invalidate_variable_metadata_for_type_rewrite(&mut self) {
        match self.var_metadata_state {
            VariableMetadataState::Unrealized => self.assert_unrealized_metadata(),
            VariableMetadataState::RepresentationsReady => {
                self.assert_representations_ready();
                assert!(
                    self.var_rc_strategies.is_empty(),
                    "representation-ready metadata cannot carry RC strategies"
                );
                self.var_reprs.clear();
                self.var_metadata_state = VariableMetadataState::Unrealized;
            }
            VariableMetadataState::Realized => {
                panic!("fully realized variable metadata is immutable across type rewrites")
            }
        }
    }

    /// Replace both derived metadata tables and mark them realized atomically.
    pub(crate) fn replace_realized_variable_metadata(
        &mut self,
        representations: Vec<ValueRepr>,
        strategies: Vec<Option<RcStrategy>>,
    ) {
        assert_eq!(
            self.var_metadata_state,
            VariableMetadataState::Unrealized,
            "complete metadata replacement requires an unrealized function"
        );
        assert!(
            self.var_reprs.is_empty() && self.var_rc_strategies.is_empty(),
            "complete metadata replacement cannot silently repair existing tables"
        );
        assert_eq!(
            representations.len(),
            self.var_types.len(),
            "realized representations must be parallel to var_types"
        );
        assert_eq!(
            strategies.len(),
            self.var_types.len(),
            "realized RC strategies must be parallel to var_types"
        );
        self.var_reprs = representations;
        self.var_rc_strategies = strategies;
        self.var_metadata_state = VariableMetadataState::Realized;
    }

    /// Complete a validated representation-ready function with RC strategies.
    pub(crate) fn complete_variable_metadata(&mut self, strategies: Vec<Option<RcStrategy>>) {
        self.assert_representations_ready();
        assert_eq!(
            strategies.len(),
            self.var_types.len(),
            "realized RC strategies must be parallel to var_types"
        );
        self.var_rc_strategies = strategies;
        self.var_metadata_state = VariableMetadataState::Realized;
    }

    fn allocate_var(&mut self, ty: Idx) -> ArcVarId {
        let id = u32::try_from(self.var_types.len())
            .unwrap_or_else(|_| panic!("variable count exceeds u32::MAX"));
        assert!(
            id < u32::MAX,
            "ARC var ID would collide with INVALID sentinel (u32::MAX)"
        );
        self.var_types.push(ty);
        ArcVarId::new(id)
    }

    fn assert_realized_metadata(&self) {
        assert_eq!(
            self.var_metadata_state,
            VariableMetadataState::Realized,
            "realized variable allocation requires realized metadata state"
        );
        assert_eq!(
            self.var_reprs.len(),
            self.var_types.len(),
            "realized var_reprs must be parallel to var_types before allocating a variable"
        );
        assert_eq!(
            self.var_rc_strategies.len(),
            self.var_types.len(),
            "realized var_rc_strategies must be parallel to var_types before allocating a variable"
        );
    }

    fn assert_representations_ready(&self) {
        assert_eq!(
            self.var_metadata_state,
            VariableMetadataState::RepresentationsReady,
            "representation-preserving allocation requires ready representations"
        );
        assert_eq!(
            self.var_reprs.len(),
            self.var_types.len(),
            "ready var_reprs must be parallel to var_types before allocating a variable"
        );
        assert!(
            self.var_rc_strategies.is_empty(),
            "representation-only metadata must not contain RC strategies"
        );
    }

    fn assert_unrealized_metadata(&self) {
        assert_eq!(
            self.var_metadata_state,
            VariableMetadataState::Unrealized,
            "unrealized variable allocation requires unrealized metadata state"
        );
        assert!(
            self.var_reprs.is_empty() && self.var_rc_strategies.is_empty(),
            "unrealized variable allocation requires absent derived metadata"
        );
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

    /// Return the vacant [`ArcBlockId`] at the end of the block table.
    pub fn next_block_id(&self) -> ArcBlockId {
        ArcBlockId::new(
            u32::try_from(self.blocks.len())
                .unwrap_or_else(|_| panic!("block count exceeds u32::MAX")),
        )
    }
}
