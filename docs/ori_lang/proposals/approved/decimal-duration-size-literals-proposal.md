# Decimal Duration and Size Literals

**Status:** Approved
**Author:** Eric
**Created:** 2026-02-02
**Approved:** 2026-02-02
**Depends On:** duration-size-types-proposal.md
**Supersedes:** Size unit definition in duration-size-types-proposal.md (changes from binary to SI units)

## Summary

Allow decimal syntax in duration and size literals as compile-time sugar that converts to exact integer values in the base unit.

## Motivation

Currently, `1.5s` produces a parse error requiring users to write `1500ms`. This is unnecessarily restrictive because:

1. `0.5s` reads naturally as "half a second"
2. The conversion is exact—no precision loss
3. Other languages (CSS, Kotlin) support decimal durations
4. It's purely syntactic sugar with zero runtime cost

## Current Behavior

```ori
let t = 1.5s   // Error E0911: floating-point duration literal not supported
               // Help: use integer with smaller unit (e.g., `1500ms` instead of `1.5s`)
```

## Proposed Behavior

```ori
let t = 1.56s       // OK: exactly 1,560,000,000 nanoseconds
let t = 0.5s        // OK: exactly 500,000,000 nanoseconds
let t = 2.25h       // OK: exactly 8,100,000,000,000 nanoseconds
let s = 1.5kb       // OK: exactly 1,500 bytes
let s = 0.25mb      // OK: exactly 250,000 bytes
```

## Design

### Core Principle

Decimal duration/size literals are **compile-time sugar**. The lexer parses decimal digits and computes an exact integer result using integer arithmetic. No floating-point operations are involved. What you write is what you get—no math required.

### Parsing Rules

1. Parse integer part and decimal digits separately (as strings/digits)
2. Compute result using integer multiplication
3. If result is a whole number in base unit → valid
4. If result has a fractional remainder → error

### No Decimal Place Limit

There is no artificial limit on decimal places. The natural constraint is whether the result is a whole number:

```ori
1.123456789s   // OK: 1,123,456,789 ns (9 decimal places, still whole)
1.1234567890s  // Error: 10th decimal = 0.1ns, not representable
1.5ns          // Error: 1.5 nanoseconds not whole
```

### Base Units

- **Duration:** nanoseconds (ns)
- **Size:** bytes (b)

### Size Units: SI (Decimal)

Size units use powers of **1000** (SI/decimal), not 1024 (binary):

| Unit | Value |
|------|-------|
| `kb` | 1,000 bytes |
| `mb` | 1,000,000 bytes |
| `gb` | 1,000,000,000 bytes |
| `tb` | 1,000,000,000,000 bytes |

This ensures decimal literals are pure syntactic sugar—what you write is what you get:

- `1.5kb` = 1,500 bytes (obvious, no math)
- `0.5mb` = 500,000 bytes (obvious, no math)
- `2.5gb` = 2,500,000,000 bytes (obvious, no math)

> **Note:** This supersedes the binary (1024-based) unit definition in `duration-size-types-proposal.md`. Users needing exact powers of 1024 should use explicit byte counts: `1024b`, `1048576b`, etc.

## Implementation (Informative)

> **Note:** This section describes a possible implementation approach and is not normative. Compilers may use any approach that produces the same observable behavior.

### Lexer Approach

Instead of producing error tokens for decimal literals, the lexer parses them directly:

```rust
// Single pass, no allocations, no floats
fn parse_decimal_duration(slice: &str, unit: DurationUnit) -> Result<u64, ParseError> {
    let multiplier = unit.to_nanos_multiplier(); // e.g., 1_000_000_000 for seconds
    let mut result: u64 = 0;
    let mut decimal_divisor: u64 = 1;
    let mut in_fraction = false;
    let mut has_fraction = false;

    for byte in slice.bytes() {
        match byte {
            b'0'..=b'9' => {
                let digit = (byte - b'0') as u64;
                if in_fraction {
                    decimal_divisor *= 10;
                    // Accumulate: digit * multiplier / decimal_divisor
                    // Check for non-whole result
                    let contribution = digit * multiplier;
                    if contribution % decimal_divisor != 0 {
                        return Err(/* not representable as whole nanoseconds */);
                    }
                    result += contribution / decimal_divisor;
                } else {
                    result = result * 10 + digit;
                }
            }
            b'.' => {
                in_fraction = true;
                has_fraction = true;
                result *= multiplier; // Apply multiplier to integer part
            }
            b'_' => {} // skip underscores
            _ => break, // hit unit suffix
        }
    }

    // If no fraction, apply multiplier now
    if !has_fraction {
        result *= multiplier;
    }

    Ok(result)
}
```

### Token Changes

Remove error token types:
- `FloatDurationError` → remove
- `FloatSizeError` → remove

Duration/Size tokens store the computed value in base units:
```rust
TokenKind::Duration(1_500_000_000)  // nanoseconds
TokenKind::Size(1_500)              // bytes
```

### Error Messages

When decimal result is not whole:

```
error[E0911]: duration literal cannot be represented exactly
 --> src/main.ori:1:9
  |
1 | let t = 1.5ns
  |         ^^^^^ 1.5 nanoseconds is not a whole number
  |
  = help: nanoseconds is the smallest unit; use an integer value
```

> **Note:** E0911 is repurposed from "floating-point duration literal not supported" to "duration/size literal cannot be represented exactly".

## Examples

### Valid

```ori
0.5s           // 500,000,000 nanoseconds
1.56s          // 1,560,000,000 nanoseconds
2.25m          // 135,000,000,000 nanoseconds (2m 15s)
0.001s         // 1,000,000 nanoseconds (1ms)
1.5kb          // 1,500 bytes
0.25mb         // 250,000 bytes
1.123456789s   // 1,123,456,789 nanoseconds
```

### Invalid

```ori
1.5ns          // Error: 1.5 nanoseconds not whole
0.5b           // Error: 0.5 bytes not whole
1.0000000001s  // Error: result has sub-nanosecond precision
```

## Comparison with Other Languages

| Language | Approach | Precision |
|----------|----------|-----------|
| **Ori** | Compile-time decimal→integer | Exact |
| Kotlin | Runtime float conversion | Rounded |
| CSS | Parsed as decimal | Exact |
| Go | No literal syntax | N/A |

Ori's approach is most similar to CSS but with explicit whole-number validation.

## Alternatives Considered

### 1. Keep Current Behavior (Error on Decimals)

Rejected: Unnecessarily restrictive for a common use case.

### 2. Use Floating-Point Conversion

Rejected: Introduces precision issues and platform-dependent rounding.

### 3. Limit to 2 Decimal Places

Rejected: Arbitrary restriction. Natural constraints (whole number result) are sufficient.

### 4. Keep Binary Units for Size

Rejected: With binary units (1024), `1.5kb` = 1,536 bytes, which requires the user to compute the result. Decimal literals should be pure syntactic sugar—what you write is what you get, no math required.

### 5. Both SI and Binary Notations (kb/kib)

Rejected: Adds complexity. Users needing exact powers of 1024 can use byte literals.

## Documentation Updates

### Spec Changes

1. **`spec/06-types.md` § Duration:**
   - Add decimal literal syntax
   - Clarify that decimals are compile-time sugar

2. **`spec/06-types.md` § Size:**
   - Add decimal literal syntax
   - Change units from binary (1024) to SI (1000)

3. **`spec/03-lexical-elements.md` § Literals:**
   - Add decimal duration/size literal grammar

4. **`grammar.ebnf`:**
   - Update literal productions

### CLAUDE.md Changes

Update Duration/Size examples to show decimal syntax:
- `1.5s`, `0.5kb` examples
- Note: "decimal syntax, not float"
- Update Size units from 1024 to 1000

## References

- Original proposal: `proposals/approved/duration-size-types-proposal.md`
- Kotlin Duration: https://kotlinlang.org/api/latest/jvm/stdlib/kotlin.time/-duration/
- CSS time values: https://developer.mozilla.org/en-US/docs/Web/CSS/time
