# Topological sort

**Problem:** Given a mapping between items, and items they depend on, a topological sort orders items so that no item precedes an item it depends upon. The compiling of a library in the VHDL language has the constraint that a library must be compiled after any library it depends on. A tool exists that extracts library dependencies. Write a function that will return a valid compile order of VHDL libraries from their dependencies. Use the following data as an example: LIBRARY LIBRARY DEPENDENCIES

**Requirements:**
- Assume library names are single words.
- Items mentioned as only dependents, (sic), have no dependents of their own, but their order of compiling must be given.

**Success Criteria:**
- Any self dependencies should be ignored.
- Any un-orderable dependencies should be flagged.
