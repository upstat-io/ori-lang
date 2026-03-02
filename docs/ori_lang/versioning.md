# Versioning

Ori uses calendar-based versioning (CalVer) for all artifacts.

## Build Versions

Format: `v<Year>.<Month>.<Day>.<Incremental>-<Stage>`

| Component     | Description                                      |
|---------------|--------------------------------------------------|
| `Year`        | Four-digit year                                  |
| `Month`       | Two-digit month (zero-padded)                    |
| `Day`         | Two-digit day (zero-padded)                      |
| `Incremental` | Sequential build number within the day (1-based) |
| `Stage`       | Release stage (see below)                        |

EXAMPLE 1  `v2026.02.28.1-Alpha` — first alpha build on 2026-02-28.

EXAMPLE 2  `v2026.09.01.5-Release` — fifth release build on 2026-09-01.

## Stages

| Stage     | Meaning                                    |
|-----------|--------------------------------------------|
| `Alpha`   | Unstable; breaking changes expected        |
| `Beta`    | Feature-complete; stabilizing              |
| `RC`      | Release candidate; final review            |
| `Release` | Stable release                             |

Stages progress in order: Alpha, Beta, RC, Release. A version shall not regress to an earlier stage.

## Specification Editions

Format: `<Year>`

A specification edition covers **all builds within that year**, regardless of stage. The directory is just the year; the displayed version is injected from `BUILD_NUMBER` at build time.

EXAMPLE  The `2026` edition applies to all `v2026.*` builds (Alpha through Release).

When the year increments, a new specification edition begins. Within a year, the spec may be revised, but all revisions apply to the same set of builds.

## Comparison

| Artifact          | Format                                 | Example                  |
|-------------------|----------------------------------------|--------------------------|
| Nightly build     | `v<Y>.<M>.<D>.<N>-<Stage>`            | `v2026.03.15.2-Alpha`   |
| Release build     | `v<Y>.<M>.<D>.<N>-<Stage>`            | `v2026.09.01.5-Release` |
| Specification     | `<Year>`                               | `2026`                   |
| Documentation dir | `docs/ori_lang/v<Year>/`               | `docs/ori_lang/v2026/`    |
| Displayed version | From `BUILD_NUMBER`                    | `2026.02.28.1-alpha`     |
