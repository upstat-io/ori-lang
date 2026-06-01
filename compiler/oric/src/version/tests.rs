//! Tests for the reported version string.
//!
//! Epoch-day fixtures verified against Python's `datetime` (days since
//! 1970-01-01): 2026-06-01 = 20605, leap 2024-02-29 = 19782, pre-epoch
//! 1969-12-31 = -1.

use super::*;

fn date(year: i64, month: i64, day: i64) -> CivilDate {
    CivilDate { year, month, day }
}

// civil_from_days — calendar conversion across boundaries.

#[test]
fn civil_from_days_at_epoch_returns_1970_01_01() {
    assert_eq!(civil_from_days(0), date(1970, 1, 1));
}

#[test]
fn civil_from_days_for_20605_returns_2026_06_01() {
    assert_eq!(civil_from_days(20_605), date(2026, 6, 1));
}

#[test]
fn civil_from_days_for_leap_day_returns_2024_02_29() {
    assert_eq!(civil_from_days(19_782), date(2024, 2, 29));
}

#[test]
fn civil_from_days_day_after_leap_returns_2024_03_01() {
    assert_eq!(civil_from_days(19_783), date(2024, 3, 1));
}

#[test]
fn civil_from_days_for_year_end_returns_1999_12_31() {
    assert_eq!(civil_from_days(10_956), date(1999, 12, 31));
}

#[test]
fn civil_from_days_for_century_rollover_returns_2000_01_01() {
    assert_eq!(civil_from_days(10_957), date(2000, 1, 1));
}

#[test]
fn civil_from_days_for_negative_count_returns_pre_epoch_date() {
    assert_eq!(civil_from_days(-1), date(1969, 12, 31));
}

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

// dev_version — composing the dev banner.

#[test]
fn dev_version_with_stage_uses_today_and_marks_dev() {
    assert_eq!(
        dev_version("2026.04.17.1-alpha", date(2026, 6, 1)),
        "2026.06.01.0-alpha-dev"
    );
}

#[test]
fn dev_version_stable_build_marks_dev_without_stage() {
    assert_eq!(
        dev_version("2026.04.17.1", date(2026, 6, 1)),
        "2026.06.01.0-dev"
    );
}

#[test]
fn dev_version_zero_pads_single_digit_month_and_day() {
    assert_eq!(
        dev_version("2026.04.17.1-alpha", date(2026, 3, 5)),
        "2026.03.05.0-alpha-dev"
    );
}

// compose_version — release vs dev branching (positive + negative pins).

#[test]
fn compose_version_release_returns_build_number_verbatim() {
    assert_eq!(
        compose_version("2026.04.17.1-alpha", false, date(2026, 6, 1)),
        "2026.04.17.1-alpha"
    );
}

#[test]
fn compose_version_release_trims_trailing_whitespace() {
    assert_eq!(
        compose_version("2026.04.17.1-alpha\n", false, date(2026, 6, 1)),
        "2026.04.17.1-alpha"
    );
}

#[test]
fn compose_version_release_never_marks_dev() {
    let release = compose_version("2026.04.17.1-alpha", false, date(2026, 6, 1));
    assert!(
        !release.contains("-dev"),
        "release banner must not be marked dev: {release}"
    );
}

#[test]
fn compose_version_dev_recomputes_date_and_marks_dev() {
    assert_eq!(
        compose_version("2026.04.17.1-alpha", true, date(2026, 6, 1)),
        "2026.06.01.0-alpha-dev"
    );
}

#[test]
fn compose_version_dev_ignores_stale_build_date() {
    let dev = compose_version("2020.01.01.9-alpha", true, date(2026, 6, 1));
    assert!(
        !dev.starts_with("2020"),
        "dev banner must use today's date, not the stale build date: {dev}"
    );
    assert!(dev.starts_with("2026.06.01"), "got: {dev}");
}

// report_version — banner matches the build profile (debug vs release).

#[test]
fn report_version_matches_build_profile() {
    let reported = report_version();
    if cfg!(debug_assertions) {
        assert!(
            reported.ends_with("-dev"),
            "debug build must report a dev banner: {reported}"
        );
    } else {
        assert!(
            !reported.ends_with("-dev"),
            "release build must report the build number verbatim: {reported}"
        );
        assert_eq!(reported, BUILD_NUMBER.trim());
    }
}
