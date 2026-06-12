use std::time::Duration;

use super::super::result::TestOutcome;
use super::{escape, parse_line, unescape, WorkerEvent, PROTOCOL_MARKER};

/// Fixed expected token for these pins (a real run uses a per-spawn nonce).
const TOKEN: &str = "tok-42";

/// Parse `rest` as the payload of a correctly-tokenized protocol line.
fn parse_tokenized(rest: &str) -> Option<WorkerEvent> {
    parse_line(&format!("{PROTOCOL_MARKER}:{TOKEN} {rest}"), TOKEN)
}

// Escape / unescape roundtrip

#[test]
fn test_escape_roundtrip_preserves_newlines_and_backslashes() {
    let original = "line one\nline two\r\nback\\slash \\n literal";
    assert_eq!(unescape(&escape(original)), original);
}

#[test]
fn test_escape_produces_single_line() {
    let escaped = escape("a\nb\rc");
    assert!(!escaped.contains('\n'));
    assert!(!escaped.contains('\r'));
}

#[test]
fn test_unescape_preserves_unknown_escape_verbatim() {
    assert_eq!(unescape("a\\qb"), "a\\qb");
    assert_eq!(unescape("trailing\\"), "trailing\\");
}

// Event parsing

#[test]
fn test_parse_plan_line_returns_plan_event() {
    assert_eq!(
        parse_tokenized("plan my_test"),
        Some(WorkerEvent::Plan {
            name: "my_test".to_string()
        })
    );
}

#[test]
fn test_parse_start_line_returns_start_event() {
    assert_eq!(
        parse_tokenized("start my_test"),
        Some(WorkerEvent::Start {
            name: "my_test".to_string()
        })
    );
}

#[test]
fn test_parse_result_pass_carries_duration() {
    assert_eq!(
        parse_tokenized("result t1 pass 1500"),
        Some(WorkerEvent::Result {
            name: "t1".to_string(),
            outcome: TestOutcome::Passed,
            duration: Duration::from_nanos(1500),
        })
    );
}

#[test]
fn test_parse_result_fail_unescapes_multiline_detail() {
    assert_eq!(
        parse_tokenized("result t1 fail 7 assertion failed\\nleft: 1"),
        Some(WorkerEvent::Result {
            name: "t1".to_string(),
            outcome: TestOutcome::Failed("assertion failed\nleft: 1".to_string()),
            duration: Duration::from_nanos(7),
        })
    );
}

#[test]
fn test_parse_result_skip_carries_reason() {
    assert_eq!(
        parse_tokenized("result t1 skip 0 not ready"),
        Some(WorkerEvent::Result {
            name: "t1".to_string(),
            outcome: TestOutcome::Skipped("not ready".to_string()),
            duration: Duration::ZERO,
        })
    );
}

#[test]
fn test_parse_result_skip_unchanged() {
    assert_eq!(
        parse_tokenized("result t1 skip-unchanged 0"),
        Some(WorkerEvent::Result {
            name: "t1".to_string(),
            outcome: TestOutcome::SkippedUnchanged,
            duration: Duration::ZERO,
        })
    );
}

#[test]
fn test_parse_result_llvm_compile_fail_carries_detail() {
    assert_eq!(
        parse_tokenized("result t1 llvm-compile-fail 0 codegen error"),
        Some(WorkerEvent::Result {
            name: "t1".to_string(),
            outcome: TestOutcome::LlvmCompileFail("codegen error".to_string()),
            duration: Duration::ZERO,
        })
    );
}

#[test]
fn test_parse_file_error_unescapes_message() {
    assert_eq!(
        parse_tokenized("file-error type error\\nat line 3"),
        Some(WorkerEvent::FileError {
            message: "type error\nat line 3".to_string()
        })
    );
}

#[test]
fn test_parse_llvm_compile_error_flag() {
    assert_eq!(
        parse_tokenized("llvm-compile-error"),
        Some(WorkerEvent::LlvmCompileError)
    );
}

#[test]
fn test_parse_done() {
    assert_eq!(parse_tokenized("done"), Some(WorkerEvent::Done));
}

// Emit -> parse roundtrip via formatted strings

#[test]
fn test_result_format_roundtrip_for_every_outcome() {
    let outcomes = vec![
        TestOutcome::Passed,
        TestOutcome::Failed("multi\nline \\ detail".to_string()),
        TestOutcome::Skipped("reason with spaces".to_string()),
        TestOutcome::SkippedUnchanged,
        TestOutcome::LlvmCompileFail("LLVM compilation failed: bad IR".to_string()),
    ];
    let mut visited = 0;
    for outcome in outcomes {
        let nanos: u64 = 42;
        let prefix = format!("{PROTOCOL_MARKER}:{TOKEN} ");
        let line = match &outcome {
            TestOutcome::Passed => format!("{prefix}result t pass {nanos}"),
            TestOutcome::Failed(d) => {
                format!("{prefix}result t fail {nanos} {}", escape(d))
            }
            TestOutcome::Skipped(d) => {
                format!("{prefix}result t skip {nanos} {}", escape(d))
            }
            TestOutcome::SkippedUnchanged => {
                format!("{prefix}result t skip-unchanged {nanos}")
            }
            TestOutcome::LlvmCompileFail(d) => {
                format!("{prefix}result t llvm-compile-fail {nanos} {}", escape(d))
            }
        };
        let parsed = parse_line(&line, TOKEN);
        assert_eq!(
            parsed,
            Some(WorkerEvent::Result {
                name: "t".to_string(),
                outcome: outcome.clone(),
                duration: Duration::from_nanos(nanos),
            }),
            "roundtrip failed for line: {line}"
        );
        visited += 1;
    }
    assert_eq!(visited, 5);
}

// Malformed / non-protocol lines

#[test]
fn test_parse_non_protocol_line_returns_none() {
    assert_eq!(parse_line("hello from a test program", TOKEN), None);
    assert_eq!(parse_line("", TOKEN), None);
    assert_eq!(parse_line("ori: FATAL — double free", TOKEN), None);
}

#[test]
fn test_parse_unknown_protocol_kind_returns_none() {
    assert_eq!(parse_tokenized("bogus payload"), None);
}

#[test]
fn test_parse_malformed_result_returns_none() {
    // Missing fields.
    assert_eq!(parse_tokenized("result"), None);
    assert_eq!(parse_tokenized("result t1"), None);
    // Bad nanos.
    assert_eq!(parse_tokenized("result t1 pass abc"), None);
    // Unknown outcome kind.
    assert_eq!(parse_tokenized("result t1 exploded 5"), None);
    // pass must not carry a detail payload.
    assert_eq!(parse_tokenized("result t1 pass 5 extra"), None);
}

#[test]
fn test_parse_malformed_plan_and_start_return_none() {
    assert_eq!(parse_tokenized("plan"), None);
    assert_eq!(parse_tokenized("plan two names"), None);
    assert_eq!(parse_tokenized("start"), None);
}

#[test]
fn test_parse_done_with_trailing_payload_returns_none() {
    assert_eq!(parse_tokenized("done extra"), None);
    assert_eq!(parse_tokenized("llvm-compile-error extra"), None);
}

// Token forgery guard

#[test]
fn test_parse_line_without_token_returns_none() {
    // The pre-token wire shape: marker + space, no `:<token>`.
    assert_eq!(parse_line("@@ori-test result evil pass 0", TOKEN), None);
    assert_eq!(parse_line("@@ori-test plan evil", TOKEN), None);
    assert_eq!(parse_line("@@ori-test done", TOKEN), None);
}

#[test]
fn test_parse_line_with_wrong_token_returns_none() {
    assert_eq!(
        parse_line("@@ori-test:wrong result evil pass 0", TOKEN),
        None
    );
    assert_eq!(parse_line("@@ori-test: result evil pass 0", TOKEN), None);
}

#[test]
fn test_parse_line_with_token_prefix_extension_returns_none() {
    // A forged token that merely STARTS with the real token must not match.
    assert_eq!(
        parse_line(
            &format!("{PROTOCOL_MARKER}:{TOKEN}x result evil pass 0"),
            TOKEN
        ),
        None
    );
}

#[test]
fn test_parse_line_with_empty_expected_token_never_matches() {
    assert_eq!(parse_line("@@ori-test: result evil pass 0", ""), None);
    assert_eq!(parse_line("@@ori-test result evil pass 0", ""), None);
}
