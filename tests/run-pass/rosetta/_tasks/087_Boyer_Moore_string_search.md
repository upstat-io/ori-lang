# Boyer-Moore string search

**Problem:** Implement Boyer-Moore string search algorithm for efficient pattern matching.

**Requirements:**
- Search for pattern string within text string
- Return all occurrence positions
- Implement bad character and good suffix preprocessing rules
- Match backwards from highest position to lowest
- Case-sensitive ASCII matching

**Success Criteria:**
- "TCTA" in "GCTAGCTCTACGAGTCTA" → positions [6, 14]
- "word" in "there would have been a time for such a word" → [40]
- "needle" in "needle need noodle needle" → [0, 19]
