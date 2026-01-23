# Tree from nesting levels

**Problem:** Given a flat list of integers greater than zero, representing object nesting levels, e.g. [1, 2, 4], generate a tree formed from nested lists of those nesting level integers where: The generated tree data structure should ideally be in a languages nested list format that can be used for further calculations rather than something just calculated for printing. An input of [1, 2, 4] should produce the equivalent of: [1, [2, 4]] where 1 is at depth1, 2 is two deep and 4 is nested 4 deep.

**Requirements:**
- Every int appears, in order, at its depth of nesting.
- If the next level int is greater than the previous then it appears in a sub-list of the list containing the previous item
- [1, 2, 4]
- [3, 1, 3, 1]
- [1, 2, 3, 1]
- [3, 2, 1, 3]
- [3, 3, 3, 1, 1, 3, 3, 3]

**Success Criteria:**
- Program produces correct output for test cases
- Implementation matches Rosetta Code specification
