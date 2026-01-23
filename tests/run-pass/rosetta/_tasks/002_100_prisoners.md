# 100 prisoners

**Problem:** Simulate the 100 prisoners problem to compare survival strategies.

**Setup:**
- 100 prisoners numbered 1-100
- 100 drawers, each containing a randomly shuffled card numbered 1-100
- Each prisoner can open maximum 50 drawers
- All prisoners must find their own number for everyone to be pardoned

**Requirements:**
- Implement two strategies:
  1. **Random:** Each prisoner randomly selects 50 drawers
  2. **Optimal:** Prisoner opens drawer matching their number, then follows the chain
- Simulate thousands of game instances with each strategy
- Calculate and display success probabilities

**Success Criteria:**
- Random strategy: ~0% success rate
- Optimal strategy: ~30-31% success rate
