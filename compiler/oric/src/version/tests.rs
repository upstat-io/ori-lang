//! Tests for the reported version string.

use super::*;

// stage_of — extracting the release stage from a build number.

#[test]
fn stage_of_alpha_build_returns_alpha() {
    assert_eq!(stage_of("2026.04.17.1-alpha"), Some("alpha"));
}

#[test]
fn stage_of_beta_build_returns_beta() {
    assert_eq!(stage_of("2026.04.17.1-beta"), Some("beta"));
}

#[test]
fn stage_of_rc_build_returns_rc() {
    assert_eq!(stage_of("2026.04.17.1-rc"), Some("rc"));
}

#[test]
fn stage_of_stable_build_returns_none() {
    assert_eq!(stage_of("2026.04.17.1"), None);
}

#[test]
fn stage_of_empty_suffix_returns_none() {
    assert_eq!(stage_of("2026.04.17.1-"), None);
}

// build_number_date — date prefix extraction for the no-stamp fallback.

#[test]
fn build_number_date_with_stage_returns_iso_prefix() {
    assert_eq!(build_number_date("2026.04.17.1-alpha"), "2026-04-17");
}

#[test]
fn build_number_date_stable_returns_iso_prefix() {
    assert_eq!(build_number_date("2026.04.17.3"), "2026-04-17");
}

// dev_version — composing the dev banner from the compile-time stamp.

#[test]
fn dev_version_with_stage_clean_carries_sha_and_date() {
    assert_eq!(
        dev_version("2026.04.17.1-alpha", "b45f5cc76", false, "2026-06-13"),
        "2026.06.13-alpha-dev (b45f5cc76 2026-06-13)"
    );
}

#[test]
fn dev_version_with_stage_dirty_appends_dirty_marker() {
    assert_eq!(
        dev_version("2026.04.17.1-alpha", "b45f5cc76", true, "2026-06-13"),
        "2026.06.13-alpha-dev (b45f5cc76 2026-06-13, dirty)"
    );
}

#[test]
fn dev_version_stable_build_marks_dev_without_stage() {
    assert_eq!(
        dev_version("2026.04.17.1", "b45f5cc76", false, "2026-06-13"),
        "2026.06.13-dev (b45f5cc76 2026-06-13)"
    );
}

#[test]
fn dev_version_uses_stamped_date_not_build_number_date() {
    // Semantic pin: the banner reflects the BUILD stamp, not the committed
    // BUILD_NUMBER date — the whole point of the build-time stamp.
    let banner = dev_version("2026.04.17.1-alpha", "b45f5cc76", false, "2026-06-13");
    assert!(
        banner.starts_with("2026.06.13"),
        "dev banner must use the stamped build date, not the build-number date: {banner}"
    );
}

#[test]
fn dev_version_without_git_falls_back_to_build_number_date() {
    // Negative path: git unavailable at build time -> empty stamp date and the
    // SHA sentinel. Banner degrades to the build-number date, no (sha …) clause.
    assert_eq!(
        dev_version("2026.04.17.1-alpha", "unknown", false, ""),
        "2026.04.17-alpha-dev"
    );
}

#[test]
fn dev_version_unknown_sha_omits_identity_clause() {
    let banner = dev_version("2026.04.17.1-alpha", "unknown", true, "2026-06-13");
    assert!(
        !banner.contains('('),
        "no identity clause when the sha is unknown: {banner}"
    );
}

// compose_version — release vs dev branching (positive + negative pins).

#[test]
fn compose_version_release_returns_build_number_verbatim() {
    assert_eq!(
        compose_version("2026.04.17.1-alpha", false, "b45f5cc76", true, "2026-06-13"),
        "2026.04.17.1-alpha"
    );
}

#[test]
fn compose_version_release_trims_trailing_whitespace() {
    assert_eq!(
        compose_version(
            "2026.04.17.1-alpha\n",
            false,
            "b45f5cc76",
            false,
            "2026-06-13"
        ),
        "2026.04.17.1-alpha"
    );
}

#[test]
fn compose_version_release_never_marks_dev() {
    let release = compose_version("2026.04.17.1-alpha", false, "b45f5cc76", true, "2026-06-13");
    assert!(
        !release.contains("-dev"),
        "release banner must not be marked dev: {release}"
    );
}

#[test]
fn compose_version_release_omits_sha_identity_clause() {
    // Negative pin: the dev-only build identity must not leak into release.
    let release = compose_version("2026.04.17.1-alpha", false, "b45f5cc76", true, "2026-06-13");
    assert!(
        !release.contains("b45f5cc76"),
        "release banner must not carry the dev sha clause: {release}"
    );
}

#[test]
fn compose_version_dev_marks_dev_and_carries_sha() {
    let dev = compose_version("2026.04.17.1-alpha", true, "b45f5cc76", false, "2026-06-13");
    assert_eq!(dev, "2026.06.13-alpha-dev (b45f5cc76 2026-06-13)");
}

#[test]
fn compose_version_dev_honors_stamped_date_over_build_number() {
    // Semantic pin (inverts the old run-date behavior): a stale BUILD_NUMBER
    // does not override the stamped build date.
    let dev = compose_version("2020.01.01.9-alpha", true, "b45f5cc76", false, "2026-06-13");
    assert!(
        dev.starts_with("2026.06.13"),
        "dev banner must use the stamped date, not the stale build number: {dev}"
    );
    assert!(
        !dev.starts_with("2020"),
        "stale build-number date must not leak into the dev banner: {dev}"
    );
}

// report_version — production entry point (the real `ori version` path),
// driving the compile-time env stamp, not an injected seam.

#[test]
fn report_version_matches_build_profile_and_carries_stamp() {
    let reported = report_version();
    if cfg!(debug_assertions) {
        assert!(
            reported.contains("-dev"),
            "debug build must report a dev banner: {reported}"
        );
        if GIT_SHA != SHA_UNKNOWN {
            assert!(
                reported.contains(GIT_SHA),
                "dev banner must carry the stamped commit sha: {reported}"
            );
        }
    } else {
        assert!(
            !reported.contains("-dev"),
            "release build must report the build number verbatim: {reported}"
        );
        assert_eq!(reported, BUILD_NUMBER.trim());
    }
}
