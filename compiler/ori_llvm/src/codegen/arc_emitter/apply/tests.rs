use super::closed_target_projection_message;

#[test]
fn closed_target_diagnostic_states_cause_and_action() {
    let message = closed_target_projection_message("clone$derived$7", "apply");
    assert!(message.contains("did not declare closed executable target"));
    assert!(message.contains("ORI_VERIFY_ARC=1"));
    assert!(message.contains("report this compiler bug"));
    assert!(!message.contains("missing mono instance"));
}
