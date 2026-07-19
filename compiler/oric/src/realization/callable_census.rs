//! Semantic callable seeds before grouped ARC preparation.

use ori_arc::{ArcFunction, ArcInstr, CtorKind};
use ori_ir::{Function, Name, StringInterner};
use ori_types::{FunctionSig, Idx, Pool, Tag};
use rustc_hash::FxHashMap;

use super::ArcFunctionGroup;

/// One canonical source-function seed and its checked signature.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SourceCallableSeed<'a> {
    pub(crate) function: &'a Function,
    pub(crate) signature: &'a FunctionSig,
}

/// Failure while replacing raw declaration enumeration with semantic seeds.
#[derive(Debug, thiserror::Error)]
pub enum CallableCensusError {
    #[error(
        "callable census received {functions} source declarations but {signatures} checked signatures"
    )]
    SignatureCountMismatch { functions: usize, signatures: usize },
    #[error(
        "callable census source declaration `{callable}` disagrees with checked signature identity `{signature}`"
    )]
    NameMismatch { callable: String, signature: String },
    #[error(
        "callable census found conflicting checked signatures for source callable `{callable}`; guard clauses must share one exact signature"
    )]
    ConflictingSourceSignatures { callable: String },
    #[error("callable census cannot materialize the registered Error constructor: {reason}")]
    MalformedBuiltinError { reason: &'static str },
    #[error(
        "callable census found a body named `{callable}` that conflicts with the registered Error constructor identity"
    )]
    ConflictingBuiltinTarget { callable: String },
}

/// Incremental builder for the backend-neutral whole-program callable census.
pub(crate) struct CallableCensusBuilder<'a> {
    interner: &'a StringInterner,
}

impl<'a> CallableCensusBuilder<'a> {
    pub(crate) const fn new(interner: &'a StringInterner) -> Self {
        Self { interner }
    }

    /// Coalesce raw source clauses after Canon has already synthesized their
    /// one name-keyed body. Exact duplicate declarations likewise publish one
    /// seed; signature disagreement remains a hard producer error.
    pub(crate) fn source_functions<'b>(
        &self,
        functions: &'b [Function],
        signatures: &'b [FunctionSig],
    ) -> Result<Vec<SourceCallableSeed<'b>>, CallableCensusError> {
        if functions.len() != signatures.len() {
            return Err(CallableCensusError::SignatureCountMismatch {
                functions: functions.len(),
                signatures: signatures.len(),
            });
        }

        let mut seeds: Vec<SourceCallableSeed<'b>> = Vec::new();
        let mut seed_by_name: FxHashMap<Name, usize> = FxHashMap::default();
        for (function, signature) in functions.iter().zip(signatures) {
            if function.name != signature.name {
                return Err(CallableCensusError::NameMismatch {
                    callable: self.interner.lookup(function.name).to_owned(),
                    signature: self.interner.lookup(signature.name).to_owned(),
                });
            }
            if let Some(&existing_index) = seed_by_name.get(&function.name) {
                if seeds[existing_index].signature != signature {
                    return Err(CallableCensusError::ConflictingSourceSignatures {
                        callable: self.interner.lookup(function.name).to_owned(),
                    });
                }
                continue;
            }
            seed_by_name.insert(function.name, seeds.len());
            seeds.push(SourceCallableSeed {
                function,
                signature,
            });
        }
        Ok(seeds)
    }

    /// Close compiler-owned callable roots requested by the assembled corpus.
    ///
    /// A first-class `Error` constructor lowers to a zero-capture closure, so
    /// it needs an ordinary body just like a source function. Selection uses
    /// the pool-registered Error identity and exact closure type; a same-spelled
    /// free function is not a builtin request.
    pub(crate) fn close_builtin_targets(
        &self,
        groups: &mut Vec<ArcFunctionGroup>,
        pool: &Pool,
    ) -> Result<(), CallableCensusError> {
        let Some(error_type) = pool.error_struct_idx() else {
            return Ok(());
        };
        if !pool.is_valid_idx(error_type) {
            return Err(CallableCensusError::MalformedBuiltinError {
                reason: "the registered type index is outside the type pool",
            });
        }
        let resolved_error = pool.resolve_fully(error_type);
        if pool.tag(resolved_error) != Tag::Struct {
            return Err(CallableCensusError::MalformedBuiltinError {
                reason: "the registered type does not resolve to a struct",
            });
        }
        let error_name = pool.struct_name(resolved_error);

        let requested = groups
            .iter()
            .flat_map(ArcFunctionGroup::bodies)
            .any(|function| {
                function.blocks.iter().any(|block| {
                    block.body.iter().any(|instruction| match instruction {
                        ArcInstr::PartialApply { ty, func, args, .. }
                        | ArcInstr::Construct {
                            ty,
                            ctor: CtorKind::Closure { func },
                            args,
                            ..
                        } => {
                            *func == error_name
                                && args.is_empty()
                                && is_error_constructor_closure_type(pool, *ty)
                        }
                        _ => false,
                    })
                })
            });
        if !requested {
            return Ok(());
        }

        let mut same_named_body = false;
        for function in groups.iter().flat_map(ArcFunctionGroup::bodies) {
            if function.name != error_name {
                continue;
            }
            same_named_body = true;
            if is_error_constructor_body(pool, function) {
                return Ok(());
            }
        }
        if same_named_body {
            return Err(CallableCensusError::ConflictingBuiltinTarget {
                callable: self.interner.lookup(error_name).to_owned(),
            });
        }

        let fields = pool.struct_fields(resolved_error);
        if fields.len() != 2 || pool.resolve_fully(fields[0].1) != Idx::STR {
            return Err(CallableCensusError::MalformedBuiltinError {
                reason: "the registered struct must contain message: str and trace fields",
            });
        }
        let trace_list_type = fields[1].1;
        if pool.tag(pool.resolve_fully(trace_list_type)) != Tag::List {
            return Err(CallableCensusError::MalformedBuiltinError {
                reason: "the registered trace field is not a list",
            });
        }
        groups.push(ArcFunctionGroup::new(
            ori_arc::build_builtin_error_constructor(error_name, error_type, trace_list_type, pool),
            Vec::new(),
        ));
        Ok(())
    }
}

fn is_error_constructor_closure_type(pool: &Pool, ty: Idx) -> bool {
    if !pool.is_valid_idx(ty) {
        return false;
    }
    let resolved = pool.resolve_fully(ty);
    pool.tag(resolved) == Tag::Function
        && pool.function_param_count(resolved) == 1
        && pool.resolve_fully(pool.function_param(resolved, 0)) == Idx::STR
        && pool.is_error_struct_receiver(pool.function_return(resolved))
}

fn is_error_constructor_body(pool: &Pool, function: &ArcFunction) -> bool {
    function.params.len() == 1
        && pool.resolve_fully(function.params[0].ty) == Idx::STR
        && pool.is_error_struct_receiver(function.return_type)
}

#[cfg(test)]
mod tests;
