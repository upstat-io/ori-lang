use ori_ir::{
    ExprId, Function, GenericParamRange, Name, ParamRange, Span, StringInterner, Visibility,
};
use ori_types::{FunctionSig, Idx, Pool};

use super::{ArcFunctionGroup, CallableCensusBuilder, CallableCensusError};

fn function(name: Name) -> Function {
    Function {
        name,
        generics: GenericParamRange::EMPTY,
        params: ParamRange::EMPTY,
        return_ty: None,
        capabilities: Vec::new(),
        where_clauses: Vec::new(),
        guard: None,
        pre_contracts: Vec::new(),
        post_contracts: Vec::new(),
        body: ExprId::INVALID,
        span: Span::DUMMY,
        visibility: Visibility::Private,
        is_fbip: false,
        target_attr: None,
        cfg_attr: None,
    }
}

#[test]
fn repeated_source_clauses_publish_one_seed() {
    let interner = StringInterner::new();
    let name = interner.intern("classify");
    let functions = vec![function(name), function(name), function(name)];
    let signature = FunctionSig::synthetic(name, Vec::new(), Vec::new(), Idx::STR);
    let signatures = vec![signature.clone(), signature.clone(), signature];

    let seeds = CallableCensusBuilder::new(&interner)
        .source_functions(&functions, &signatures)
        .unwrap_or_else(|error| panic!("matching guard clauses must coalesce: {error}"));

    assert_eq!(seeds.len(), 1);
    assert_eq!(seeds[0].function.name, name);
}

#[test]
fn repeated_source_name_with_conflicting_signature_fails_closed() {
    let interner = StringInterner::new();
    let name = interner.intern("conflict");
    let functions = vec![function(name), function(name)];
    let signatures = vec![
        FunctionSig::synthetic(name, Vec::new(), Vec::new(), Idx::INT),
        FunctionSig::synthetic(name, Vec::new(), Vec::new(), Idx::STR),
    ];

    let Err(error) =
        CallableCensusBuilder::new(&interner).source_functions(&functions, &signatures)
    else {
        panic!("conflicting signatures must not be first-wins")
    };

    assert!(matches!(
        error,
        CallableCensusError::ConflictingSourceSignatures { .. }
    ));
    assert!(error.to_string().contains("conflict"));
}

fn registered_error_pool(interner: &StringInterner) -> (Pool, Name, Idx) {
    let mut pool = Pool::new();
    let error_name = interner.intern("Error");
    let message_name = interner.intern("message");
    let trace_name = interner.intern("trace");
    let trace_entry = pool.named(interner.intern("TraceEntry"));
    let trace_list = pool.list(trace_entry);
    let error_type = pool.named(error_name);
    let error_struct = pool.struct_type(
        error_name,
        &[(message_name, Idx::STR), (trace_name, trace_list)],
    );
    pool.set_resolution(error_type, error_struct);
    pool.set_error_struct_idx(error_type);
    (pool, error_name, error_type)
}

fn closure_reference(parent: Name, target: Name, closure_type: Idx) -> ArcFunctionGroup {
    let closure = ori_arc::ArcVarId::new(0);
    ArcFunctionGroup::new(
        ori_arc::ArcFunction {
            name: parent,
            return_type: closure_type,
            blocks: vec![ori_arc::ArcBlock {
                id: ori_arc::ArcBlockId::new(0),
                params: Vec::new(),
                body: vec![ori_arc::ArcInstr::PartialApply {
                    dst: closure,
                    ty: closure_type,
                    func: target,
                    args: Vec::new(),
                }],
                terminator: ori_arc::ArcTerminator::Return { value: closure },
            }],
            var_types: vec![closure_type],
            spans: vec![vec![None]],
            ..ori_arc::ArcFunction::default()
        },
        Vec::new(),
    )
}

#[test]
fn first_class_error_constructor_adds_one_ordinary_body() {
    let interner = StringInterner::new();
    let (mut pool, error_name, error_type) = registered_error_pool(&interner);
    let closure_type = pool.function1(Idx::STR, error_type);
    let mut groups = vec![closure_reference(
        interner.intern("main"),
        error_name,
        closure_type,
    )];

    let census = CallableCensusBuilder::new(&interner);
    census
        .close_builtin_targets(&mut groups, &pool)
        .unwrap_or_else(|error| panic!("registered Error closure must close: {error}"));
    census
        .close_builtin_targets(&mut groups, &pool)
        .unwrap_or_else(|error| panic!("builtin closure completion must be idempotent: {error}"));

    let bodies: Vec<_> = groups
        .iter()
        .flat_map(ArcFunctionGroup::bodies)
        .filter(|function| function.name == error_name)
        .collect();
    assert_eq!(bodies.len(), 1);
    let body = bodies[0];
    assert_eq!(body.params.len(), 1);
    assert_eq!(body.params[0].ty, Idx::STR);
    assert_eq!(body.return_type, error_type);
    assert!(matches!(
        body.blocks[0].body.as_slice(),
        [
            ori_arc::ArcInstr::Construct {
                ctor: ori_arc::CtorKind::ListLiteral,
                args,
                ..
            },
            ori_arc::ArcInstr::Construct {
                ctor: ori_arc::CtorKind::Struct(name),
                args: fields,
                ..
            }
        ] if args.is_empty() && *name == error_name && fields.len() == 2
    ));
}

#[test]
fn same_spelled_non_error_function_does_not_seed_builtin_body() {
    let interner = StringInterner::new();
    let (mut pool, error_name, _) = registered_error_pool(&interner);
    let unrelated_type = pool.function1(Idx::STR, Idx::INT);
    let mut groups = vec![closure_reference(
        interner.intern("main"),
        error_name,
        unrelated_type,
    )];

    CallableCensusBuilder::new(&interner)
        .close_builtin_targets(&mut groups, &pool)
        .unwrap_or_else(|error| panic!("same spelling is not builtin identity: {error}"));

    assert_eq!(groups.len(), 1);
}
