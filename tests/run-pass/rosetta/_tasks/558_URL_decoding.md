# URL decoding

**Problem:** This task (the reverse of URL encoding and distinct from URL parser) is to provide a function or mechanism to convert an URL-encoded string into its original unencoded form. Test cases:

**Requirements:**
- Implement the task according to the specification

**Success Criteria:**
- The encoded string "http%3A%2F%2Ffoo%20bar%2F" should be reverted to the unencoded form "http://foo bar/".
- The encoded string "google.com/search?q=%60Abdu%27l-Bah%C3%A1" should revert to the unencoded form "google.com/search?q=`Abdu'l-Bahá".
- The encoded string "%25%32%35" should revert to the unencoded form "%25" and not "%".
