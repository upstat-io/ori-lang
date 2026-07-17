//! Shared ARC bodies for compiler-derived string formatting methods.
//!
//! Generated bodies contain only semantic projections, calls, and string
//! concatenation. They enter the ordinary AIMS pipeline for ownership-event
//! placement and every physical executor consumes the resulting shared plan.

use ori_ir::{BinaryOp, DerivedTrait, FormatOpen, Name, StringInterner, StructBody};
use ori_types::{FunctionSig, Idx, Pool, Tag, TypeFlags};

use crate::classify::ArcClassifier;
use crate::ir::{
    compute_var_reprs, ArcFunction, ArcParam, ArcValue, ArcVarId, LitValue, MethodCallForm, PrimOp,
};
use crate::lower::ArcIrBuilder;
use crate::Ownership;

const SELF_PARAMETER: &str = "self parameter";
const RETURN_TYPE: &str = "return type";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ConcreteTypeError {
    InvalidTypeIndex { position: &'static str, ty: Idx },
    NonConcreteType { position: &'static str, ty: Idx },
}

#[derive(Clone, Copy)]
struct FormatSpec {
    open: FormatOpen,
    separator: &'static str,
    suffix: &'static str,
    include_names: bool,
}

/// Invalid input to a compiler-derived `Printable` or `Debug` body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DerivedFormatBodyError {
    /// The requested trait does not use the string-format body shape.
    UnsupportedTrait { trait_kind: DerivedTrait },
    /// The supplied signature still has generic parameters.
    GenericSignature {
        type_parameters: usize,
        const_parameters: usize,
    },
    /// Formatting methods require exactly one named receiver parameter.
    InvalidParameterShape {
        parameter_names: usize,
        parameter_types: usize,
    },
    /// A type index does not belong to the supplied type pool.
    InvalidTypeIndex { position: &'static str, ty: Idx },
    /// A signature position contains an unresolved, generic, or poison type.
    NonConcreteType { position: &'static str, ty: Idx },
    /// Formatting methods must return `str`.
    ReturnTypeMismatch { return_type: Idx },
    /// Derived formatting is defined only for concrete products and sums.
    UnsupportedReceiverType { receiver_type: Idx, tag: Tag },
    /// A semantic owner, field, or variant name is absent from the interner.
    UnknownName { role: &'static str, name: Name },
    /// The call target does not match the requested derived trait.
    MethodNameMismatch {
        method_name: Name,
        expected: &'static str,
    },
    /// A field or variant index cannot fit the ARC IR carrier.
    IndexOverflow {
        receiver_type: Idx,
        index_kind: &'static str,
        index: usize,
    },
}

impl std::fmt::Display for DerivedFormatBodyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedTrait { trait_kind } => write!(
                formatter,
                "derived {trait_kind:?} does not use the shared string-format body"
            ),
            Self::GenericSignature {
                type_parameters,
                const_parameters,
            } => write!(
                formatter,
                "derived format body requires a concrete signature, found {type_parameters} type and {const_parameters} const parameters"
            ),
            Self::InvalidParameterShape {
                parameter_names,
                parameter_types,
            } => write!(
                formatter,
                "derived format body requires one named self parameter, found {parameter_names} names and {parameter_types} types"
            ),
            Self::InvalidTypeIndex { position, ty } => write!(
                formatter,
                "derived format body {position} has an invalid type index {ty:?}"
            ),
            Self::NonConcreteType { position, ty } => write!(
                formatter,
                "derived format body {position} must be concrete, found {ty:?}"
            ),
            Self::ReturnTypeMismatch { return_type } => write!(
                formatter,
                "derived format body must return str, found {return_type:?}"
            ),
            Self::UnsupportedReceiverType { receiver_type, tag } => write!(
                formatter,
                "derived format body requires a concrete struct or enum receiver, found {receiver_type:?} with shape {tag:?}"
            ),
            Self::UnknownName { role, name } => write!(
                formatter,
                "derived format body {role} has unknown semantic name {name:?}"
            ),
            Self::MethodNameMismatch {
                method_name,
                expected,
            } => write!(
                formatter,
                "derived format body method {method_name:?} does not match semantic method {expected}"
            ),
            Self::IndexOverflow {
                receiver_type,
                index_kind,
                index,
            } => write!(
                formatter,
                "derived format body for {receiver_type:?} has {index_kind} index {index}, which exceeds the ARC IR index range"
            ),
        }
    }
}

impl std::error::Error for DerivedFormatBodyError {}

/// Build a concrete derived `Printable` or `Debug` body.
pub fn build_derived_format(
    trait_kind: DerivedTrait,
    executable_name: Name,
    owner_name: Name,
    method_name: Name,
    signature: &FunctionSig,
    interner: &StringInterner,
    pool: &Pool,
) -> Result<ArcFunction, DerivedFormatBodyError> {
    let spec = format_spec(trait_kind)?;
    if signature.is_generic() {
        return Err(DerivedFormatBodyError::GenericSignature {
            type_parameters: signature.type_params.len(),
            const_parameters: signature.const_params.len(),
        });
    }
    if signature.param_names.len() != 1 || signature.param_types.len() != 1 {
        return Err(DerivedFormatBodyError::InvalidParameterShape {
            parameter_names: signature.param_names.len(),
            parameter_types: signature.param_types.len(),
        });
    }

    let receiver_type = signature.param_types[0];
    validate_concrete_type(pool, SELF_PARAMETER, receiver_type).map_err(map_type_error)?;
    validate_concrete_type(pool, RETURN_TYPE, signature.return_type).map_err(map_type_error)?;
    if !pool.structural_eq(signature.return_type, Idx::STR) {
        return Err(DerivedFormatBodyError::ReturnTypeMismatch {
            return_type: signature.return_type,
        });
    }
    let owner_text = lookup_name(interner, owner_name, "owner")?;
    let method_text = lookup_name(interner, method_name, "method")?;
    if method_text != trait_kind.method_name() {
        return Err(DerivedFormatBodyError::MethodNameMismatch {
            method_name,
            expected: trait_kind.method_name(),
        });
    }

    let resolved_receiver = pool.resolve_fully(receiver_type);
    let receiver_tag = pool.tag(resolved_receiver);
    let mut builder = ArcIrBuilder::new();
    let entry = builder.entry_block();
    let receiver = builder.fresh_var(receiver_type);
    match receiver_tag {
        Tag::Struct => {
            let result = emit_struct_format(
                &mut builder,
                receiver,
                resolved_receiver,
                owner_text,
                method_name,
                spec,
                interner,
                pool,
            )?;
            builder.terminate_return(result);
        }
        Tag::Enum => emit_enum_format(
            &mut builder,
            receiver,
            resolved_receiver,
            method_name,
            spec.separator,
            interner,
            pool,
        )?,
        tag => {
            return Err(DerivedFormatBodyError::UnsupportedReceiverType { receiver_type, tag });
        }
    }

    let mut function = builder.finish(
        executable_name,
        vec![ArcParam {
            var: receiver,
            ty: receiver_type,
            ownership: Ownership::Owned,
        }],
        signature.return_type,
        entry,
        false,
    );
    let classifier = ArcClassifier::new(pool);
    let representations = compute_var_reprs(&function, &classifier, pool);
    function.replace_variable_representations(representations);
    Ok(function)
}

fn format_spec(trait_kind: DerivedTrait) -> Result<FormatSpec, DerivedFormatBodyError> {
    let StructBody::FormatFields {
        open,
        separator,
        suffix,
        include_names,
    } = trait_kind.strategy().struct_body
    else {
        return Err(DerivedFormatBodyError::UnsupportedTrait { trait_kind });
    };
    Ok(FormatSpec {
        open,
        separator,
        suffix,
        include_names,
    })
}

#[expect(
    clippy::too_many_arguments,
    reason = "format emission requires shape, semantic names, strategy, and type context"
)]
fn emit_struct_format(
    builder: &mut ArcIrBuilder,
    receiver: ArcVarId,
    receiver_type: Idx,
    owner_text: &str,
    method_name: Name,
    spec: FormatSpec,
    interner: &StringInterner,
    pool: &Pool,
) -> Result<ArcVarId, DerivedFormatBodyError> {
    let prefix = match spec.open {
        FormatOpen::TypeNameParen => format!("{owner_text}("),
        FormatOpen::TypeNameBrace => format!("{owner_text} {{ "),
    };
    let mut result = emit_string_literal(builder, &prefix, interner);
    let fields = pool.struct_fields(receiver_type);
    for (field_index, (field_name, field_type)) in fields.iter().copied().enumerate() {
        let projection =
            u32::try_from(field_index).map_err(|_| DerivedFormatBodyError::IndexOverflow {
                receiver_type,
                index_kind: "field",
                index: field_index,
            })?;
        if spec.include_names {
            let field_text = lookup_name(interner, field_name, "field")?;
            let label = emit_string_literal(builder, &format!("{field_text}: "), interner);
            result = emit_concat(builder, result, label);
        }
        let field = builder.emit_project(field_type, receiver, projection, None);
        let formatted = emit_field_format(builder, field, field_type, method_name, pool);
        result = emit_concat(builder, result, formatted);
        if field_index + 1 < fields.len() {
            let separator = emit_string_literal(builder, spec.separator, interner);
            result = emit_concat(builder, result, separator);
        }
    }
    let suffix = emit_string_literal(builder, spec.suffix, interner);
    Ok(emit_concat(builder, result, suffix))
}

fn emit_enum_format(
    builder: &mut ArcIrBuilder,
    receiver: ArcVarId,
    receiver_type: Idx,
    method_name: Name,
    separator: &str,
    interner: &StringInterner,
    pool: &Pool,
) -> Result<(), DerivedFormatBodyError> {
    let tag = builder.emit_project(Idx::INT, receiver, 0, None);
    let default = builder.new_block();
    let variants = pool.enum_variants(receiver_type);
    let mut cases = Vec::with_capacity(variants.len());
    let mut arms = Vec::with_capacity(variants.len());
    for (variant_index, (variant_name, fields)) in variants.into_iter().enumerate() {
        let switch_index =
            u64::try_from(variant_index).map_err(|_| DerivedFormatBodyError::IndexOverflow {
                receiver_type,
                index_kind: "variant",
                index: variant_index,
            })?;
        let block = builder.new_block();
        cases.push((switch_index, block));
        arms.push((block, variant_name, fields));
    }
    builder.terminate_switch(tag, cases, default);

    for (block, variant_name, fields) in arms {
        builder.position_at(block);
        let variant_text = lookup_name(interner, variant_name, "variant")?;
        let result = if fields.is_empty() {
            emit_string_literal(builder, variant_text, interner)
        } else {
            emit_enum_payload(
                builder,
                receiver,
                receiver_type,
                variant_text,
                &fields,
                method_name,
                separator,
                interner,
                pool,
            )?
        };
        builder.terminate_return(result);
    }
    builder.position_at(default);
    builder.terminate_unreachable();
    Ok(())
}

#[expect(
    clippy::too_many_arguments,
    reason = "payload emission requires receiver, variant, method, formatting, and type context"
)]
fn emit_enum_payload(
    builder: &mut ArcIrBuilder,
    receiver: ArcVarId,
    receiver_type: Idx,
    variant_text: &str,
    fields: &[Idx],
    method_name: Name,
    separator: &str,
    interner: &StringInterner,
    pool: &Pool,
) -> Result<ArcVarId, DerivedFormatBodyError> {
    let mut result = emit_string_literal(builder, &format!("{variant_text}("), interner);
    for (field_index, field_type) in fields.iter().copied().enumerate() {
        let offset =
            u32::try_from(field_index).map_err(|_| DerivedFormatBodyError::IndexOverflow {
                receiver_type,
                index_kind: "field",
                index: field_index,
            })?;
        let projection =
            1_u32
                .checked_add(offset)
                .ok_or(DerivedFormatBodyError::IndexOverflow {
                    receiver_type,
                    index_kind: "field",
                    index: field_index,
                })?;
        let field = builder.emit_project(field_type, receiver, projection, None);
        let formatted = emit_field_format(builder, field, field_type, method_name, pool);
        result = emit_concat(builder, result, formatted);
        if field_index + 1 < fields.len() {
            let separator = emit_string_literal(builder, separator, interner);
            result = emit_concat(builder, result, separator);
        }
    }
    let suffix = emit_string_literal(builder, ")", interner);
    Ok(emit_concat(builder, result, suffix))
}

fn emit_field_format(
    builder: &mut ArcIrBuilder,
    field: ArcVarId,
    field_type: Idx,
    method_name: Name,
    pool: &Pool,
) -> ArcVarId {
    let result = if pool
        .builtin_type_tag(pool.resolve_fully(field_type))
        .is_some()
    {
        builder.emit_apply(Idx::STR, method_name, vec![field], None, None)
    } else {
        builder.emit_invoke(Idx::STR, method_name, vec![field], None, None)
    };
    builder.note_method_call(result, field_type, MethodCallForm::Instance);
    result
}

fn emit_string_literal(
    builder: &mut ArcIrBuilder,
    text: &str,
    interner: &StringInterner,
) -> ArcVarId {
    builder.emit_let(
        Idx::STR,
        ArcValue::Literal(LitValue::String(interner.intern(text))),
        None,
    )
}

fn emit_concat(builder: &mut ArcIrBuilder, left: ArcVarId, right: ArcVarId) -> ArcVarId {
    builder.emit_let(
        Idx::STR,
        ArcValue::PrimOp {
            op: PrimOp::Binary(BinaryOp::Add),
            args: vec![left, right],
        },
        None,
    )
}

fn lookup_name<'a>(
    interner: &'a StringInterner,
    name: Name,
    role: &'static str,
) -> Result<&'a str, DerivedFormatBodyError> {
    interner
        .try_lookup(name)
        .ok_or(DerivedFormatBodyError::UnknownName { role, name })
}

fn map_type_error(error: ConcreteTypeError) -> DerivedFormatBodyError {
    match error {
        ConcreteTypeError::InvalidTypeIndex { position, ty } => {
            DerivedFormatBodyError::InvalidTypeIndex { position, ty }
        }
        ConcreteTypeError::NonConcreteType { position, ty } => {
            DerivedFormatBodyError::NonConcreteType { position, ty }
        }
    }
}

fn validate_concrete_type(
    pool: &Pool,
    position: &'static str,
    ty: Idx,
) -> Result<(), ConcreteTypeError> {
    if !pool.is_valid_idx(ty) {
        return Err(ConcreteTypeError::InvalidTypeIndex { position, ty });
    }
    let resolved = pool.resolve_fully(ty);
    if !pool.is_valid_idx(resolved) {
        return Err(ConcreteTypeError::InvalidTypeIndex {
            position,
            ty: resolved,
        });
    }

    let flags = pool.flags(resolved);
    let unresolved = TypeFlags::HAS_SELF | TypeFlags::HAS_PROJECTION;
    if !flags.is_recordable()
        || flags.intersects(unresolved)
        || matches!(pool.tag(resolved), Tag::Scheme | Tag::ModuleNs)
    {
        return Err(ConcreteTypeError::NonConcreteType { position, ty });
    }
    Ok(())
}

#[cfg(test)]
mod tests;
