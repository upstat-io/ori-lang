# Terminal control/Cursor movement

**Problem:** Demonstrate how to achieve movement of the terminal cursor: For the purpose of this task, it is not permitted to overwrite any characters or attributes on any part of the screen (so outputting a space is not a suitable solution to achieve a movement to the right). Handling of out of bounds locomotion This task has no specific requirements to trap or correct cursor movement beyond the terminal boundaries, so the implementer should decide what behavior fits best in terms of the chosen language.

**Requirements:**
- how to move the cursor one position to the left
- how to move the cursor one position to the right
- how to move the cursor up one line (without affecting its horizontal position)
- how to move the cursor down one line (without affecting its horizontal position)
- how to move the cursor to the beginning of the line
- how to move the cursor to the end of the line
- how to move the cursor to the top left corner of the screen
- how to move the cursor to the bottom right corner of the screen

**Success Criteria:**
- Program produces correct output for test cases
- Implementation matches Rosetta Code specification
