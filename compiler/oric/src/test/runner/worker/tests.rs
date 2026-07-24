//! Pins for the worker-side protocol-token scrub.
//!
//! The token must be gone from the worker's environment before any test
//! code executes — JIT'd test code (and its children) seeing the token
//! could forge protocol records. Env vars are process globals; every test
//! here holds `ENV_LOCK`.

use super::take_protocol_token;
use crate::test::runner::ENV_LOCK;

const TOKEN_VAR: &str = crate::debug_flags::ORI_TEST_PROTOCOL_TOKEN;

#[test]
fn test_take_protocol_token_returns_token_and_scrubs_env() {
    let _guard = ENV_LOCK.lock();
    std::env::set_var(TOKEN_VAR, "feedface00000000feedface00000000");
    let token = take_protocol_token();
    assert_eq!(token.as_deref(), Some("feedface00000000feedface00000000"));
    assert!(
        std::env::var(TOKEN_VAR).is_err(),
        "token must be scrubbed from the worker env before any test code runs"
    );
}

#[test]
fn test_take_protocol_token_scrubs_empty_token_and_returns_none() {
    let _guard = ENV_LOCK.lock();
    std::env::set_var(TOKEN_VAR, "");
    assert_eq!(take_protocol_token(), None);
    assert!(
        std::env::var(TOKEN_VAR).is_err(),
        "even an empty token must be scrubbed"
    );
}

#[test]
fn test_take_protocol_token_absent_returns_none() {
    let _guard = ENV_LOCK.lock();
    std::env::remove_var(TOKEN_VAR);
    assert_eq!(take_protocol_token(), None);
    assert!(std::env::var(TOKEN_VAR).is_err());
}
