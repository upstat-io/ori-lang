//! Validation and merging for user-type representation attributes.

use ori_ir::ReprAttrKind;

use crate::{ModuleChecker, TypeCheckError};

pub(super) fn validate_and_merge_repr_attrs(
    checker: &mut ModuleChecker<'_>,
    decl: &ori_ir::TypeDecl,
) -> Option<ReprAttrKind> {
    if decl.repr_attrs.is_empty() {
        return None;
    }

    let is_struct = matches!(decl.kind, ori_ir::TypeDeclKind::Struct(_));
    let is_newtype = matches!(decl.kind, ori_ir::TypeDeclKind::Newtype(_));
    let mut valid_attrs = Vec::new();
    for attr in &decl.repr_attrs {
        match attr {
            ReprAttrKind::Transparent => {
                if let ori_ir::TypeDeclKind::Struct(fields) = &decl.kind {
                    if fields.is_empty() {
                        checker.push_error(TypeCheckError::invalid_repr_attribute(
                            decl.span,
                            decl.name,
                            "`#repr(\"transparent\")` requires exactly one field, but this struct has none",
                        ));
                        continue;
                    }
                    if fields.len() > 1 {
                        checker.push_error(TypeCheckError::invalid_repr_attribute(
                            decl.span,
                            decl.name,
                            format!(
                                "`#repr(\"transparent\")` requires exactly one field, but this struct has {}",
                                fields.len()
                            ),
                        ));
                        continue;
                    }
                    valid_attrs.push(*attr);
                } else if is_newtype {
                    checker.push_error(TypeCheckError::invalid_repr_attribute(
                        decl.span,
                        decl.name,
                        "`#repr(\"transparent\")` can only be applied to structs, not newtypes (newtypes are implicitly transparent)",
                    ));
                } else {
                    checker.push_error(TypeCheckError::invalid_repr_attribute(
                        decl.span,
                        decl.name,
                        "`#repr(\"transparent\")` can only be applied to structs, not sum types",
                    ));
                }
            }
            ReprAttrKind::Aligned(alignment) => {
                if *alignment == 0 {
                    checker.push_error(TypeCheckError::invalid_repr_attribute(
                        decl.span,
                        decl.name,
                        "alignment must be greater than zero",
                    ));
                    continue;
                }
                if !alignment.is_power_of_two() {
                    checker.push_error(TypeCheckError::invalid_repr_attribute(
                        decl.span,
                        decl.name,
                        format!("alignment must be a power of two, but got {alignment}"),
                    ));
                    continue;
                }
                if !is_struct {
                    checker.push_error(TypeCheckError::invalid_repr_attribute(
                        decl.span,
                        decl.name,
                        "`#repr(\"aligned\", N)` can only be applied to structs, not newtypes or sum types",
                    ));
                    continue;
                }
                valid_attrs.push(*attr);
            }
            ReprAttrKind::C => {
                if !is_struct {
                    checker.push_error(TypeCheckError::invalid_repr_attribute(
                        decl.span,
                        decl.name,
                        "`#repr(\"c\")` can only be applied to structs, not newtypes or sum types",
                    ));
                    continue;
                }
                valid_attrs.push(*attr);
            }
            ReprAttrKind::Packed => {
                if !is_struct {
                    checker.push_error(TypeCheckError::invalid_repr_attribute(
                        decl.span,
                        decl.name,
                        "`#repr(\"packed\")` can only be applied to structs, not newtypes or sum types",
                    ));
                    continue;
                }
                valid_attrs.push(*attr);
            }
            ReprAttrKind::CAligned(_) => valid_attrs.push(*attr),
        }
    }

    merge_repr_attrs(checker, decl, &valid_attrs)
}

fn merge_repr_attrs(
    checker: &mut ModuleChecker<'_>,
    decl: &ori_ir::TypeDecl,
    valid_attrs: &[ReprAttrKind],
) -> Option<ReprAttrKind> {
    if valid_attrs.is_empty() {
        return None;
    }
    if valid_attrs.len() == 1 {
        return Some(valid_attrs[0]);
    }

    let has_c = valid_attrs
        .iter()
        .any(|attr| matches!(attr, ReprAttrKind::C));
    let has_packed = valid_attrs
        .iter()
        .any(|attr| matches!(attr, ReprAttrKind::Packed));
    let has_transparent = valid_attrs
        .iter()
        .any(|attr| matches!(attr, ReprAttrKind::Transparent));
    let aligned = valid_attrs.iter().find_map(|attr| match attr {
        ReprAttrKind::Aligned(alignment) => Some(*alignment),
        _ => None,
    });

    if has_packed && aligned.is_some() {
        checker.push_error(TypeCheckError::invalid_repr_attribute(
            decl.span,
            decl.name,
            "cannot combine `#repr(\"packed\")` and `#repr(\"aligned\", N)` — they are contradictory",
        ));
        return None;
    }
    if has_c && has_packed {
        checker.push_error(TypeCheckError::invalid_repr_attribute(
            decl.span,
            decl.name,
            "cannot combine `#repr(\"c\")` and `#repr(\"packed\")` — use `#repr(\"c\")` with explicit padding",
        ));
        return None;
    }
    if has_transparent && valid_attrs.len() > 1 {
        checker.push_error(TypeCheckError::invalid_repr_attribute(
            decl.span,
            decl.name,
            "`#repr(\"transparent\")` cannot be combined with other repr attributes",
        ));
        return None;
    }
    if let (true, Some(alignment)) = (has_c, aligned) {
        if valid_attrs.len() > 2 {
            checker.push_error(TypeCheckError::invalid_repr_attribute(
                decl.span,
                decl.name,
                "only `#repr(\"c\")` + `#repr(\"aligned\", N)` is a valid combination — extra `#repr` attributes are not permitted",
            ));
            return None;
        }
        return Some(ReprAttrKind::CAligned(alignment));
    }

    checker.push_error(TypeCheckError::invalid_repr_attribute(
        decl.span,
        decl.name,
        "duplicate `#repr` attributes are not permitted — each `#repr` kind may only appear once",
    ));
    None
}
