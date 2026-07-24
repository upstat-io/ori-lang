//! Version string the `ori` binary reports.
//!
//! Single source of truth for the banner shown by `ori version` and `ori help`.
//! `BUILD_NUMBER` (repo root) is the canonical release version and stage;
//! release builds report it verbatim. Dev builds report the build identity
//! stamped at compile time by `build.rs` — commit short SHA, dirty flag, and
//! commit date — so a binary's age is visible from `ori version` rather than
//! masked behind a run-time clock or a stale committed date.

/// The committed build number, `YYYY.MM.DD.N-STAGE` (stage optional).
const BUILD_NUMBER: &str = include_str!("../../../BUILD_NUMBER");

/// Build identity stamped by `build.rs` at compile time.
const GIT_SHA: &str = env!("ORI_GIT_SHA");
/// `"1"` when the build worktree had uncommitted changes, else `"0"`.
const GIT_DIRTY: &str = env!("ORI_GIT_DIRTY");
/// HEAD commit date `YYYY-MM-DD`, or empty when git was unavailable at build.
const BUILD_DATE: &str = env!("ORI_BUILD_DATE");

/// Sentinel `GIT_SHA` value when git was unavailable at build time.
const SHA_UNKNOWN: &str = "unknown";

/// The version string this binary reports in `ori version` and `ori help`.
///
/// Release builds return `BUILD_NUMBER` verbatim. Dev builds report the
/// stamped commit date, stage, `-dev`, and a `(sha date[, dirty])` identity.
// Why: the banner is build identity, not Ori-program behavior — the
// debug/release divergence here is the banner's purpose, not a codegen-parity gap.
#[must_use]
pub fn report_version() -> String {
    compose_version(
        BUILD_NUMBER,
        cfg!(debug_assertions),
        GIT_SHA,
        GIT_DIRTY == "1",
        BUILD_DATE,
    )
}

/// Assemble the reported version from a build number, dev flag, and the
/// compile-time build stamp.
///
/// `dev == false` returns the trimmed build number verbatim (release / CI path).
/// `dev == true` reports the stamped build identity.
fn compose_version(
    build_number: &str,
    dev: bool,
    sha: &str,
    dirty: bool,
    build_date: &str,
) -> String {
    let build_number = build_number.trim();
    if dev {
        dev_version(build_number, sha, dirty, build_date)
    } else {
        build_number.to_string()
    }
}

/// Compose a dev-build version from the compile-time stamp.
///
/// `2026.06.13-alpha-dev (b45f5cc76 2026-06-13, dirty)` — stage from
/// `build_number`, date + sha from the stamp. Falls back to the `build_number`
/// date when the stamp carries no commit date, and drops the `(sha …)` clause
/// when git was unavailable at build time.
fn dev_version(build_number: &str, sha: &str, dirty: bool, build_date: &str) -> String {
    let date_iso = if build_date.is_empty() {
        build_number_date(build_number)
    } else {
        build_date.to_string()
    };
    let date_dotted = date_iso.replace('-', ".");

    let core = match stage_of(build_number) {
        Some(stage) => format!("{date_dotted}-{stage}-dev"),
        None => format!("{date_dotted}-dev"),
    };

    if sha == SHA_UNKNOWN {
        return core;
    }

    let dirty_suffix = if dirty { ", dirty" } else { "" };
    format!("{core} ({sha} {date_iso}{dirty_suffix})")
}

/// The `YYYY-MM-DD` date prefix of a build number (`2026.04.17.1-alpha` ->
/// `2026-04-17`). Used as the dev-banner date when no commit date was stamped.
fn build_number_date(build_number: &str) -> String {
    let mut parts = build_number.split('.');
    let year = parts.next().unwrap_or("0000");
    let month = parts.next().unwrap_or("00");
    let day = parts.next().unwrap_or("00");
    format!("{year}-{month}-{day}")
}

/// The release-stage suffix (`alpha` / `beta` / `rc`) of a build number, if any.
///
/// Date components are dot-separated; the stage is the segment after the first
/// `-`. Returns `None` for a stable build number (no `-`).
fn stage_of(build_number: &str) -> Option<&str> {
    build_number
        .split_once('-')
        .map(|(_, stage)| stage)
        .filter(|stage| !stage.is_empty())
}

#[cfg(test)]
mod tests;
