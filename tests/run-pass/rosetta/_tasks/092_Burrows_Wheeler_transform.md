# Burrows-Wheeler transform

**Problem:** Implement the Burrows-Wheeler transform for text compression preprocessing.

**Requirements:**
- Forward transform (BWT):
  - Add sentinel markers (STX at start, ETX at end)
  - Generate all cyclic rotations
  - Sort rotations lexicographically
  - Return last column of sorted table
- Inverse transform (IBWT):
  - Reconstruct original string from BWT output
- Input cannot contain STX (0x02) or ETX (0x03) characters

**Success Criteria:**
- Test strings: "banana", "appellee", "dogwood"
- Inverse transform perfectly reconstructs original input
