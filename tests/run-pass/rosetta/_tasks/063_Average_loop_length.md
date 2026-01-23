# Average loop length

**Problem:** Analyze the average length of sequences before repetition occurs in random mappings.

**Requirements:**
- For N from 1 to 20, simulate random mappings f: {1..N} → {1..N}
- For each mapping, find the sequence 1, f(1), f(f(1))... until a repetition occurs
- Estimate average loop length through simulation
- Calculate expected length analytically: sum from i=1 to N of [N! / (N-i)! / N^i]
- Compare simulated vs theoretical results with error percentages

**Success Criteria:**
- Display N, simulated average, analytical expected value, and % error
- Errors should be under 1% for well-tuned simulations
