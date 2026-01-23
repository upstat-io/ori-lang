# Rep-string

**Problem:** Given a series of ones and zeroes in a string, define a repeated string or rep-string as a string which is created by repeating a substring of the first N characters of the string truncated on the right to the length of the input string, and in which the substring appears repeated at least twice in the original. For example, the string 10011001100 is a rep-string as the leftmost four characters of 1001 are repeated three times and truncated on the right to give the original string.

**Requirements:**
- There may be multiple sub-strings that make a string a rep-string - in that case an indication of all, or the longest, or the shortest would suffice.
- Use the function to indicate the repeating substring if any, in the following:

**Success Criteria:**
- Write a function/subroutine/method/... that takes a string and returns an indication of if it is a rep-string and the repeated string. (Either the string that is repeated, or the number of repeated characters would suffice).
- Show your output on this page.
