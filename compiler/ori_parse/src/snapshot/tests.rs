use super::*;

#[test]
fn test_snapshot_size() {
    // Verify snapshot is lightweight
    assert!(
        std::mem::size_of::<ParserSnapshot>() <= 24,
        "ParserSnapshot should be small (got {} bytes)",
        std::mem::size_of::<ParserSnapshot>()
    );
}

#[test]
fn test_snapshot_creation() {
    let snapshot = ParserSnapshot::new(42, ParseContext::IN_LOOP);
    assert_eq!(snapshot.cursor_pos, 42);
    assert!(snapshot.context.in_loop());
}

#[test]
fn test_snapshot_copy() {
    let snapshot1 = ParserSnapshot::new(10, ParseContext::IN_TYPE);
    let snapshot2 = snapshot1; // Copy
    assert_eq!(snapshot1.cursor_pos, snapshot2.cursor_pos);
    assert_eq!(snapshot1.context, snapshot2.context);
}
