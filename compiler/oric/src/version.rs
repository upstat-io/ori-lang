//! Version string the `ori` binary reports.
//!
//! Single source of truth for the banner shown by `ori version` and `ori help`.
//! `BUILD_NUMBER` (repo root) is the canonical source for the release version and
//! stage. Release builds report it verbatim. Dev builds recompute the date to the
//! local build day so a stale committed `BUILD_NUMBER` does not surface a
//! months-old date during development.

use std::time::{SystemTime, UNIX_EPOCH};

/// The committed build number, `YYYY.MM.DD.N-STAGE` (stage optional).
const BUILD_NUMBER: &str = include_str!("../../../BUILD_NUMBER");

const SECONDS_PER_DAY: i64 = 86_400;

/// A proleptic-Gregorian calendar date.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CivilDate {
    year: i64,
    month: i64,
    day: i64,
}

/// The version string this binary reports in `ori version` and `ori help`.
///
/// Release builds return `BUILD_NUMBER` verbatim. Dev builds replace the date
/// with the local build day and append `-dev`, keeping the stage from
/// `BUILD_NUMBER`.
// Why: the banner is build identity, not Ori-program behavior — the
// debug/release divergence here is the banner's purpose, not a codegen-parity gap.
#[must_use]
pub fn report_version() -> String {
    compose_version(BUILD_NUMBER, cfg!(debug_assertions), today_utc())
}

/// Assemble the reported version from a build number, dev flag, and current date.
///
/// `dev == false` returns the trimmed build number verbatim (release / CI path).
/// `dev == true` recomputes the date to `today` and marks the result `-dev`.
fn compose_version(build_number: &str, dev: bool, today: CivilDate) -> String {
    let build_number = build_number.trim();
    if dev {
        dev_version(build_number, today)
    } else {
        build_number.to_string()
    }
}

/// Compose a dev-build version: today's date, stage from `build_number`, `-dev`.
///
/// `2026.04.17.1-alpha` + 2026-06-01 -> `2026.06.01.0-alpha-dev`.
/// `2026.04.17.1`       + 2026-06-01 -> `2026.06.01.0-dev`.
fn dev_version(build_number: &str, today: CivilDate) -> String {
    let date = format!("{:04}.{:02}.{:02}", today.year, today.month, today.day);
    match stage_of(build_number) {
        Some(stage) => format!("{date}.0-{stage}-dev"),
        None => format!("{date}.0-dev"),
    }
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

/// Today's date in UTC, from the system clock.
fn today_utc() -> CivilDate {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let secs = i64::try_from(secs).unwrap_or(i64::MAX);
    civil_from_days(secs.div_euclid(SECONDS_PER_DAY))
}

/// Convert a count of days since 1970-01-01 to a civil date.
///
/// Howard Hinnant's `civil_from_days` (public domain). Valid across the full i64
/// day range, including negative (pre-epoch) counts.
fn civil_from_days(z: i64) -> CivilDate {
    // Shift the epoch to 0000-03-01 so the leap day lands at the end of the
    // 400-year era, simplifying the day-of-era arithmetic below.
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let day = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let month = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    CivilDate {
        year: if month <= 2 { year + 1 } else { year },
        month,
        day,
    }
}

#[cfg(test)]
mod tests;
