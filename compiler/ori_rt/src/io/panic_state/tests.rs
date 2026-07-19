use super::{
    default_report_message, mark_panic_reported_by_handler, reset_panic_state, store_panic,
    take_panic_message,
};

#[test]
fn stored_panic_is_reportable_until_catch_consumes_it() {
    reset_panic_state();
    store_panic("caught-message");

    assert_eq!(
        default_report_message().as_deref(),
        Some("caught-message"),
        "a newly stored panic remains eligible for default boundary reporting"
    );
    assert_eq!(
        take_panic_message().as_deref(),
        Some("caught-message"),
        "catch recovery consumes the stored panic message"
    );
    assert_eq!(
        default_report_message(),
        None,
        "a caught panic must not remain eligible for boundary reporting"
    );
}

#[test]
fn user_handler_suppresses_only_default_boundary_report() {
    reset_panic_state();
    store_panic("handled-message");
    mark_panic_reported_by_handler();

    assert_eq!(
        default_report_message(),
        None,
        "a user panic handler replaces the runtime's default report"
    );
    assert_eq!(
        take_panic_message().as_deref(),
        Some("handled-message"),
        "handler reporting must not erase the message needed by catch recovery"
    );
}
