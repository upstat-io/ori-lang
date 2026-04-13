# Abbreviations, automatic

**Problem:** Find minimum abbreviation length to uniquely identify each word in a list.

**Requirements:**
- Read lines of space-separated words
- For each line, find minimum prefix length where all abbreviations are unique
- Example: "Sunday Monday Tuesday..." needs length 2 ("Su", "Mo", "Tu"... all distinct)

**Output:**
- Display minimum length (right-aligned, width 2) followed by the line
- Handle blank lines (return 0 or empty)
- Support Unicode/accented characters

**Success Criteria:**
- Correctly computes minimum unique abbreviation length per line
- Handles edge cases (empty lines, special characters)
