//! Production-path pins for the summary CLI: the REAL binary on REAL files.
//!
//! `print_summary` lives in `main.rs` and is unreachable from a library unit
//! test, so its output could drift from `ingest`'s semantics without any suite
//! noticing. That is exactly what happened once: the headerless branch kept
//! describing an unversioned dev stream after `ingest` stopped admitting one.

use std::io::Write;
use std::process::Command;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_ori-rc-remarks")
}

fn write_stream(name: &str, body: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!("ori-rc-remarks-{name}-{}.jsonl", std::process::id()));
    let mut f = std::fs::File::create(&path).unwrap_or_else(|e| panic!("create {name}: {e}"));
    f.write_all(body.as_bytes())
        .unwrap_or_else(|e| panic!("write {name}: {e}"));
    path
}

#[test]
fn blank_input_reports_an_empty_stream_and_succeeds() {
    // The ONLY headerless stream ingest can return is one with no records, so
    // the summary must say that and nothing about an assumed schema version.
    let path = write_stream("blank", "");
    let out = Command::new(bin())
        .arg(&path)
        .output()
        .unwrap_or_else(|e| panic!("spawn: {e}"));
    let _ = std::fs::remove_file(&path);
    assert!(out.status.success(), "blank input must succeed: {out:?}");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("empty stream (no header, no remarks)"),
        "summary must name the empty state; got: {stdout}"
    );
    assert!(
        !stdout.contains("schema"),
        "an empty stream claims no schema version; got: {stdout}"
    );
}

#[test]
fn a_remark_without_a_header_is_refused_by_the_binary() {
    // The negative half: the refusal must reach a CLI user with the fix, not
    // just a library caller.
    let remark = r#"{"kind":"missed","pass":"p","name":"n","rc_op":"burden_inc","function":"f","debug_loc":null,"ssa_value":1,"exit_block":null,"cause":null,"burden_net":null,"args":[],"cow_mode":null}"#;
    let path = write_stream("bare", remark);
    let out = Command::new(bin())
        .arg(&path)
        .output()
        .unwrap_or_else(|e| panic!("spawn: {e}"));
    let _ = std::fs::remove_file(&path);
    assert!(!out.status.success(), "an unversioned remark must not succeed");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("--emit-rc-remarks"),
        "refusal must name the regeneration command; got: {stderr}"
    );
}
