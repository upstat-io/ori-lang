# Balanced ternary

**Problem:** Implement balanced ternary numbers where digits can be 1, 0, or -1.

**Requirements:**
- Support arbitrarily large integers (positive and negative)
- Convert to/from text using '+', '-', and '0' characters
- Convert to/from native integers with overflow detection
- Implement addition, negation, and multiplication directly (without converting to native integers)

**Success Criteria:**
- a = "+−0++0+" (523 in decimal)
- b = -436
- c = "+−++−" (65 in decimal)
- a × (b − c) = -262023 = "----0+--0++0"
