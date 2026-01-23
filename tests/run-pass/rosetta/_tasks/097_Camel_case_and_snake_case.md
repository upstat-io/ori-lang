# Camel case and snake case

**Problem:** Convert strings between camelCase and snake_case naming conventions.

**Requirements:**
- snake_case: all lowercase with underscores between words
- camelCase: lowercase first letter, capitalize initial letters of subsequent words
- Handle spaces and hyphens as word separators
- Ignore leading/trailing whitespace
- Handle non-alphanumeric characters appropriately

**Success Criteria:**
- "snakeCase" → "snake_case" (to snake)
- "snake_case" → "snakeCase" (to camel)
- "variable_10_case" and "variable10Case" handled correctly
- "hurry-up-joe!" converts properly
