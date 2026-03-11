//! Argument ownership annotation for AIMS RC emission.
//!
//! Populates `arg_ownership` on `Apply`/`Invoke` instructions from
//! [`MemoryContract`] signatures. Delegates to the existing
//! [`annotate_arg_ownership`](crate::rc_insert::annotate_arg_ownership)
//! after converting contracts to [`AnnotatedSig`]s.
//!
//! # Stage 1
//!
//! During Stage 1, this is a thin wrapper: convert `MemoryContract` →
//! `AnnotatedSig` via contract fields, then call the existing annotation
//! function. This avoids duplicating the type-qualified builtin dispatch
//! logic (250+ lines). Post-Stage 1, this should be replaced with direct
//! `MemoryContract` consumption.

use rustc_hash::FxHashMap;

use ori_ir::{Name, StringInterner};
use ori_types::Pool;

use crate::aims::contract::MemoryContract;
use crate::aims::lattice::{AccessClass, Consumption};
use crate::ir::ArcFunction;
use crate::ownership::{AnnotatedParam, AnnotatedSig, Ownership};
use crate::BuiltinOwnershipSets;

/// Populate `arg_ownership` on all call sites from AIMS contracts.
///
/// Converts each `MemoryContract` to an `AnnotatedSig` and delegates to
/// [`crate::rc_insert::annotate_arg_ownership`]. This preserves the
/// type-qualified builtin dispatch logic during Stage 1.
///
/// Must be called **before** RC emission (pipeline step 4 in Section 06.2).
#[expect(clippy::implicit_hasher, reason = "FxHashMap is the canonical hasher")]
pub fn emit_arg_ownership(
    func: &mut ArcFunction,
    contracts: &FxHashMap<Name, MemoryContract>,
    interner: &StringInterner,
    builtins: &BuiltinOwnershipSets,
    pool: &Pool,
) {
    let sigs: FxHashMap<Name, AnnotatedSig> = contracts
        .iter()
        .map(|(&name, contract)| {
            let params = contract
                .params
                .iter()
                .enumerate()
                .map(|(i, pc)| {
                    let ownership = if pc.consumption == Consumption::Dead {
                        Ownership::Borrowed
                    } else {
                        match pc.access {
                            AccessClass::Borrowed => Ownership::Borrowed,
                            AccessClass::Owned => Ownership::Owned,
                        }
                    };
                    AnnotatedParam {
                        name: Name::from_raw(
                            u32::try_from(i)
                                .unwrap_or_else(|_| panic!("param index {i} exceeds u32::MAX")),
                        ),
                        ty: ori_types::Idx::NONE,
                        ownership,
                    }
                })
                .collect();
            (
                name,
                AnnotatedSig {
                    params,
                    return_type: ori_types::Idx::NONE,
                },
            )
        })
        .collect();

    crate::rc_insert::annotate_arg_ownership(func, &sigs, interner, builtins, pool);
}
