//! Content-stable compatibility identities for external callable facts.

use ori_arc::aims::contract::{FipContract, ReturnAliasShape};
use ori_arc::aims::lattice::{
    AccessClass, Cardinality, Consumption, Locality, ReuseCtorKind, ShapeClass, Uniqueness,
};
use ori_arc::{EffectSummary, MemoryContract};
use ori_types::{Idx, Pool};

use super::{ExternalFactIdentities, ExternalUnwind};

pub(super) fn compute_identities(
    link_symbol: &str,
    parameter_types: &[Idx],
    return_type: Idx,
    contract: &MemoryContract,
    unwind: ExternalUnwind,
    pool: &Pool,
) -> ExternalFactIdentities {
    let mut signature = StableHasher::new(b"ori.external.signature.v1");
    signature.bytes(link_symbol.as_bytes());
    signature.usize(parameter_types.len());
    for &ty in parameter_types {
        signature.u64(pool.hash(ty));
    }
    signature.u64(pool.hash(return_type));

    let mut ownership = StableHasher::new(b"ori.external.ownership.v1");
    ownership.bytes(link_symbol.as_bytes());
    hash_contract_ownership(&mut ownership, contract);

    let mut effects = StableHasher::new(b"ori.external.effects.v1");
    effects.bytes(link_symbol.as_bytes());
    hash_effects(&mut effects, contract.effects);

    let mut unwind_hash = StableHasher::new(b"ori.external.unwind.v1");
    unwind_hash.bytes(link_symbol.as_bytes());
    unwind_hash.tag(match unwind {
        ExternalUnwind::NoUnwind => 0,
        ExternalUnwind::MayUnwind => 1,
    });

    ExternalFactIdentities::from_raw(
        signature.finish(),
        ownership.finish(),
        effects.finish(),
        unwind_hash.finish(),
    )
}

fn hash_contract_ownership(hasher: &mut StableHasher, contract: &MemoryContract) {
    hasher.usize(contract.params.len());
    for param in &contract.params {
        hasher.tag(match param.access {
            AccessClass::Borrowed => 0,
            AccessClass::Owned => 1,
        });
        hasher.tag(match param.consumption {
            Consumption::Dead => 0,
            Consumption::Linear => 1,
            Consumption::Affine => 2,
            Consumption::Unrestricted => 3,
        });
        hasher.tag(match param.cardinality {
            Cardinality::Absent => 0,
            Cardinality::Once => 1,
            Cardinality::Many => 2,
        });
        hasher.bool(param.may_escape);
        hasher.bool(param.may_share);
        hash_locality(hasher, param.locality_bound);
        hash_uniqueness(hasher, param.uniqueness);
        hasher.bool(param.transfers_through_return);
        match param.return_alias {
            None => hasher.tag(0),
            Some(ReturnAliasShape::Direct) => hasher.tag(1),
            Some(ReturnAliasShape::Project { field }) => {
                hasher.tag(2);
                hasher.u32(field);
            }
        }
        hasher.bool(param.return_payload_contains_param);
        hasher.bool(param.iter_consumes);
        hasher.bool(param.borrowed_read_only);
        hasher.bool(param.borrowed_cow_consumed);
        hasher.bool(param.borrowed_cow_mutated);
        match param.iter_consumes_projected_field {
            Some(field) => {
                hasher.tag(1);
                hasher.u32(field);
            }
            None => hasher.tag(0),
        }
    }

    hash_uniqueness(hasher, contract.return_info.uniqueness);
    hasher.bool(contract.return_info.preserves_freshness);
    hash_locality(hasher, contract.return_info.locality);
    hasher.tag(match contract.return_info.shape {
        ShapeClass::NonReusable => 0,
        ShapeClass::ReusableCtor(ReuseCtorKind::Struct) => 1,
        ShapeClass::ReusableCtor(ReuseCtorKind::EnumVariant) => 2,
        ShapeClass::CollectionBuffer => 3,
        ShapeClass::ContextHole => 4,
    });
    hasher.bool(contract.return_info.returns_fresh_self_alloc);
    hasher.bool(contract.return_info.returns_sharing_view);

    hasher.bool(contract.context_behavior.preserves_context);
    hasher.bool(contract.context_behavior.consumes_hole);
    hasher.bool(contract.context_behavior.requires_unique_context);
    hasher.bool(contract.context_behavior.may_resume_nonlinearly);
    match &contract.fip {
        FipContract::Never => hasher.tag(0),
        FipContract::Conditional {
            requires_unique_params,
        } => {
            hasher.tag(1);
            hasher.usize(requires_unique_params.len());
            for &required in requires_unique_params {
                hasher.bool(required);
            }
        }
        FipContract::Certified => hasher.tag(2),
        FipContract::Bounded(limit) => {
            hasher.tag(3);
            hasher.u16(*limit);
        }
    }
    hasher.bool(contract.is_fbip);
}

fn hash_effects(hasher: &mut StableHasher, effects: EffectSummary) {
    hasher.bool(effects.may_allocate);
    hasher.bool(effects.alloc_only_on_slow_path);
    hasher.bool(effects.may_deallocate);
    hasher.bool(effects.may_share);
    hasher.bool(effects.may_throw);
    hasher.bool(effects.has_unbounded_stack);
}

fn hash_locality(hasher: &mut StableHasher, locality: Locality) {
    hasher.tag(match locality {
        Locality::BlockLocal => 0,
        Locality::FunctionLocal => 1,
        Locality::HeapEscaping => 2,
        Locality::Unknown => 3,
    });
}

fn hash_uniqueness(hasher: &mut StableHasher, uniqueness: Uniqueness) {
    hasher.tag(match uniqueness {
        Uniqueness::Unique => 0,
        Uniqueness::MaybeShared => 1,
        Uniqueness::Shared => 2,
    });
}

/// Fixed FNV-1a encoder used only for artifact compatibility identities.
struct StableHasher(u64);

impl StableHasher {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;

    fn new(domain: &[u8]) -> Self {
        let mut hasher = Self(Self::OFFSET);
        hasher.bytes(domain);
        hasher
    }

    fn bytes(&mut self, bytes: &[u8]) {
        self.write_raw(&u64::try_from(bytes.len()).unwrap_or(u64::MAX).to_le_bytes());
        self.write_raw(bytes);
    }

    fn write_raw(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.0 ^= u64::from(byte);
            self.0 = self.0.wrapping_mul(Self::PRIME);
        }
    }

    fn bool(&mut self, value: bool) {
        self.tag(u8::from(value));
    }

    fn tag(&mut self, value: u8) {
        self.0 ^= u64::from(value);
        self.0 = self.0.wrapping_mul(Self::PRIME);
    }

    fn u16(&mut self, value: u16) {
        self.write_raw(&value.to_le_bytes());
    }

    fn u32(&mut self, value: u32) {
        self.write_raw(&value.to_le_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.write_raw(&value.to_le_bytes());
    }

    fn usize(&mut self, value: usize) {
        self.u64(u64::try_from(value).unwrap_or(u64::MAX));
    }

    const fn finish(self) -> u64 {
        self.0
    }
}
