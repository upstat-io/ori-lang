# Best shuffle

**Problem:** Shuffle string characters so as many as possible are in different positions.

**Requirements:**
- Produce a shuffle where characters are maximally displaced from original positions
- Display format: "original, shuffled, (score)"
- Score = number of positions where character did NOT change
- Randomized result preferred but deterministic acceptable

**Success Criteria:**
- Test cases: abracadabra, seesaw, elk, grrrrrr, up, a
- "tree" → "eetr" with score (0)
- "grrrrrr" → score (5) - perfect derangement impossible when >50% identical
