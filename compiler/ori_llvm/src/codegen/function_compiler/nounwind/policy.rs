//! Nounwind policy for compiler-generated derived artifacts.

pub(super) fn derived_artifact_allows_nounwind(name: &str) -> bool {
    ori_ir::DerivedTrait::from_executable_body_name(name)
        .is_none_or(|(trait_kind, _)| trait_kind.is_nounwind_derived())
}

#[cfg(test)]
mod tests;
