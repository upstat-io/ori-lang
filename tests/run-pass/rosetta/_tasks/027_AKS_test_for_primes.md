# AKS test for primes

**Problem:** Implement AKS primality test using polynomial coefficients.

**Theory:** p is prime iff all coefficients of (x-1)^p - (x^p - 1) are divisible by p.

**Requirements:**
- Create coefficient generator for (x-1)^p expansion
- Display polynomial expansions for p = 0 to 7
- Implement primality test using coefficient divisibility
- Find all primes under 35
- Stretch: primes under 50 (needs >31-bit integers)

**Success Criteria:**
- Correct binomial coefficients
- Readable polynomial format
- Accurate prime identification
