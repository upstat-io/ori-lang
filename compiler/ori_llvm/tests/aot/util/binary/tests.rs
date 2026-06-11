//! Unit pins for the per-process artifact snapshot staging.
//!
//! The staging core exists so the AOT suite resolves immutable per-run
//! snapshots of the compiler binary + runtime staticlib instead of the
//! shared mutable `target/<profile>/` paths a concurrent build can swap
//! mid-suite (mass bogus failures otherwise).

use super::{stage_snapshot, SnapshotStrategy};
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
