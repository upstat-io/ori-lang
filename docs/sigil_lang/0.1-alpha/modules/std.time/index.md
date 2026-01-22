# std.time

Date, time, and timezone operations.

```sigil
use std.time { Date, Time, DateTime, now, parse_datetime }
```

**Capability required:** `Clock` (for current time operations)

---

## Overview

The `std.time` module provides:

- `Date` — Calendar date (year, month, day)
- `Time` — Time of day (hour, minute, second, nanosecond)
- `DateTime` — Combined date and time with timezone
- Parsing and formatting
- Duration arithmetic (see also: `Duration` in prelude)

---

## The Clock Capability

```sigil
trait Clock {
    @now () -> DateTime
    @today () -> Date
    @timestamp () -> int
}
```

The `Clock` capability represents the ability to read the current time. Functions that query the current time must declare `uses Clock` in their signature.

```sigil
@log_with_time (message: str) -> void uses Clock =
    print("[" + Clock.now().format("%H:%M:%S") + "] " + message)
```

> **Note:** Pure time computations (parsing, formatting, arithmetic) don't require the `Clock` capability. Only reading the *current* time requires it.

**Implementations:**

| Type | Description |
|------|-------------|
| `SystemClock` | Real system clock (default) |
| `MockClock` | Fixed or controllable time for testing |

### MockClock

For testing time-dependent code:

```sigil
type MockClock = {
    fixed_time: DateTime,
}

impl Clock for MockClock {
    @now () -> DateTime = self.fixed_time
    @today () -> Date = self.fixed_time.date
    @timestamp () -> int = self.fixed_time.timestamp()
}
```

```sigil
@test_token_expiry tests @is_expired () -> void =
    with Clock = MockClock {
        fixed_time: DateTime.parse("2024-01-15T10:00:00Z")?
    } in
    run(
        let token = Token { expires_at: DateTime.parse("2024-01-15T09:00:00Z")? },
        assert(is_expired(token)),  // Token expired 1 hour ago

        let fresh = Token { expires_at: DateTime.parse("2024-01-15T11:00:00Z")? },
        assert(not(is_expired(fresh))),  // Token valid for 1 more hour
    )
```

---

## Types

### Date

```sigil
type Date = {
    year: int,
    month: int,   // 1-12
    day: int,     // 1-31
}
```

A calendar date without time or timezone.

```sigil
use std.time { Date }

let today = Date.today()  // requires Clock capability
let date = Date.new(2024, 12, 25)

date.year       // 2024
date.month      // 12
date.day        // 25
date.weekday()  // Weekday.Wednesday
```

**Methods:**
- `new(year: int, month: int, day: int) -> Result<Date, TimeError>` — Create date
- `today() uses Clock -> Date` — Current date
- `weekday() -> Weekday` — Day of week
- `day_of_year() -> int` — Day 1-366
- `is_leap_year() -> bool` — Leap year check
- `add_days(n: int) -> Date` — Add days
- `add_months(n: int) -> Date` — Add months
- `add_years(n: int) -> Date` — Add years
- `diff_days(other: Date) -> int` — Days between dates

---

### Time

```sigil
type Time = {
    hour: int,        // 0-23
    minute: int,      // 0-59
    second: int,      // 0-59
    nanosecond: int,  // 0-999_999_999
}
```

A time of day without date or timezone.

```sigil
use std.time { Time }

let t = Time.new(14, 30, 0)
let now = Time.now()  // requires Clock capability

t.hour    // 14
t.minute  // 30
t.second  // 0
```

**Methods:**
- `new(hour: int, minute: int, second: int) -> Result<Time, TimeError>` — Create time
- `now() uses Clock -> Time` — Current time
- `add(d: Duration) -> Time` — Add duration
- `diff(other: Time) -> Duration` — Difference

---

### DateTime

```sigil
type DateTime = {
    date: Date,
    time: Time,
    offset: int,  // UTC offset in seconds
}
```

Combined date and time with timezone offset.

```sigil
use std.time { DateTime, now }

let dt = now()  // requires Clock capability
let dt = DateTime.utc_now()
let dt = DateTime.parse("2024-12-25T14:30:00Z")?

dt.date          // Date
dt.time          // Time
dt.to_utc()      // Convert to UTC
dt.to_local()    // Convert to local time
```

**Methods:**
- `now() uses Clock -> DateTime` — Current local time
- `utc_now() uses Clock -> DateTime` — Current UTC time
- `parse(s: str) -> Result<DateTime, TimeError>` — Parse ISO 8601
- `format(fmt: str) -> str` — Format to string
- `to_utc() -> DateTime` — Convert to UTC
- `to_offset(seconds: int) -> DateTime` — Change timezone
- `add(d: Duration) -> DateTime` — Add duration
- `diff(other: DateTime) -> Duration` — Difference

---

### Weekday

```sigil
type Weekday = Monday | Tuesday | Wednesday | Thursday | Friday | Saturday | Sunday
```

---

### TimeError

```sigil
type TimeError =
    | InvalidDate(year: int, month: int, day: int)
    | InvalidTime(hour: int, minute: int, second: int)
    | ParseError(str)
```

---

## Functions

### @now

```sigil
@now () uses Clock -> DateTime
```

Returns current local date and time.

```sigil
use std.time { now }

let current = now()
print("Current time: " + current.format("%Y-%m-%d %H:%M:%S"))
```

---

### @parse_datetime

```sigil
@parse_datetime (s: str) -> Result<DateTime, TimeError>
@parse_datetime (s: str, format: str) -> Result<DateTime, TimeError>
```

Parses a datetime string.

```sigil
use std.time { parse_datetime }

let dt = parse_datetime("2024-12-25T14:30:00Z")?
let dt = parse_datetime("Dec 25, 2024", "%b %d, %Y")?
```

---

### @parse_date

```sigil
@parse_date (s: str) -> Result<Date, TimeError>
@parse_date (s: str, format: str) -> Result<Date, TimeError>
```

Parses a date string.

---

## Format Specifiers

| Specifier | Meaning | Example |
|-----------|---------|---------|
| `%Y` | 4-digit year | 2024 |
| `%m` | Month (01-12) | 12 |
| `%d` | Day (01-31) | 25 |
| `%H` | Hour (00-23) | 14 |
| `%M` | Minute (00-59) | 30 |
| `%S` | Second (00-59) | 00 |
| `%b` | Month abbrev | Dec |
| `%B` | Month name | December |
| `%a` | Weekday abbrev | Wed |
| `%A` | Weekday name | Wednesday |
| `%Z` | Timezone | UTC |

---

## Duration (from prelude)

`Duration` is in the prelude but commonly used with `std.time`:

```sigil
let dt = now()
let future = dt.add(7 * 24h)  // One week later
let past = dt.add(-30 * 24h)  // 30 days ago

let diff = future.diff(dt)    // Duration
diff.as_hours()               // 168
```

---

## Examples

### Calculating age

```sigil
use std.time { Date, now }

@age (birthday: Date) uses Clock -> int = run(
    let today = now().date,
    let years = today.year - birthday.year,
    if (today.month, today.day) < (birthday.month, birthday.day) then
        years - 1
    else
        years,
)
```

### Parsing and formatting

```sigil
use std.time { parse_datetime, DateTime }

@reformat (input: str) -> Result<str, TimeError> = run(
    let dt = parse_datetime(input)?,
    Ok(dt.format("%B %d, %Y at %H:%M")),
)

reformat("2024-12-25T14:30:00Z")
// "December 25, 2024 at 14:30"
```

### Working with timezones

```sigil
use std.time { DateTime, now }

@show_times () uses Clock -> void = run(
    let local = now(),
    let utc = local.to_utc(),
    let tokyo = local.to_offset(9 * 3600),  // UTC+9

    print("Local: " + local.format("%H:%M %Z")),
    print("UTC:   " + utc.format("%H:%M %Z")),
    print("Tokyo: " + tokyo.format("%H:%M")),
)
```

---

## See Also

- [Duration](../prelude.md#duration-type) — Duration type in prelude
- [Capabilities](../../spec/14-capabilities.md) — Clock capability
