# Aliquot sequence classifications

**Problem:** Classify aliquot sequences by their termination/repetition patterns.

**Sequence:** Each term = sum of proper divisors of previous term.

**Classifications:**
1. Terminating - reaches 0
2. Perfect - returns to start immediately (period 1)
3. Amicable - returns to start on 3rd term (period 2)
4. Sociable - returns to start after N>3 terms
5. Aspiring - settles into repeating non-start number
6. Cyclic - enters cycle with non-start number
7. Non-terminating - doesn't classify after 16 terms or exceeds 2^47

**Test:** Numbers 1-10, plus 11, 12, 28, 496, 220, 1184, 12496, 1264460, 790, 909, 562, 1064, 1488

**Success Criteria:** Show classification and full sequence for each number
