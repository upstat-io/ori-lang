//! Tests for `ErrorValue` and `TraceEntryData`.

use super::{ErrorValue, TraceEntryData};

#[test]
fn new_error_has_no_trace() {
    let err = ErrorValue::new("something failed");
    assert_eq!(err.message(), "something failed");
    assert!(err.trace().is_empty());
    assert!(!err.has_trace());
}

#[test]
fn with_trace_creates_error_with_entries() {
    let entries = vec![
        TraceEntryData {
            function: "foo".into(),
            file: "test.ori".into(),
            line: 10,
            column: 5,
        },
        TraceEntryData {
            function: "bar".into(),
            file: "test.ori".into(),
            line: 20,
            column: 3,
        },
    ];
    let err = ErrorValue::with_trace("bad", entries.clone());
    assert_eq!(err.message(), "bad");
    assert_eq!(err.trace(), &entries);
    assert!(err.has_trace());
}

#[test]
fn push_trace_appends_entry() {
    let mut err = ErrorValue::new("oops");
    err.push_trace(TraceEntryData {
        function: "main".into(),
        file: "app.ori".into(),
        line: 1,
        column: 1,
    });
    assert_eq!(err.trace().len(), 1);
    assert_eq!(err.trace()[0].function, "main");
}

#[test]
fn with_entry_returns_new_value() {
    let err = ErrorValue::new("fail");
    let entry = TraceEntryData {
        function: "helper".into(),
        file: "lib.ori".into(),
        line: 42,
        column: 8,
    };
    let err2 = err.with_entry(entry);
    // Original unchanged
    assert!(!err.has_trace());
    // New has the entry
    assert_eq!(err2.trace().len(), 1);
    assert_eq!(err2.trace()[0].function, "helper");
    assert_eq!(err2.message(), "fail");
}

#[test]
fn format_trace_empty() {
    let err = ErrorValue::new("no trace");
    assert_eq!(err.format_trace(), "");
}

#[test]
fn format_trace_with_entries() {
    let err = ErrorValue::with_trace(
        "err",
        vec![
            TraceEntryData {
                function: "parse".into(),
                file: "parser.ori".into(),
                line: 5,
                column: 12,
            },
            TraceEntryData {
                function: "run".into(),
                file: "main.ori".into(),
                line: 15,
                column: 3,
            },
        ],
    );
    let trace = err.format_trace();
    assert_eq!(trace, "parse at parser.ori:5:12\nrun at main.ori:15:3");
}

#[test]
fn display_without_trace() {
    let err = ErrorValue::new("simple error");
    assert_eq!(format!("{err}"), "simple error");
}

#[test]
fn display_with_trace() {
    let mut err = ErrorValue::new("file not found");
    err.push_trace(TraceEntryData {
        function: "read_file".into(),
        file: "io.ori".into(),
        line: 7,
        column: 4,
    });
    let display = format!("{err}");
    assert!(display.contains("file not found"));
    assert!(display.contains("read_file at io.ori:7:4"));
}

#[test]
fn trace_entry_data_format() {
    let entry = TraceEntryData {
        function: "process".into(),
        file: "worker.ori".into(),
        line: 99,
        column: 1,
    };
    assert_eq!(entry.format(), "process at worker.ori:99:1");
    assert_eq!(format!("{entry}"), "process at worker.ori:99:1");
}

#[test]
fn error_value_clone_and_eq() {
    let err1 = ErrorValue::new("test");
    let err2 = err1.clone();
    assert_eq!(err1, err2);

    let err3 = ErrorValue::new("different");
    assert_ne!(err1, err3);
}
