# Linear congruential generator

**Problem:** The linear congruential generator is a very simple example of a random number generator. All linear congruential generators use this formula: If one chooses the values of a, c and m with care, then the generator produces a uniform distribution of integers from 0 to m - 1. LCG numbers have poor quality. r_n and r_{n + 1} are not independent, as true random numbers would be. Anyone who knows r_n can predict r_{n + 1}, therefore LCG is not cryptographically secure.

**Requirements:**
- r_{n + 1} = a r_n + c m
- r_0 is a seed.
- r_1, r_2, r_3, ..., are the random numbers.
- a, c, m are constants.
- state_{n + 1} = 1103515245 state_n + 12345 }
- rand_n = state_n
- rand_n is in range 0 to 2147483647.
- state_{n + 1} = 214013 state_n + 2531011 }
- rand_n = state_n 2^{16}
- rand_n is in range 0 to 32767.

**Success Criteria:**
- Program produces correct output for test cases
- Implementation matches Rosetta Code specification
