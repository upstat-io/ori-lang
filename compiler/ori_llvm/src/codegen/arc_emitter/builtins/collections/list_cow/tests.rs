use super::YieldReceiverStorage;

#[test]
fn yield_receiver_storage_stack_property_covers_every_mode() {
    assert!(!YieldReceiverStorage::Runtime.is_stack());
    assert!(YieldReceiverStorage::ManagedStack.is_stack());
    assert!(YieldReceiverStorage::CompactStack.is_stack());
}
