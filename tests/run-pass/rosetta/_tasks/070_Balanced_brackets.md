# Balanced brackets

**Problem:** Generate random bracket strings and determine if they are balanced.

**Requirements:**
- Generate a string with N opening `[` and N closing `]` brackets in random order
- Determine if the string is balanced (properly nested)
- Balanced means: at no point do closing brackets exceed opening brackets
- Use counter or stack-based algorithm

**Success Criteria:**
- `[]` → balanced
- `[][]` → balanced
- `[[][][]]` → balanced
- `][` → NOT balanced
- `[]][[]` → NOT balanced
