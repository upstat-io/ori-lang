# Proposal: std.time API Design

**Status:** Approved
**Approved:** 2026-01-30
**Author:** Eric (with AI assistance)
**Created:** 2026-01-29
**Affects:** Standard library

---

## Summary

This proposal defines the API for `std.time`, providing date/time types, formatting, parsing, arithmetic, and timezone handling.

---

## Motivation

Time handling is needed for:
- Timestamps and logging
- Scheduling and timeouts
- Date calculations
- User-facing date display
- Data serialization

The API should be:
1. **Correct** — Handle edge cases (leap years, DST, timezones)
2. **Ergonomic** — Common operations should be simple
3. **Explicit** — Timezone handling must be clear
4. **Efficient** — Minimal overhead for common operations

---

## Core Types

### Instant

A point in time (UTC), independent of timezone:

```ori
type Instant = { nanoseconds: int }  // Nanoseconds since Unix epoch

impl Instant {
    @now () -> Instant uses Clock
    @from_unix_secs (secs: int) -> Instant
    @from_unix_millis (millis: int) -> Instant
    @to_unix_secs (self) -> int
    @to_unix_millis (self) -> int

    // Arithmetic
    @add (self, duration: Duration) -> Instant
    @sub (self, duration: Duration) -> Instant
    @diff (self, other: Instant) -> Duration

    // Comparison (via Comparable trait)
}
```

Usage:
```ori
use std.time { Instant }

let start = Instant.now()
do_work()
let elapsed = Instant.now().diff(other: start)
print(msg: `Took {elapsed.to_millis()}ms`)
```

### DateTime

A date and time in a specific timezone:

```ori
type DateTime = {
    year: int,
    month: int,      // 1-12
    day: int,        // 1-31
    hour: int,       // 0-23
    minute: int,     // 0-59
    second: int,     // 0-59
    nanosecond: int, // 0-999,999,999
    timezone: Timezone,
}

impl DateTime {
    @now () -> DateTime uses Clock           // Local time
    @now_utc () -> DateTime uses Clock       // UTC
    @from_instant (instant: Instant, tz: Timezone) -> DateTime
    @from_parts (date: Date, time: Time, tz: Timezone) -> DateTime

    @to_instant (self) -> Instant
    @to_timezone (self, tz: Timezone) -> DateTime
    @to_utc (self) -> DateTime
    @to_local (self) -> DateTime uses Clock

    // Components
    @date (self) -> Date
    @time (self) -> Time
    @weekday (self) -> Weekday

    // Arithmetic
    @add (self, duration: Duration) -> DateTime
    @add_days (self, days: int) -> DateTime
    @add_months (self, months: int) -> DateTime
    @add_years (self, years: int) -> DateTime
}
```

Usage:
```ori
use std.time { DateTime, Timezone }

let now = DateTime.now()
print(msg: `Current time: {now.hour}:{now.minute}`)

let utc = now.to_utc()
let tokyo = now.to_timezone(tz: Timezone.from_name("Asia/Tokyo"))
```

### Date

Just a date (no time component):

```ori
type Date = {
    year: int,
    month: int,
    day: int,
}

impl Date {
    @today () -> Date uses Clock
    @new (year: int, month: int, day: int) -> Result<Date, TimeError>

    @weekday (self) -> Weekday
    @day_of_year (self) -> int
    @is_leap_year (self) -> bool
    @days_in_month (self) -> int

    @add_days (self, days: int) -> Date
    @add_months (self, months: int) -> Date
    @add_years (self, years: int) -> Date
    @diff_days (self, other: Date) -> int
}
```

Usage:
```ori
use std.time { Date }

let today = Date.today()
let birthday = Date.new(year: 1990, month: 3, day: 15)?
let age_days = today.diff_days(other: birthday)
```

### Time

Just a time of day (no date component):

```ori
type Time = {
    hour: int,
    minute: int,
    second: int,
    nanosecond: int,
}

impl Time {
    @now () -> Time uses Clock
    @new (hour: int, minute: int, second: int = 0, nanosecond: int = 0) -> Result<Time, TimeError>
    @midnight () -> Time
    @noon () -> Time

    @to_seconds (self) -> int
    @to_millis (self) -> int
}
```

### Timezone

```ori
type Timezone = { ... }  // Opaque

impl Timezone {
    @utc () -> Timezone
    @local () -> Timezone uses Clock
    @from_name (name: str) -> Result<Timezone, TimeError>  // "America/New_York"
    @from_offset (hours: int, minutes: int = 0) -> Timezone
    @fixed (offset: Duration) -> Timezone

    @name (self) -> str
    @offset_at (self, instant: Instant) -> Duration
}
```

Usage:
```ori
use std.time { Timezone }

let utc = Timezone.utc()
let eastern = Timezone.from_name(name: "America/New_York")?
let plus_9 = Timezone.from_offset(hours: 9)  // UTC+9
```

### Weekday

```ori
type Weekday = Monday | Tuesday | Wednesday | Thursday | Friday | Saturday | Sunday

impl Weekday {
    @is_weekend (self) -> bool
    @next (self) -> Weekday
    @prev (self) -> Weekday
    @all () -> [Weekday]  // [Monday, Tuesday, ..., Sunday]
}
```

---

## Duration Type

Already exists in Ori as a built-in. This proposal adds extension methods.

> **Note:** Duration methods defined in `std.time` are extension methods. The built-in `Duration` type supports literals (`100ms`, `30s`) and basic arithmetic. Extended methods like `from_nanos()` and component extraction require importing from `std.time`:
>
> ```ori
> use std.time { Duration }
> let d = Duration.from_days(n: 7)
> ```

This proposal adds methods:

```ori
impl Duration {
    // Construction
    @from_nanos (n: int) -> Duration
    @from_micros (n: int) -> Duration
    @from_millis (n: int) -> Duration
    @from_secs (n: int) -> Duration
    @from_mins (n: int) -> Duration
    @from_hours (n: int) -> Duration
    @from_days (n: int) -> Duration

    // Extraction
    @to_nanos (self) -> int
    @to_micros (self) -> int
    @to_millis (self) -> int
    @to_secs (self) -> int
    @to_mins (self) -> int
    @to_hours (self) -> int

    // Components
    @hours_part (self) -> int     // Hours component (0-23 if < 1 day)
    @minutes_part (self) -> int   // Minutes component (0-59)
    @seconds_part (self) -> int   // Seconds component (0-59)

    // Arithmetic
    @add (self, other: Duration) -> Duration
    @sub (self, other: Duration) -> Duration
    @mul (self, factor: int) -> Duration
    @div (self, divisor: int) -> Duration

    // Checks
    @is_zero (self) -> bool
    @is_negative (self) -> bool
}
```

---

## Formatting

### Format Patterns

```ori
@format (dt: DateTime, pattern: str) -> str
@format_date (d: Date, pattern: str) -> str
@format_time (t: Time, pattern: str) -> str
```

Pattern specifiers:
| Specifier | Meaning | Example |
|-----------|---------|---------|
| `YYYY` | 4-digit year | 2024 |
| `YY` | 2-digit year | 24 |
| `MM` | 2-digit month | 03 |
| `M` | Month (no pad) | 3 |
| `DD` | 2-digit day | 05 |
| `D` | Day (no pad) | 5 |
| `HH` | 24-hour (padded) | 09 |
| `H` | 24-hour (no pad) | 9 |
| `hh` | 12-hour (padded) | 09 |
| `h` | 12-hour (no pad) | 9 |
| `mm` | Minutes | 05 |
| `ss` | Seconds | 30 |
| `SSS` | Milliseconds | 123 |
| `a` | AM/PM | PM |
| `E` | Weekday short | Mon |
| `EEEE` | Weekday full | Monday |
| `MMM` | Month short | Jan |
| `MMMM` | Month full | January |
| `Z` | Timezone offset | +0900 |
| `ZZ` | Timezone offset | +09:00 |
| `z` | Timezone abbr | JST |

Usage:
```ori
use std.time { DateTime, format }

let now = DateTime.now()
format(dt: now, pattern: "YYYY-MM-DD")        // "2024-03-15"
format(dt: now, pattern: "HH:mm:ss")          // "14:30:45"
format(dt: now, pattern: "MMMM D, YYYY")      // "March 15, 2024"
format(dt: now, pattern: "EEEE, MMM D")       // "Friday, Mar 15"
```

### ISO 8601

```ori
@to_iso8601 (dt: DateTime) -> str
@to_iso8601_date (d: Date) -> str
@to_iso8601_time (t: Time) -> str
```

Usage:
```ori
use std.time { DateTime, to_iso8601 }

let now = DateTime.now_utc()
to_iso8601(dt: now)  // "2024-03-15T14:30:45.123Z"
```

---

## Parsing

### Parse with Pattern

```ori
@parse (source: str, pattern: str, tz: Timezone = Timezone.utc()) -> Result<DateTime, TimeError>
@parse_date (source: str, pattern: str) -> Result<Date, TimeError>
@parse_time (source: str, pattern: str) -> Result<Time, TimeError>
```

The `tz` parameter specifies the timezone for the result when the pattern doesn't include timezone info. Defaults to UTC. Override with `tz: Timezone.local()` for local time.

Usage:
```ori
use std.time { parse, parse_date, Timezone }

let dt = parse(source: "2024-03-15 14:30:00", pattern: "YYYY-MM-DD HH:mm:ss")?  // UTC
let dt_local = parse(source: "2024-03-15 14:30:00", pattern: "YYYY-MM-DD HH:mm:ss", tz: Timezone.local())?
let d = parse_date(source: "March 15, 2024", pattern: "MMMM D, YYYY")?
```

### Parse ISO 8601

```ori
@from_iso8601 (source: str) -> Result<DateTime, TimeError>
@from_iso8601_date (source: str) -> Result<Date, TimeError>
```

Usage:
```ori
use std.time { from_iso8601 }

let dt = from_iso8601(source: "2024-03-15T14:30:45.123Z")?
```

---

## Error Type

```ori
type TimeError = {
    kind: TimeErrorKind,
    message: str,
}

type TimeErrorKind =
    | InvalidDate        // Feb 30, etc.
    | InvalidTime        // 25:00, etc.
    | InvalidTimezone    // Unknown timezone name
    | ParseError         // Failed to parse string
    | Overflow           // Date arithmetic overflow
```

---

## Clock Capability

Time operations that read the current time use the `Clock` capability:

```ori
trait Clock {
    @now () -> Instant
    @local_timezone () -> Timezone
}
```

This allows testing with controlled time using stateful handlers:

```ori
@test_expiration tests @is_expired () -> void = {
    let fixed_time = Instant.from_unix_secs(secs: 1700000000)

    with Clock = handler(state: fixed_time) {
        now: (s) -> (s, s)
        advance: (s, by: Duration) -> (s + by, ())
    } in {
        let token = Token { expires: fixed_time.add(duration: 1h) }
        assert(!is_expired(token: token))

        Clock.advance(by: 2h)
        assert(is_expired(token: token))
    }
}
```

> **Note:** Mock clocks use the `handler(state: expr) { ... }` construct to thread time state through operations. State is frame-local and does not require interior mutability. See `proposals/approved/stateful-mock-testing-proposal.md`.

## Errata (added 2026-02-18)

> **Superseded by stateful-mock-testing-proposal**: The original `MockClock` design used interior mutability (a runtime-provided type with special mutable state). This was replaced by the stateful handler mechanism (`handler(state: expr) { ... }`), which enables the same testing patterns while preserving value semantics. Users build their own stateful clock mocks using the handler construct instead of relying on a runtime-provided `MockClock` type.

---

## Examples

### Timestamp Logging

```ori
use std.time { DateTime, format }

@log (level: str, message: str) -> void uses Clock, Print =
    print(msg: `[{format(dt: DateTime.now(), pattern: "YYYY-MM-DD HH:mm:ss")}] [{level}] {message}`)
```

### Age Calculation

```ori
use std.time { Date }

@age (birthdate: Date) -> int uses Clock = {
    let today = Date.today()
    let age = today.year - birthdate.year

    // Adjust if birthday hasn't occurred this year
    if today.month < birthdate.month ||
       (today.month == birthdate.month && today.day < birthdate.day) then
        age - 1
    else
        age
}
```

### Business Days

```ori
use std.time { Date, Weekday }

@add_business_days (start: Date, days: int) -> Date = {
    let current = start
    let remaining = days

    loop {
        if remaining == 0 then break current

        current = current.add_days(days: 1)
        if !current.weekday().is_weekend() then
            remaining = remaining - 1
        continue
    }
}
```

### Time Until

```ori
use std.time { DateTime, Duration }

@time_until (target: DateTime) -> str uses Clock = {
    let now = DateTime.now()
    let diff = target.to_instant().diff(other: now.to_instant())

    if diff.is_negative() then
        "already passed"
    else if diff.to_hours() > 24 then
        `{diff.to_hours() / 24} days`
    else if diff.to_hours() > 0 then
        `{diff.to_hours()} hours`
    else if diff.to_mins() > 0 then
        `{diff.to_mins()} minutes`
    else
        `{diff.to_secs()} seconds`
}
```

### Timezone Conversion

```ori
use std.time { DateTime, Timezone }

@show_meeting_times (meeting_utc: DateTime, attendee_timezones: [str]) -> void uses Print =
    for tz_name in attendee_timezones do {
        let tz = Timezone.from_name(name: tz_name)?
        let local_time = meeting_utc.to_timezone(tz: tz)
        print(msg: `{tz_name}: {format(dt: local_time, pattern: "YYYY-MM-DD HH:mm")}`)
    }
```

---

## Module Structure

```ori
// std/time/mod.ori
pub use "./instant" { Instant }
pub use "./datetime" { DateTime }
pub use "./date" { Date }
pub use "./time" { Time }
pub use "./timezone" { Timezone }
pub use "./weekday" { Weekday }
pub use "./duration" { Duration }  // Re-export with extended methods
pub use "./format" { format, format_date, format_time, to_iso8601, to_iso8601_date, to_iso8601_time }
pub use "./parse" { parse, parse_date, parse_time, from_iso8601, from_iso8601_date }
pub use "./error" { TimeError, TimeErrorKind }
```

---

## Summary

| Type | Purpose |
|------|---------|
| `Instant` | UTC timestamp (for computation) |
| `DateTime` | Date + time + timezone (for display) |
| `Date` | Date only |
| `Time` | Time only |
| `Duration` | Time interval |
| `Timezone` | Timezone info |
| `Weekday` | Day of week enum |

| Operation | Function |
|-----------|----------|
| Current time | `Instant.now()`, `DateTime.now()` |
| Format | `format(dt, pattern)`, `to_iso8601(dt)` |
| Parse | `parse(source, pattern, tz)`, `from_iso8601(source)` |
| Convert timezone | `dt.to_timezone(tz)` |
| Arithmetic | `dt.add(duration)`, `d.add_days(n)` |
| Comparison | `<`, `>`, `==` via Comparable trait |
