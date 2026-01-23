# Set consolidation

**Problem:** Given two sets of items then if any item is common to any set then the result of applying consolidation to those sets is a set of sets whose contents is: Given N sets of items where N>2 then the result is the same as repeatedly replacing all combinations of two sets by their consolidation until no further consolidation between set pairs is possible. If N{A,B} and {C,D} then there is no common element between the sets and the result is the same as the input.

**Requirements:**
- The two input sets if no common item exists between the two input sets of items.
- The single set that is the union of the two input sets if they share a common item.
- Range consolidation

**Success Criteria:**
- Program produces correct output for test cases
- Implementation matches Rosetta Code specification
