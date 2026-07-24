use ori_ir::canon::CanonResult;
use ori_ir::StringInterner;
use ori_repr::monomorphize::{MonoFunction, MonoFunctionIdentity, MonoFunctionOrigin};
use ori_types::{FunctionSig, Idx, MonoInstance, Pool};
use rustc_hash::FxHashMap;

use super::lower_mono_functions_for_analysis;

#[test]
fn imported_metadata_is_not_lowered_against_host_canon() {
    let interner = StringInterner::new();
    let name = interner.intern("imported_identity$m$3_int");
    let original_name = interner.intern("imported_identity");
    let instance = MonoInstance::new_top_level(
        original_name,
        Vec::new(),
        vec![Idx::INT],
        Idx::INT,
        Vec::new(),
    );
    let monos = vec![MonoFunction {
        mangled_name: name,
        origin: MonoFunctionOrigin::Source,
        identity: MonoFunctionIdentity::generated(&instance),
        sig: FunctionSig::simple(name, vec![Idx::INT], Idx::INT),
        body_type_map: FxHashMap::default(),
        is_imported: true,
        receiver_type_name: None,
    }];
    let mut problems = Vec::new();
    let canon = CanonResult::empty();
    let pool = Pool::new();
    let mut context = crate::arc_lowering::ArcLoweringContext {
        canon: &canon,
        interner: &interner,
        pool: &pool,
        problems: &mut problems,
    };

    let groups = lower_mono_functions_for_analysis(&monos, &[], &[], &mut context);

    assert!(groups.is_empty());
    assert!(problems.is_empty());
}
