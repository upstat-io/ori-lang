//! Callable-contract and public-export projection from a closed executable.

use ori_arc::aims::lattice::{AccessClass, Consumption};
use ori_arc::{AnnotatedParam, Ownership};
use ori_ir::{Name, StringInterner};
use ori_repr::executable::ExecutableProgram;
use ori_repr::monomorphize::ImportSig;
use ori_types::FunctionSig;
use oric::parser::ParseOutput;
use rustc_hash::FxHashMap;

use super::RealizedCallableExport;

pub(super) fn artifact_annotated_signatures(
    program: &ExecutableProgram,
    imports: &[ImportSig],
) -> Result<FxHashMap<Name, ori_arc::AnnotatedSig>, String> {
    let mut signatures = FxHashMap::default();
    for function in program.functions() {
        let function_id = program.function_id(function.name).ok_or_else(|| {
            format!(
                "validated executable has no stable identity for {:?}",
                function.name
            )
        })?;
        signatures.insert(
            function.name,
            program
                .function_contract(function_id)
                .to_annotated_sig(&function.params, function.return_type),
        );
    }
    for import in imports {
        let external_id = program.external_function_id(import.name).ok_or_else(|| {
            format!(
                "validated executable has no external callable for imported alias {:?}",
                import.name
            )
        })?;
        let external = program.external_function(external_id);
        let params = import
            .sig
            .param_names
            .iter()
            .copied()
            .zip(external.parameter_types().iter().copied())
            .zip(&external.contract().params)
            .map(|((name, ty), contract)| AnnotatedParam {
                name,
                ty,
                ownership: if contract.consumption == Consumption::Dead
                    || contract.access == AccessClass::Borrowed
                {
                    Ownership::Borrowed
                } else {
                    Ownership::Owned
                },
            })
            .collect();
        signatures.insert(
            import.name,
            ori_arc::AnnotatedSig {
                params,
                return_type: external.return_type(),
            },
        );
    }
    Ok(signatures)
}

pub(super) fn project_callable_exports(
    program: &ExecutableProgram,
    parse: &ParseOutput,
    signatures: &[FunctionSig],
    interner: &StringInterner,
    symbol_prefix: &str,
) -> Result<Vec<RealizedCallableExport>, String> {
    let mangler = ori_llvm::aot::Mangler::new();
    let mut exports = Vec::new();
    for (function, signature) in parse.module.functions.iter().zip(signatures) {
        if !function.visibility.is_public() || signature.is_generic() {
            continue;
        }
        let function_id = program.function_id(function.name).ok_or_else(|| {
            format!(
                "closed executable omitted public callable '{}'",
                interner.lookup(function.name)
            )
        })?;
        let source_name = interner.lookup(function.name).to_string();
        let mangled_name = mangler.mangle_function(symbol_prefix, &source_name);
        let metadata = program.export_callable_metadata(function_id, &mangled_name);
        exports.push(RealizedCallableExport {
            mangled_name,
            source_name,
            metadata,
        });
    }
    Ok(exports)
}
