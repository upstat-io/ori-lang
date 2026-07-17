use ori_ir::{
    ExprId, Function, GenericParamRange, Name, ParamRange, Span, StringInterner, Visibility,
};
use ori_types::{FunctionSig, Idx};

use super::{CallableCensusBuilder, CallableCensusError};

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
