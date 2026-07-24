use super::derived_artifact_allows_nounwind;

#[test]
fn derived_artifact_nounwind_policy_uses_trait_metadata() {
    assert!(derived_artifact_allows_nounwind("eq$derived$0"));
    assert!(!derived_artifact_allows_nounwind("to_str$derived$1"));
    assert!(!derived_artifact_allows_nounwind("debug$derived$2"));
    assert!(derived_artifact_allows_nounwind("ordinary_function"));
}
