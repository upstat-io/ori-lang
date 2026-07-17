use ori_arc::{ArcBlock, ArcBlockId, ArcParam, ArcVarId, ArgOwnership, Ownership};
use ori_ir::SharedInterner;
use ori_types::Pool;

use super::*;

struct Fixture {
    functions: Vec<ArcFunction>,
    facts: FxHashMap<Name, FunctionCallableFacts>,
    symbols: SharedInterner,
    main: Name,
    closure_ty: Idx,
}

fn fixture() -> Fixture {
    let symbols = SharedInterner::new();
    let main = symbols.intern("main");
    let lambda = symbols.intern("lambda");
    let mut pool = Pool::new();
    let closure_ty = pool.function(&[Idx::INT], Idx::INT);
    let main_function = ArcFunction {
        name: main,
        return_type: Idx::INT,
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![
                ArcInstr::PartialApply {
                    dst: ArcVarId::new(0),
                    ty: closure_ty,
                    func: lambda,
                    args: Vec::new(),
                },
                ArcInstr::ApplyIndirect {
                    dst: ArcVarId::new(2),
                    ty: Idx::INT,
                    closure: ArcVarId::new(0),
                    args: vec![ArcVarId::new(1)],
                    arg_ownership: vec![ArgOwnership::Borrowed],
                },
            ],
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(2),
            },
        }],
        var_types: vec![closure_ty, Idx::INT, Idx::INT],
        ..ArcFunction::default()
    };
    let lambda_function = ArcFunction {
        name: lambda,
        params: vec![ArcParam {
            var: ArcVarId::new(0),
            ty: Idx::INT,
            ownership: Ownership::Borrowed,
        }],
        return_type: Idx::INT,
        var_types: vec![Idx::INT],
        ..ArcFunction::default()
    };
    let functions = vec![main_function, lambda_function];
    let facts = ori_arc::freeze_function_callable_facts(&functions, &pool);
    Fixture {
        functions,
        facts,
        symbols,
        main,
        closure_ty,
    }
}

#[test]
fn accepts_exact_residual_signature_without_pool_access() {
    let fixture = fixture();
    assert!(freeze_callable_facts(&fixture.functions, fixture.facts, &fixture.symbols,).is_ok());
}

#[test]
fn rejects_indirect_call_without_callable_signature() {
    let mut fixture = fixture();
    fixture.facts.insert(
        fixture.main,
        FunctionCallableFacts::from_register_signatures(vec![None, None, None]),
    );
    let Err(error) = freeze_callable_facts(&fixture.functions, fixture.facts, &fixture.symbols)
    else {
        panic!("an indirect call without a callable signature unexpectedly validated")
    };
    assert!(matches!(
        error,
        RealizationError::InvalidCallableFacts { details, .. }
            if details.contains("closure construction destination")
    ));
}

#[test]
fn rejects_indirect_call_arity_drift_before_backend_projection() {
    let mut fixture = fixture();
    fixture.facts.insert(
        fixture.main,
        FunctionCallableFacts::from_register_signatures(vec![
            Some(ClosureValueSignature::from_parts(
                fixture.closure_ty,
                Vec::new(),
                Idx::INT,
            )),
            None,
            None,
        ]),
    );
    let Err(error) = freeze_callable_facts(&fixture.functions, fixture.facts, &fixture.symbols)
    else {
        panic!("an indirect call with arity drift unexpectedly validated")
    };
    assert!(matches!(
        error,
        RealizationError::InvalidCallableFacts { details, .. }
            if details.contains("residual target")
    ));
}
