//! Semantic callable seeds before grouped ARC preparation.

use ori_ir::{Function, Name, StringInterner};
use ori_types::FunctionSig;
use rustc_hash::FxHashMap;

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
}

#[cfg(test)]
mod tests {
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
}
