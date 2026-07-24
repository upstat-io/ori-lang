use ori_ir::SharedInterner;
use ori_types::Idx;

use super::same_physical_abi;
use crate::codegen::abi::{
    CallConv, FunctionAbi, ParamAbi, ParamPassing, ReturnAbi, ReturnPassing,
};

fn parameterized_abi(name: ori_ir::Name) -> FunctionAbi {
    FunctionAbi {
        params: vec![ParamAbi {
            name,
            ty: Idx::INT,
            passing: ParamPassing::Direct,
            readonly: false,
        }],
        return_abi: ReturnAbi {
            ty: Idx::INT,
            passing: ReturnPassing::Direct,
        },
        call_conv: CallConv::Fast,
    }
}

#[test]
fn same_physical_abi_different_parameter_names_matches() {
    let symbols = SharedInterner::new();
    let left = parameterized_abi(symbols.intern("left_display_name"));
    let right = parameterized_abi(symbols.intern("right_display_name"));

    assert!(same_physical_abi(&left, &right));
}

#[test]
fn same_physical_abi_parameter_passing_mismatch_rejects() {
    let symbols = SharedInterner::new();
    let left = parameterized_abi(symbols.intern("parameter"));
    let mut right = left.clone();
    right.params[0].passing = ParamPassing::Reference;

    assert!(!same_physical_abi(&left, &right));
}

#[test]
fn same_physical_abi_parameter_readonly_mismatch_rejects() {
    let symbols = SharedInterner::new();
    let left = parameterized_abi(symbols.intern("parameter"));
    let mut right = left.clone();
    right.params[0].readonly = true;

    assert!(!same_physical_abi(&left, &right));
}

#[test]
fn same_physical_abi_parameter_type_mismatch_rejects() {
    let symbols = SharedInterner::new();
    let left = parameterized_abi(symbols.intern("parameter"));
    let mut right = left.clone();
    right.params[0].ty = Idx::BOOL;

    assert!(!same_physical_abi(&left, &right));
}

#[test]
fn same_physical_abi_return_type_mismatch_rejects() {
    let symbols = SharedInterner::new();
    let left = parameterized_abi(symbols.intern("parameter"));
    let mut right = left.clone();
    right.return_abi.ty = Idx::BOOL;

    assert!(!same_physical_abi(&left, &right));
}

#[test]
fn same_physical_abi_return_passing_mismatch_rejects() {
    let symbols = SharedInterner::new();
    let left = parameterized_abi(symbols.intern("parameter"));
    let mut right = left.clone();
    right.return_abi.passing = ReturnPassing::Sret { alignment: 8 };

    assert!(!same_physical_abi(&left, &right));
}
