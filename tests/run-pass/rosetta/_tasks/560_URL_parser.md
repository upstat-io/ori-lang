# URL parser

**Problem:** URLs are strings with a simple syntax: scheme://[username:password@]domain[:port]/path?query_string#fragment_id Parse a well-formed URL to retrieve the relevant information: scheme, domain, path, ... Note: this task has nothing to do with URL encoding or URL decoding. According to the standards, the characters: ! * ' ( ) ; : @ & = + $ , / ? % # [ ] only need to be percent-encoded (%) in case of possible confusion.

**Requirements:**
- Here is the official standard: https://tools.ietf.org/html/rfc3986,
- and here is a simpler BNF: http://www.w3.org/Addressing/URL/5_URI_BNF.html.
- scheme = foo
- domain = example.com
- port = :8042
- path = over/there
- query = name=ferret
- fragment = nose
- scheme = urn
- path = example:animal:ferret:nose

**Success Criteria:**
- Program produces correct output for test cases
- Implementation matches Rosetta Code specification
