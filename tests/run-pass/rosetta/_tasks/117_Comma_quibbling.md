# Comma quibbling

**Problem:** Comma quibbling is a task originally set by Eric Lippert in his blog. Task Write a function to generate a string output which is the concatenation of input words from a list/sequence where: An input of no words produces the output string of just the two brace characters "". An input of just one word, e.g.

**Requirements:**
- . ["ABC"], produces the output string of the word inside the two braces, e.g. "".
- An input of two words, e.g. ["ABC", "DEF"], produces the output string of the two words inside the two braces with the words separated by the string " and ", e.g. "".
- An input of three or more words, e.g. ["ABC", "DEF", "G", "H"], produces the output string of all but the last word separated by ", " with the last word separated by " and " and all within braces
- Test your function with the following series of inputs showing your output here on this page:
- [] # (No input words).
- ["ABC", "DEF"]
- ["ABC", "DEF", "G", "H"]
- Note: Assume words are non-empty strings of uppercase characters for this task.

**Success Criteria:**
- Task completed according to Rosetta Code specification
