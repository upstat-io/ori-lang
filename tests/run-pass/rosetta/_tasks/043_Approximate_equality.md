# Approximate equality

**Problem:** Determine whether two floating-point numbers are approximately equal, accounting for differences in magnitude.

**Requirements:**
- Create a function that returns true/false for approximate equality
- Allow for magnitude-based tolerance (relative comparison, not fixed decimals)
- Large numbers with small relative differences should be approximately equal
- Smaller numbers require stricter comparisons

**Success Criteria:**
- `100000000000000.01 ≈ 100000000000000.011` → true
- `100.01 ≈ 100.011` → false
- `sqrt(2) * sqrt(2) ≈ 2.0` → true
- `-sqrt(2) * sqrt(2) ≈ -2.0` → true
