# Map range

**Problem:** Given two ranges: Write a function/subroutine/... that takes two ranges and a real number, and returns the mapping of the real number from the first to the second range. Use this function to map values from the range [0, 10] to the range [-1, 0]. Extra credit: Show additional idiomatic ways of performing the mapping, using tools available to the language.

**Requirements:**
- [a_1,a_2] and
- [b_1,b_2];
- then a value s in range [a_1,a_2]
- is linearly mapped to a value t in range [b_1,b_2]
- t = b_1 + {(s - a_1)(b_2 - b_1) (a_2 - a_1)}

**Success Criteria:**
- Program produces correct output for test cases
- Implementation matches Rosetta Code specification
