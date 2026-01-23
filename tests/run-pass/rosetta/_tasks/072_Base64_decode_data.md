# Base64 decode data

**Problem:** Decode Base64-encoded data back to original form.

**Requirements:**
- Accept Base64-encoded input strings
- Decode to original binary or text data
- Handle padding with `=` characters
- Use standard Base64 alphabet: A-Za-z0-9+/

**Success Criteria:**
- Input: "VG8gZXJyIGlzIGh1bWFuLCBidXQgdG8gcmVhbGx5IGZvdWwgdGhpbmdzIHVwIHlvdSBuZWVkIGEgY29tcHV0ZXIuCiAgICAtLSBQYXVsIFIuIEVocmxpY2g="
- Output: "To err is human, but to really foul things up you need a computer.\n    -- Paul R. Ehrlich"
