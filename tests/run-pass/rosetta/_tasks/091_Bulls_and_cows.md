# Bulls and cows

**Problem:** Create a number guessing game with positional feedback.

**Requirements:**
- Generate random 4-digit code from digits 1-9 with no duplicates
- Accept player guesses and validate input (4 digits, 1-9, no duplicates)
- Score each guess:
  - Bull: correct digit in correct position
  - Cow: correct digit in wrong position
- Continue until player achieves 4 bulls (exact match)

**Success Criteria:**
- Game rejects malformed guesses
- Correct scoring of bulls and cows
- Game ends when secret is guessed
