# Tokenize a string with escaping

**Problem:** Write a function or program that can split a string at each non-escaped occurrence of a separator character. It should accept three input parameters: It should output a list of strings. Rules for splitting: Rules for escaping: Demonstrate that your function satisfies the following test-case: {| class="wikitable" | style="vertical-align:top" | {| style="border-collapse:collapse; border:none" border="0" | style="border:none; text-align:right" | string: | style="border:none" | one^|uno||three^^^^|f

**Requirements:**
- The string
- The separator character
- The escape character
- "Escaped" means preceded by an occurrence of the escape character that is not already escaped itself.
- When the escape character precedes a character that has no special meaning, it still counts as an escape (but does not do anything special).

**Success Criteria:**
- The fields that were separated by the separators, become the elements of the output list.
- Empty fields should be preserved, even at the start and end.
- Each occurrence of the escape character that was used to escape something, should not become part of the output.
