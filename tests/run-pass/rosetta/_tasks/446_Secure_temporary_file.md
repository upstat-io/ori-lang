# Secure temporary file

**Problem:** Create a temporary file, securely and exclusively (opening it such that there are no possible race conditions). It's fine assuming local filesystem semantics (NFS or other networking filesystems can have signficantly more complicated semantics for satisfying the "no race conditions" criteria).

**Requirements:**
- Implement the task according to the specification

**Success Criteria:**
- Program produces correct output for test cases
- Implementation matches Rosetta Code specification
