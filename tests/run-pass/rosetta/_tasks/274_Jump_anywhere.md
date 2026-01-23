# Jump anywhere

**Problem:** Imperative programs like to jump around, but some languages restrict these jumps. Many structured languages restrict their conditional structures and loops to local jumps within a function. Some assembly languages limit certain jumps or branches to a small range. This task is to demonstrate a local jump and a global jump and the various other types of jumps that the language supports.

**Requirements:**
- Some languages can go to any global label in a program.
- Some languages can break multiple function calls, also known as unwinding the call stack.
- Some languages can save a continuation. The program can later continue from the same place. So you can jump anywhere, but only if you have a previous visit there (to save the continuation).

**Success Criteria:**
- Program produces correct output for test cases
- Implementation matches Rosetta Code specification
