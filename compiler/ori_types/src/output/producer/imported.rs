//! Stable identities for producer-module implementation methods.

use ori_ir::{ExprArena, ImplMethod, Name, ParsedType, StringInterner, WhereClause};

use super::MethodProducer;

/// Schema of stable cross-module impl-method producer identities.
///
/// Increment this whenever the symbol coordinates or signature fingerprint
/// below change. The schema lives in the symbol itself, so stale cached
/// identities fail closed instead of aliasing a newly-shaped method.
pub const IMPORTED_METHOD_PRODUCER_SCHEMA: u8 = 1;

/// Build the stable exported identity for one parsed impl method.
///
/// `module_identity` is supplied by the import resolver (normally the resolved
/// source path). Impl and method ordinals are source-template coordinates, not
/// importer-local registry indices. The signature fingerprint is independent
/// of arena `ExprId`s and source spans.
#[must_use]
pub fn imported_method_producer(
    module_identity: &str,
    impl_index: usize,
    method_index: usize,
    method: &ImplMethod,
    arena: &ExprArena,
    interner: &StringInterner,
) -> MethodProducer {
    let method_name = interner
        .try_lookup(method.name)
        .unwrap_or("<unresolved-method>");
    let symbol = format!(
        "ori.imported.impl.v{IMPORTED_METHOD_PRODUCER_SCHEMA}:{}:{module_identity}:{impl_index}:{method_index}:{method_name}",
        module_identity.len(),
    )
    .into_boxed_str();
    let signature_hash = imported_method_signature_hash(method, arena, interner);
    MethodProducer::Imported {
        symbol,
        signature_hash,
    }
}

/// Stable source-signature fingerprint for an imported impl method.
///
/// This deliberately hashes only signature-bearing syntax. In particular it
/// excludes body ids and spans, both of which are arena-local, while retaining
/// generic bounds, parameter defaults' presence, capabilities, and where
/// constraints. Names are hashed by text rather than interner coordinates.
#[must_use]
pub fn imported_method_signature_hash(
    method: &ImplMethod,
    arena: &ExprArena,
    interner: &StringInterner,
) -> u64 {
    let mut hash = StableMethodHasher::new(b"ori.imported.method.signature.v1");
    hash.name(method.name, interner);

    let generics = arena.get_generic_params(method.generics);
    hash.usize(generics.len());
    for generic in generics {
        hash.name(generic.name, interner);
        hash.bool(generic.is_const);
        hash.usize(generic.bounds.len());
        for bound in &generic.bounds {
            hash.usize(bound.path().len());
            for segment in bound.path() {
                hash.name(segment, interner);
            }
        }
        hash.optional_type(generic.default_type.as_ref(), arena, interner);
        hash.optional_type(generic.const_type.as_ref(), arena, interner);
        hash.bool(generic.default_value.is_some());
    }

    let params = arena.get_params(method.params);
    hash.usize(params.len());
    for param in params {
        hash.name(param.name, interner);
        hash.optional_type(param.ty.as_ref(), arena, interner);
        hash.bool(param.default.is_some());
        hash.bool(param.is_variadic);
    }
    hash.parsed_type(&method.return_ty, arena, interner);

    hash.usize(method.capabilities.len());
    for capability in &method.capabilities {
        hash.name(capability.name, interner);
    }
    hash.usize(method.where_clauses.len());
    for clause in &method.where_clauses {
        match clause {
            WhereClause::TypeBound {
                param,
                projection,
                bounds,
                ..
            } => {
                hash.tag(0);
                hash.name(*param, interner);
                hash.bool(projection.is_some());
                if let Some(projection) = projection {
                    hash.name(*projection, interner);
                }
                hash.usize(bounds.len());
                for bound in bounds {
                    let path = bound.path();
                    hash.usize(path.len());
                    for segment in path {
                        hash.name(segment, interner);
                    }
                }
            }
            WhereClause::ConstBound { .. } => {
                // Const-expression ids are arena-local. The current type system
                // does not evaluate const bounds, but their presence remains a
                // signature coordinate and therefore participates in identity.
                hash.tag(1);
            }
        }
    }
    hash.finish()
}

struct StableMethodHasher(u64);

impl StableMethodHasher {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;

    fn new(domain: &[u8]) -> Self {
        let mut hash = Self(Self::OFFSET);
        hash.bytes(domain);
        hash
    }

    fn parsed_type(&mut self, ty: &ParsedType, arena: &ExprArena, interner: &StringInterner) {
        match ty {
            ParsedType::Primitive(id) => {
                self.tag(0);
                self.u64(u64::from(id.raw()));
            }
            ParsedType::Named { name, type_args } => {
                self.tag(1);
                self.name(*name, interner);
                self.type_range(*type_args, arena, interner);
            }
            ParsedType::List(element) => {
                self.tag(2);
                self.parsed_type(arena.get_parsed_type(*element), arena, interner);
            }
            ParsedType::FixedList { elem, .. } => {
                self.tag(3);
                self.parsed_type(arena.get_parsed_type(*elem), arena, interner);
            }
            ParsedType::Tuple(elements) => {
                self.tag(4);
                self.type_range(*elements, arena, interner);
            }
            ParsedType::Function { params, ret } => {
                self.tag(5);
                self.type_range(*params, arena, interner);
                self.parsed_type(arena.get_parsed_type(*ret), arena, interner);
            }
            ParsedType::Map { key, value } => {
                self.tag(6);
                self.parsed_type(arena.get_parsed_type(*key), arena, interner);
                self.parsed_type(arena.get_parsed_type(*value), arena, interner);
            }
            ParsedType::Infer => self.tag(7),
            ParsedType::SelfType => self.tag(8),
            ParsedType::AssociatedType {
                base,
                assoc_name,
                type_args,
            } => {
                self.tag(9);
                self.parsed_type(arena.get_parsed_type(*base), arena, interner);
                self.name(*assoc_name, interner);
                self.type_range(*type_args, arena, interner);
            }
            ParsedType::ConstExpr(_) => self.tag(10),
            ParsedType::TraitBounds(bounds) => {
                self.tag(11);
                self.type_range(*bounds, arena, interner);
            }
        }
    }

    fn optional_type(
        &mut self,
        ty: Option<&ParsedType>,
        arena: &ExprArena,
        interner: &StringInterner,
    ) {
        self.bool(ty.is_some());
        if let Some(ty) = ty {
            self.parsed_type(ty, arena, interner);
        }
    }

    fn type_range(
        &mut self,
        range: ori_ir::ParsedTypeRange,
        arena: &ExprArena,
        interner: &StringInterner,
    ) {
        let types = arena.get_parsed_type_list(range);
        self.usize(types.len());
        for &id in types {
            self.parsed_type(arena.get_parsed_type(id), arena, interner);
        }
    }

    fn name(&mut self, name: Name, interner: &StringInterner) {
        self.bytes(
            interner
                .try_lookup(name)
                .unwrap_or("<unresolved-name>")
                .as_bytes(),
        );
    }

    fn bytes(&mut self, bytes: &[u8]) {
        self.usize(bytes.len());
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

    fn u64(&mut self, value: u64) {
        for byte in value.to_le_bytes() {
            self.tag(byte);
        }
    }

    fn usize(&mut self, value: usize) {
        self.u64(u64::try_from(value).unwrap_or(u64::MAX));
    }

    const fn finish(self) -> u64 {
        self.0
    }
}
