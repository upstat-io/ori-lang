use super::require_protocol_result;

#[test]
#[should_panic(expected = "verify its receiver type and result layout")]
fn missing_protocol_result_fails_loudly() {
    require_protocol_result::<()>("__index", None);
}
