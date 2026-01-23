# Abbreviations, easy

**Problem:** Validate user input against a command table with abbreviation rules.

**Requirements:**
- Command table uses capitals to show minimum abbreviation (e.g., "ALTer" needs 3+ chars)
- Valid abbreviation must:
  - Be at least as long as the capital letter count
  - Match leading characters (case-insensitive)
  - Not exceed full command length
- Return full uppercase command name for valid matches
- Return `*error*` for invalid inputs

**Example:**
- Input: `riG rePEAT copies put mo rest`
- Output: `RIGHT REPEAT *error* PUT MOVE RESTORE`

**Success Criteria:**
- Correctly matches abbreviations to commands
- Handles case-insensitivity
- Returns `*error*` for non-matches
