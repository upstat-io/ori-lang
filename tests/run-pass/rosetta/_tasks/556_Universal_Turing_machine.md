# Universal Turing machine

**Problem:** One of the foundational mathematical constructs behind computer science is the universal Turing Machine. (Alan Turing introduced the idea of such a machine in 1936–1937.) Indeed one way to definitively prove that a language is turing-complete is to implement a universal Turing machine in it. Simulate such a machine capable of taking the definition of any other Turing machine and executing it. Of course, you will not have an infinite tape, but you should emulate this as much as is possible.

**Requirements:**
- States: q0, qf
- Initial state: q0
- Terminating states: qf
- Permissible symbols: B, 1
- Blank symbol: B
- Rules:
- States: a, b, c, halt
- Initial state: a
- Terminating states: halt
- Permissible symbols: 0, 1

**Success Criteria:**
- Program produces correct output for test cases
- Implementation matches Rosetta Code specification
