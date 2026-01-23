# Abbreviations, simple

**Problem:** Validate words against a command table with explicit minimum lengths.

**Requirements:**
- Command table has commands with optional minimum abbreviation lengths
- If no number follows command, no abbreviation is permitted
- Valid abbreviation: length ≥ minimum, length ≤ full, matches prefix (case-insensitive)
- Non-alphabetic input is invalid
- Return uppercase command for valid, `*error*` for invalid

**Example:**
- Input: `riG rePEAT copies put mo rest`
- Output: `RIGHT REPEAT *error* PUT MOVE RESTORE`

**Success Criteria:**
- Correctly validates against explicit length requirements
- Handles case-insensitivity
- Rejects non-alphabetic input
