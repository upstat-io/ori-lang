# Circular primes

**Problem:** Definitions A circular prime is a prime number with the property that the number generated at each intermediate step when cyclically permuting its (base 10) digits will also be prime. For example: 1193 is a circular prime, since 1931, 9311 and 3119 are all also prime. Note that a number which is a cyclic permutation of a smaller circular prime is not considered to be itself a circular prime. So 13 is a circular prime, but 31 is not.

**Requirements:**
- Find the first 19 circular primes.
- If your language has access to arbitrary precision integer arithmetic, given that they are all repunits, find the next 4 circular primes.
- (Stretch) Determine which of the following repunits are probably circular primes: R(5003), R(9887), R(15073), R(25031), R(35317) and R(49081). The larger ones may take a long time to process so just do as many as you reasonably can.

**Success Criteria:**
- Program produces correct output for test cases
- Implementation matches Rosetta Code specification
