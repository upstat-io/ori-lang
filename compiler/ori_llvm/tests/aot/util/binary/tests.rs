//! Unit pins for the per-process artifact snapshot staging.
//!
//! The staging core exists so the AOT suite resolves immutable per-run
//! snapshots of the compiler binary + runtime staticlib instead of the
//! shared mutable `target/<profile>/` paths a concurrent build can swap
//! mid-suite (mass bogus failures otherwise).

use super::{
    artifact_identity_of, publish_stage_manifest, render_stage_manifest, stage_snapshot,
    SnapshotStrategy, StagedArtifact, STAGE_MANIFEST_NAME,
};
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
use std::path::Path;

fn write_file(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            panic!("test setup: create_dir_all {}: {e}", parent.display());
        }
    }
    if let Err(e) = fs::write(path, content) {
        panic!("test setup: write {}: {e}", path.display());
    }
}

/// Replace `path` the way cargo/rustc do: write a temp sibling, then rename(2)
/// over the destination. A hardlink to the OLD inode must be unaffected.
fn replace_via_rename(path: &Path, new_content: &str) {
    let tmp = path.with_extension("tmp-replace");
    write_file(&tmp, new_content);
    if let Err(e) = fs::rename(&tmp, path) {
        panic!("test setup: rename over {}: {e}", path.display());
    }
}

fn read_to_string_required(path: &Path) -> String {
    match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => panic!("read {}: {e}", path.display()),
    }
}

fn temp_test_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("ori-stage-pin-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    if let Err(e) = fs::create_dir_all(&dir) {
        panic!("test setup: create {}: {e}", dir.display());
    }
    dir
}

#[test]
fn stage_snapshot_creates_dir_with_required_files() {
    let root = temp_test_dir("create");
    let (src, stage) = (root.join("src"), root.join("stage"));
    write_file(&src.join("ori"), "binary-v1");
    write_file(&src.join("libori_rt.a"), "lib-v1");

    let r = stage_snapshot(
        &src,
        &stage,
        &["ori", "libori_rt.a"],
        &[],
        SnapshotStrategy::HardLink,
    );
    if let Err(e) = r {
        panic!("stage_snapshot failed: {e}");
    }
    assert!(stage.join("ori").exists());
    assert!(stage.join("libori_rt.a").exists());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn staged_content_survives_rename_replacement_of_source() {
    let root = temp_test_dir("survive-link");
    let (src, stage) = (root.join("src"), root.join("stage"));
    write_file(&src.join("ori"), "v1");

    let r = stage_snapshot(&src, &stage, &["ori"], &[], SnapshotStrategy::HardLink);
    if let Err(e) = r {
        panic!("stage_snapshot failed: {e}");
    }
    replace_via_rename(&src.join("ori"), "v2-swapped-by-concurrent-build");

    // The snapshot pins the ORIGINAL inode; the rename-based swap must not
    // reach it (this is the cure for the mass bogus-failure class).
    assert_eq!(read_to_string_required(&stage.join("ori")), "v1");
    // Sanity inverse: the source really did change (the pin detects a real swap).
    assert_eq!(
        read_to_string_required(&src.join("ori")),
        "v2-swapped-by-concurrent-build"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn staged_content_survives_replacement_under_copy_strategy() {
    let root = temp_test_dir("survive-copy");
    let (src, stage) = (root.join("src"), root.join("stage"));
    write_file(&src.join("ori"), "v1");

    let r = stage_snapshot(&src, &stage, &["ori"], &[], SnapshotStrategy::Copy);
    if let Err(e) = r {
        panic!("stage_snapshot failed: {e}");
    }
    assert_eq!(read_to_string_required(&stage.join("ori")), "v1");

    replace_via_rename(&src.join("ori"), "v2");
    assert_eq!(read_to_string_required(&stage.join("ori")), "v1");
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn restage_is_idempotent_and_reflects_current_source() {
    let root = temp_test_dir("idem");
    let (src, stage) = (root.join("src"), root.join("stage"));
    write_file(&src.join("ori"), "v1");

    if let Err(e) = stage_snapshot(&src, &stage, &["ori"], &[], SnapshotStrategy::HardLink) {
        panic!("first stage failed: {e}");
    }
    replace_via_rename(&src.join("ori"), "v2");
    if let Err(e) = stage_snapshot(&src, &stage, &["ori"], &[], SnapshotStrategy::HardLink) {
        panic!("re-stage failed: {e}");
    }
    assert_eq!(read_to_string_required(&stage.join("ori")), "v2");
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn optional_names_stage_when_present_and_skip_when_absent() {
    let root = temp_test_dir("optional");
    let (src, stage) = (root.join("src"), root.join("stage"));
    write_file(&src.join("ori"), "bin");

    if let Err(e) = stage_snapshot(
        &src,
        &stage,
        &["ori"],
        &["libori_rt_asan.a"],
        SnapshotStrategy::HardLink,
    ) {
        panic!("stage without optional failed: {e}");
    }
    assert!(!stage.join("libori_rt_asan.a").exists());

    write_file(&src.join("libori_rt_asan.a"), "asan");
    if let Err(e) = stage_snapshot(
        &src,
        &stage,
        &["ori"],
        &["libori_rt_asan.a"],
        SnapshotStrategy::HardLink,
    ) {
        panic!("stage with optional failed: {e}");
    }
    assert!(stage.join("libori_rt_asan.a").exists());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn missing_required_source_is_an_error_never_a_silent_fallback() {
    let root = temp_test_dir("missing");
    let (src, stage) = (root.join("src"), root.join("stage"));
    if let Err(e) = fs::create_dir_all(&src) {
        panic!("test setup: {e}");
    }

    let r = stage_snapshot(&src, &stage, &["ori"], &[], SnapshotStrategy::HardLink);
    assert!(
        r.is_err(),
        "staging with a missing required artifact must fail loudly"
    );
}

#[cfg(unix)]
#[test]
fn staged_hardlink_shares_inode_with_source_at_stage_time() {
    let root = temp_test_dir("inode");
    let (src, stage) = (root.join("src"), root.join("stage"));
    write_file(&src.join("ori"), "v1");

    if let Err(e) = stage_snapshot(&src, &stage, &["ori"], &[], SnapshotStrategy::HardLink) {
        panic!("stage failed: {e}");
    }
    let src_meta = match fs::metadata(src.join("ori")) {
        Ok(m) => m,
        Err(e) => panic!("stat source: {e}"),
    };
    let staged_meta = match fs::metadata(stage.join("ori")) {
        Ok(m) => m,
        Err(e) => panic!("stat staged: {e}"),
    };
    assert_eq!(src_meta.dev(), staged_meta.dev());
    assert_eq!(src_meta.ino(), staged_meta.ino());
    let _ = fs::remove_dir_all(&root);
}

fn identity_of_required(path: &Path) -> String {
    match artifact_identity_of(path) {
        Ok(id) => id,
        Err(e) => panic!("identity of {}: {e}", path.display()),
    }
}

#[cfg(unix)]
#[test]
fn stage_snapshot_records_stage_time_staged_identity() {
    let root = temp_test_dir("identity");
    let (src, stage) = (root.join("src"), root.join("stage"));
    write_file(&src.join("ori"), "binary-v1");

    let pre_stage_identity = identity_of_required(&src.join("ori"));
    let staged = match stage_snapshot(&src, &stage, &["ori"], &[], SnapshotStrategy::HardLink) {
        Ok(artifacts) => artifacts,
        Err(e) => panic!("stage_snapshot failed: {e}"),
    };
    assert_eq!(staged.len(), 1);
    assert_eq!(staged[0].name, "ori");
    assert_eq!(staged[0].strategy_used, "hardlink");
    assert_eq!(staged[0].staged_identity, pre_stage_identity);

    // Shape pin: dev:inode:mtime:size — four ':'-separated integer fields,
    // the exact tuple the test-all.sh identity gate stats for its baseline.
    let fields: Vec<&str> = staged[0].staged_identity.split(':').collect();
    assert_eq!(fields.len(), 4, "identity must be dev:inode:mtime:size");
    for field in &fields {
        assert!(
            field.parse::<i128>().is_ok(),
            "non-integer identity field: {field}"
        );
    }
    let _ = fs::remove_dir_all(&root);
}

#[cfg(unix)]
#[test]
fn hardlink_identity_describes_pinned_inode_after_source_swap() {
    let root = temp_test_dir("pin-id");
    let (src, stage) = (root.join("src"), root.join("stage"));
    write_file(&src.join("ori"), "v1");

    let staged = match stage_snapshot(&src, &stage, &["ori"], &[], SnapshotStrategy::HardLink) {
        Ok(artifacts) => artifacts,
        Err(e) => panic!("stage_snapshot failed: {e}"),
    };
    replace_via_rename(&src.join("ori"), "v2-swapped-by-concurrent-build");

    // The recorded identity describes the PINNED inode (still readable via
    // the staged hardlink) — not whatever now lives at the source path.
    assert_eq!(
        staged[0].staged_identity,
        identity_of_required(&stage.join("ori")),
        "recorded identity must describe the pinned inode"
    );
    assert_ne!(
        staged[0].staged_identity,
        identity_of_required(&src.join("ori")),
        "a rename swap must move the live path off the pinned identity"
    );
    let _ = fs::remove_dir_all(&root);
}

#[cfg(unix)]
#[test]
fn copy_strategy_records_staged_identity() {
    let root = temp_test_dir("copy-id");
    let (src, stage) = (root.join("src"), root.join("stage"));
    write_file(&src.join("ori"), "v1");

    let staged = match stage_snapshot(&src, &stage, &["ori"], &[], SnapshotStrategy::Copy) {
        Ok(artifacts) => artifacts,
        Err(e) => panic!("stage_snapshot failed: {e}"),
    };
    assert_eq!(staged[0].strategy_used, "copy");
    // Snapshot-integrity: the gate re-stats the STAGED copy, so the recorded
    // identity is the copy's own (dst), never the source's. A fresh copy has a
    // distinct inode from the source, so the two identities differ.
    assert_eq!(
        staged[0].staged_identity,
        identity_of_required(&stage.join("ori")),
        "copy strategy must record the STAGED identity the gate re-stats"
    );
    assert_ne!(
        staged[0].staged_identity,
        identity_of_required(&src.join("ori")),
        "a copy's staged inode differs from the source inode"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn manifest_artifact_lines_carry_identity_in_fourth_field() {
    let artifacts = vec![
        StagedArtifact {
            name: "ori".to_string(),
            staged_identity: "64769:101:1700000000:4096".to_string(),
            strategy_used: "hardlink",
        },
        StagedArtifact {
            name: "libori_rt.a".to_string(),
            staged_identity: "64769:102:1700000001:8192".to_string(),
            strategy_used: "copy",
        },
    ];
    let manifest = render_stage_manifest(
        "debug",
        Path::new("/tmp/stage"),
        Path::new("/tmp/target/debug"),
        &artifacts,
    );
    let lines: Vec<&str> = manifest.lines().collect();
    assert_eq!(lines.first(), Some(&"schema 1"), "schema header must lead");
    assert!(lines.contains(&"profile debug"), "profile record required");

    // The test-all.sh gate extracts whitespace field 4 of each `artifact`
    // line; pin that exact shape: `artifact <name> <strategy> <identity>`.
    let mut artifact_lines = 0;
    for line in &lines {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.first() != Some(&"artifact") {
            continue;
        }
        artifact_lines += 1;
        assert_eq!(
            fields.len(),
            4,
            "artifact record must have 4 fields: {line}"
        );
        let Some(recorded) = artifacts.iter().find(|a| a.name == fields[1]) else {
            panic!("unknown artifact line: {line}")
        };
        assert_eq!(fields[2], recorded.strategy_used);
        assert_eq!(fields[3], recorded.staged_identity);
    }
    assert_eq!(artifact_lines, 2, "every staged artifact gets a record");
}

#[test]
fn manifest_publish_is_atomic_overwrite_with_no_temp_residue() {
    let root = temp_test_dir("manifest");
    let (stage, build) = (root.join("stage"), root.join("build"));
    let pointer = build.join("aot-stage-manifest-debug.txt");

    if let Err(e) = publish_stage_manifest(
        &stage,
        &pointer,
        "schema 1\nartifact ori hardlink 1:2:3:4\n",
    ) {
        panic!("first publish failed: {e}");
    }
    let second = "schema 1\nartifact ori hardlink 5:6:7:8\n";
    if let Err(e) = publish_stage_manifest(&stage, &pointer, second) {
        panic!("second publish failed: {e}");
    }

    // Overwrite-only channel: the second publish fully replaces BOTH homes.
    assert_eq!(read_to_string_required(&pointer), second);
    assert_eq!(
        read_to_string_required(&stage.join(STAGE_MANIFEST_NAME)),
        second
    );

    // temp+rename atomicity leaves no in-flight residue in either home.
    for dir in [&stage, &build] {
        let entries = match fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(e) => panic!("read_dir {}: {e}", dir.display()),
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            assert!(!name.ends_with(".tmp"), "temp residue left behind: {name}");
        }
    }
    let _ = fs::remove_dir_all(&root);
}

#[cfg(target_os = "linux")]
#[test]
fn identity_format_matches_gnu_stat_gate_consumer() {
    // SYNC PIN: artifact_identity_of() must byte-match the exact GNU
    // `stat -c '%d:%i:%Y:%s'` invocation the test-all.sh identity gate uses
    // for its baseline — the manifest values and the baseline are compared
    // as opaque strings.
    let root = temp_test_dir("statfmt");
    let file = root.join("ori");
    write_file(&file, "v1");

    let rust_identity = identity_of_required(&file);
    let out = match std::process::Command::new("stat")
        .args(["-c", "%d:%i:%Y:%s"])
        .arg(&file)
        .output()
    {
        Ok(out) => out,
        Err(e) => panic!("spawn stat: {e}"),
    };
    assert!(
        out.status.success(),
        "stat failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stat_identity = String::from_utf8_lossy(&out.stdout).trim().to_string();
    assert_eq!(rust_identity, stat_identity);
    let _ = fs::remove_dir_all(&root);
}

#[cfg(unix)]
#[test]
fn stale_dead_pid_stage_dirs_are_cleaned() {
    let tag_root = std::env::temp_dir();
    // A PID that cannot be alive (beyond pid_max defaults) embedded in the
    // stale dir name; the cleaner must remove it. A dir named with OUR live
    // pid must survive.
    // Non-profile suffixes: never collide with the REAL staged-artifacts dir
    // of this very test process (creating/removing that name mid-suite would
    // clobber the production snapshot).
    let stale = tag_root.join("ori-aot-stage-999999999-pintest");
    let live = tag_root.join(format!("ori-aot-stage-{}-pintest", std::process::id()));
    if let Err(e) = fs::create_dir_all(&stale) {
        panic!("test setup: {e}");
    }
    if let Err(e) = fs::create_dir_all(&live) {
        panic!("test setup: {e}");
    }

    super::clean_stale_stage_dirs();

    assert!(!stale.exists(), "dead-pid stage dir must be removed");
    assert!(live.exists(), "live-pid stage dir must be kept");
    let _ = fs::remove_dir_all(&live);
}
