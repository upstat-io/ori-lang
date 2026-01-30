# Proposal: std.time API Design (FFI Revision)

**Status:** Draft (Revision of Approved)
**Created:** 2026-01-30
**Affects:** Standard library
**Depends on:** C FFI proposal

---

## Summary

This revision adds FFI implementation details to the approved `std.time` proposal. The public API remains unchanged; this documents the C library backends.

---

## FFI Implementation

### Backend: libc/POSIX time functions

All time operations are backed by standard POSIX/C library functions, ensuring correctness and consistency with the OS.

### External Declarations

```ori
// std/time/ffi.ori (internal)
#repr("c")
type CTimeSpec = {
    tv_sec: int,   // time_t
    tv_nsec: int   // long
}

#repr("c")
type CTm = {
    tm_sec: int,
    tm_min: int,
    tm_hour: int,
    tm_mday: int,
    tm_mon: int,
    tm_year: int,
    tm_wday: int,
    tm_yday: int,
    tm_isdst: int
}

extern "c" from "libc" {
    // High-resolution time
    @_clock_gettime (clock_id: int, tp: CTimeSpec) -> int as "clock_gettime"

    // Time conversion
    @_gmtime_r (time: int, result: CTm) -> CPtr as "gmtime_r"
    @_localtime_r (time: int, result: CTm) -> CPtr as "localtime_r"
    @_mktime (tm: CTm) -> int as "mktime"
    @_timegm (tm: CTm) -> int as "timegm"

    // Formatting
    @_strftime (buf: [byte], maxsize: int, format: str, tm: CTm) -> int as "strftime"
    @_strptime (s: str, format: str, tm: CTm) -> CPtr as "strptime"

    // Timezone
    @_tzset () -> void as "tzset"
}

// Clock IDs
let $CLOCK_REALTIME: int = 0
let $CLOCK_MONOTONIC: int = 1
```

### Platform-Specific Variants

```ori
#target(os: "macos")
extern "c" from "libc" {
    // macOS doesn't have clock_gettime before 10.12, use mach_absolute_time
    @_mach_absolute_time () -> int as "mach_absolute_time"
    @_gettimeofday (tv: CTimeVal, tz: CPtr) -> int as "gettimeofday"
}

#target(os: "windows")
extern "c" from "kernel32" {
    @_GetSystemTimePreciseAsFileTime (ft: CPtr) -> void as "GetSystemTimePreciseAsFileTime"
    @_QueryPerformanceCounter (counter: CPtr) -> int as "QueryPerformanceCounter"
    @_QueryPerformanceFrequency (freq: CPtr) -> int as "QueryPerformanceFrequency"
}
```

---

## Implementation Mapping

### Instant

| Method | FFI Implementation |
|--------|-------------------|
| `Instant.now()` | `clock_gettime(CLOCK_REALTIME)` |
| `Instant.from_unix_secs(n)` | Pure Ori: `{ nanoseconds: n * 1_000_000_000 }` |
| `Instant.to_unix_secs()` | Pure Ori: `self.nanoseconds / 1_000_000_000` |

```ori
// std/time/instant.ori
use "./ffi" { _clock_gettime, CTimeSpec, $CLOCK_REALTIME }

impl Instant {
    pub @now () -> Instant uses Clock =
        run(
            let ts = CTimeSpec { tv_sec: 0, tv_nsec: 0 },
            let result = _clock_gettime(clock_id: $CLOCK_REALTIME, tp: ts),
            if result != 0 then panic(msg: "clock_gettime failed"),
            Instant { nanoseconds: ts.tv_sec * 1_000_000_000 + ts.tv_nsec }
        )
}
```

### DateTime Conversion

| Method | FFI Implementation |
|--------|-------------------|
| `DateTime.from_instant(instant, tz)` | `gmtime_r` or `localtime_r` + offset calculation |
| `DateTime.to_instant()` | `timegm` or `mktime` |

```ori
// std/time/datetime.ori
use "./ffi" { _gmtime_r, _localtime_r, _mktime, _timegm, CTm }

impl DateTime {
    pub @from_instant (instant: Instant, tz: Timezone) -> DateTime =
        run(
            let secs = instant.nanoseconds / 1_000_000_000,
            let nanos = instant.nanoseconds % 1_000_000_000,
            let tm = CTm { ... },  // zero-initialized

            if tz.is_utc() then
                _gmtime_r(time: secs, result: tm)
            else
                _localtime_r(time: secs, result: tm),

            DateTime {
                year: tm.tm_year + 1900,
                month: tm.tm_mon + 1,
                day: tm.tm_mday,
                hour: tm.tm_hour,
                minute: tm.tm_min,
                second: tm.tm_sec,
                nanosecond: nanos as int,
                timezone: tz
            }
        )
}
```

### Formatting

| Function | FFI Implementation |
|----------|-------------------|
| `format(dt, pattern)` | `strftime` with pattern translation |
| `to_iso8601(dt)` | `strftime` with `"%Y-%m-%dT%H:%M:%S"` + manual timezone |

```ori
// Pattern translation from Ori format to strftime
@translate_pattern (ori_pattern: str) -> str =
    ori_pattern
        .replace(from: "YYYY", to: "%Y")
        .replace(from: "YY", to: "%y")
        .replace(from: "MMMM", to: "%B")
        .replace(from: "MMM", to: "%b")
        .replace(from: "MM", to: "%m")
        .replace(from: "DD", to: "%d")
        .replace(from: "HH", to: "%H")
        .replace(from: "mm", to: "%M")
        .replace(from: "ss", to: "%S")
        .replace(from: "EEEE", to: "%A")
        .replace(from: "E", to: "%a")
        // ... etc

pub @format (dt: DateTime, pattern: str) -> str =
    run(
        let tm = datetime_to_ctm(dt: dt),
        let c_pattern = translate_pattern(ori_pattern: pattern),
        let buf = [0 as byte; 256],
        let len = _strftime(buf: buf, maxsize: 256, format: c_pattern, tm: tm),
        str.from_bytes(bytes: buf[0..len])
    )
```

### Parsing

| Function | FFI Implementation |
|----------|-------------------|
| `parse(source, pattern, tz)` | `strptime` with pattern translation |
| `from_iso8601(source)` | `strptime` with `"%Y-%m-%dT%H:%M:%S"` + manual timezone parsing |

### Timezone

| Method | FFI Implementation |
|--------|-------------------|
| `Timezone.utc()` | Pure Ori: fixed offset of 0 |
| `Timezone.local()` | `localtime_r` to detect offset |
| `Timezone.from_name(name)` | IANA tzdb lookup (see below) |

#### Timezone Database

For IANA timezone support (e.g., "America/New_York"), we have options:

**Option A: Bundle tzdb (Recommended)**
- Ship compiled timezone data with Ori stdlib
- Use Howard Hinnant's `date` library C++ code or similar
- Pro: Consistent behavior across platforms
- Con: Larger binary size (~2MB)

**Option B: Use system tzdb**
- Read `/usr/share/zoneinfo/` on Unix
- Use `GetTimeZoneInformation` on Windows
- Pro: Smaller binary, system-consistent
- Con: Platform differences, Windows lacks IANA names

```ori
// Timezone lookup
impl Timezone {
    pub @from_name (name: str) -> Result<Timezone, TimeError> =
        run(
            let tz_data = load_tzdb_entry(name: name)?,
            Ok(Timezone { data: tz_data })
        )
}
```

---

## Pure Ori Components

These components don't need FFI:

| Component | Implementation |
|-----------|----------------|
| `Duration` arithmetic | Pure Ori integer math |
| `Date` arithmetic | Pure Ori (add_days, diff_days, etc.) |
| `Weekday` enum | Pure Ori |
| `Date.is_leap_year()` | Pure Ori algorithm |
| `Date.days_in_month()` | Pure Ori lookup table |

```ori
impl Date {
    pub @is_leap_year (self) -> bool =
        (self.year % 4 == 0 && self.year % 100 != 0) || (self.year % 400 == 0)

    pub @days_in_month (self) -> int =
        match(self.month,
            1 -> 31, 2 -> if self.is_leap_year() then 29 else 28,
            3 -> 31, 4 -> 30, 5 -> 31, 6 -> 30,
            7 -> 31, 8 -> 31, 9 -> 30, 10 -> 31, 11 -> 30, 12 -> 31,
            _ -> panic(msg: "invalid month")
        )
}
```

---

## Build Configuration

```toml
# ori.toml
[native]
libraries = []  # libc is implicit

[native.linux]
libraries = ["rt"]  # clock_gettime on older Linux

[native.macos]
frameworks = []

[native.windows]
libraries = ["kernel32"]
```

---

## Testing Strategy

### Mock Clock for Tests

The `Clock` capability allows injecting mock time without FFI changes:

```ori
type MockClock = { current: Instant }

impl Clock for MockClock {
    @now () -> Instant = self.current
    @local_timezone () -> Timezone = Timezone.utc()
}

@t tests @format () -> void =
    run(
        let fixed = Instant.from_unix_secs(secs: 1700000000),
        let mock = MockClock { current: fixed },

        with Clock = mock in run(
            let dt = DateTime.now_utc(),
            assert_eq(actual: dt.year, expected: 2023),
            assert_eq(actual: dt.month, expected: 11),
        )
    )
```

### FFI Boundary Tests

Test actual FFI calls against known values:

```ori
@t tests @clock_gettime_works () -> void =
    run(
        let before = 1700000000,  // Known past timestamp
        let now = Instant.now(),
        assert(condition: now.to_unix_secs() > before)
    )
```

---

## Summary of Changes from Original

| Aspect | Original | This Revision |
|--------|----------|---------------|
| Public API | Defined | **Unchanged** |
| Implementation | Not specified | **FFI to libc** |
| Platform support | Implied | **Explicit per-platform** |
| Timezone data | Not specified | **Bundled tzdb recommended** |
| Testing | Capability-based | **Capability + FFI boundary tests** |
