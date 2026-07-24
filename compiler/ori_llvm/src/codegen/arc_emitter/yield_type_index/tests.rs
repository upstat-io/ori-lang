use ori_arc::ir::{
    AllocationSiteId, ArcFunction, ArcVarId, YieldAllocationFact, YieldAllocationLocality,
    YieldExtent,
};
use ori_types::Idx;

use super::index_yield_types_by_elem_size_var;

#[test]
fn yield_element_size_index_reads_the_typed_fact() {
    let elem_size_var = ArcVarId::new(2);
    let function = ArcFunction {
        var_types: vec![Idx::UNIT, Idx::BOOL, Idx::INT],
        yield_allocations: vec![YieldAllocationFact {
            site: AllocationSiteId::new(0),
            builder: ArcVarId::new(0),
            result: ArcVarId::new(1),
            elem_ty: Idx::INT,
            elem_size_var,
            elem_size: 8,
            extent: YieldExtent::StaticExact(4),
            locality: YieldAllocationLocality::Local,
        }],
        ..ArcFunction::default()
    };

    let index = index_yield_types_by_elem_size_var(&function);
    assert_eq!(index.get(&elem_size_var), Some(&(Idx::BOOL, Idx::INT)));
}
