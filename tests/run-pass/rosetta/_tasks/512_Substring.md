# Substring

**Problem:** Display a substring: If the program uses UTF-8 or UTF-16, it must work on any valid Unicode code point, whether in the Basic Multilingual Plane or above it. The program must reference logical characters (code points), not 8-bit code units for UTF-8 or 16-bit code units for UTF-16. Programs for other encodings (such as 8-bit ASCII, or EUC-JP) are not required to handle all Unicode characters.

**Requirements:**
- starting from n characters in and of m length;
- starting from n characters in, up to the end of the string;
- whole string minus the last character;
- starting from a known character within the string and of m length;
- starting from a known substring within the string and of m length.

**Success Criteria:**
- Program produces correct output for test cases
- Implementation matches Rosetta Code specification
