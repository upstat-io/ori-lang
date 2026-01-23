# URL encoding

**Problem:** Provide a function or mechanism to convert a provided string into URL encoding representation. In URL encoding, special characters, control characters and extended characters are converted into a percent symbol followed by a two digit hexadecimal code, So a space character encodes into %20 within the string.

**Requirements:**
- ASCII control codes (Character ranges 00-1F hex (0-31 decimal) and 7F (127 decimal).
- ASCII symbols (Character ranges 32-47 decimal (20-2F hex))
- ASCII symbols (Character ranges 58-64 decimal (3A-40 hex))
- ASCII symbols (Character ranges 91-96 decimal (5B-60 hex))
- ASCII symbols (Character ranges 123-126 decimal (7B-7E hex))
- Extended characters with character codes of 128 decimal (80 hex) and above.
- Lowercase escapes are legal, as in "http%3a%2f%2ffoo%20bar%2f".
- Special characters have different encodings for different standards:
- RFC 3986, Uniform Resource Identifier (URI): Generic Syntax, section 2.3, says to preserve "-._~".
- HTML 5, section 4.10.22.5 URL-encoded form data, says to preserve "-._*", and to encode space " " to "+".

**Success Criteria:**
- Program produces correct output for test cases
- Implementation matches Rosetta Code specification
