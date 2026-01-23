# Range consolidation

**Problem:** Define a range of numbers R, with bounds b0 and b1 covering all numbers between and including both bounds. That range can be shown as: or equally as: Given two ranges, the act of consolidation between them compares the two ranges: Given N ranges where N > 2 then the result is the same as repeatedly replacing all combinations of two ranges by their consolidation until no further consolidation between range pairs is possible. [6.1, 7.2], [7.2, 8.3] [4, 3], [2, 1] [4, 3], [2, 1], [-1, -2], [3.

**Requirements:**
- Set consolidation
- Set of real numbers

**Success Criteria:**
- If one range covers all of the other then the result is that encompassing range.
- If the ranges touch or intersect then the result is one new single range covering the overlapping ranges.
- Otherwise the act of consolidation is to return the two non-touching ranges.
