# 24 game

**Problem:** Implement the 24 Game where players form expressions equaling 24.

**Requirements:**
- Randomly choose and display four digits (1-9, repetitions allowed)
- Player enters an arithmetic expression using those digits
- Validate the expression:
  - Must use all four digits exactly once
  - No forming multi-digit numbers (can't combine 1,2 into 12)
  - Only +, -, *, / operators allowed
  - Parentheses permitted
- Use floating point division (preserve remainders)
- Evaluate and check if result equals 24

**Success Criteria:**
- Correctly validates digit usage
- Properly evaluates expressions
- Accepts valid solutions, rejects invalid ones
- Note: Program validates player input, does NOT generate solutions
