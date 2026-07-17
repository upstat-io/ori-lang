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
mod tests;
