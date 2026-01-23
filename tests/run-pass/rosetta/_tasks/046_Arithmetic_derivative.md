# Arithmetic derivative

**Problem:** Compute the Lagarias arithmetic derivative for integers based on prime factorization.

**Requirements:**
- D(0) = D(1) = 0
- D(p) = 1 for any prime p
- D(mn) = D(m)·n + m·D(n) (Leibniz/product rule)
- D(-n) = -D(n) for negative integers
- Calculate derivatives for all integers from -99 through 100
- Display results in a formatted table (10 values per row)

**Success Criteria:**
- D(6) = 5 (since 2·3 → 1·3 + 2·1)
- D(9) = 6 (since 3² → 1·3 + 3·1)
- D(27) = 27 (since 3³ → 1·9 + 3·6)
- Stretch: compute D(10^m) ÷ 7 for m from 1 to 20
