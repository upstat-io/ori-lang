# Josephus problem

**Problem:** Josephus problem is a math puzzle with a grim description: n prisoners are standing on a circle, sequentially numbered from 0 to n-1. An executioner walks along the circle, starting from prisoner 0, removing every k-th prisoner and killing him. As the process goes on, the circle becomes smaller and smaller, until only one prisoner remains, who is then freed.

**Requirements:**
- You can always play the executioner and follow the procedure exactly as described, walking around the circle, counting (and cutting off) heads along the way. This would yield the complete killing sequence and answer the above questions, with a complexity of probably O(kn). However, individually it takes no more than O(m) to find out which prisoner is the m-th to die.
- If it's more convenient, you can number prisoners from 1 to n instead. If you choose to do so, please state it clearly.
- An alternative description has the people committing assisted suicide instead of being executed, and the last person simply walks away. These details are not relevant, at least not mathematically.

**Success Criteria:**
- Program produces correct output for test cases
- Implementation matches Rosetta Code specification
