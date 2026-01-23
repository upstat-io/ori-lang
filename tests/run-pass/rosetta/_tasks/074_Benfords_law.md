# Benford's law

**Problem:** Calculate distribution of first significant digits and compare to Benford's Law predictions.

**Requirements:**
- Count how often each digit (1-9) appears as leading digit in a dataset
- Apply Benford's Law formula: P(d) = log₁₀(1 + 1/d)
- Use first 1,000 Fibonacci numbers as primary dataset
- Display actual vs expected distribution

**Success Criteria:**
- Digit 1: ~30% (actual) vs 30.1% (expected)
- Digit 9: ~4.5% (actual) vs 4.6% (expected)
- Fibonacci numbers closely follow Benford's Law
