# Take notes on the command line

**Problem:** Invoking NOTES without commandline arguments displays the current contents of the local NOTES.TXT if it exists. If NOTES has arguments, the current date and time are appended to the local NOTES.TXT followed by a newline. Then all the arguments, joined with spaces, prepended with a tab, and appended with a trailing newline, are written to NOTES.TXT. If NOTES.TXT doesn't already exist in the current directory then a new NOTES.TXT file should be created.

**Requirements:**
- If NOTES.TXT doesn't already exist in the current directory then a new NOTES.TXT file should be created.

**Success Criteria:**
- Program produces correct output for test cases
- Implementation matches Rosetta Code specification
