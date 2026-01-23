# Quine

**Problem:** A quine is a self-referential program that can, without any external access, output its own source. A quine (named after Willard Van Orman Quine) is also known as: It is named after the philosopher and logician who studied self-reference and quoting in natural language, as for example in the paradox "'Yields falsehood when preceded by its quotation' yields falsehood when preceded by its quotation." "Source" has one of two meanings. It can refer to the text-based program source.

**Requirements:**
- self-replicating program or self-replicating computer program
- self-reproducing program or self-reproducing computer program
- self-copying program or self-copying computer program
- Part of the code usually needs to be stored as a string or structural literal in the language, which needs to be quoted somehow. However, including quotation marks in the string literal itself would be troublesome because it requires them to be escaped, which then necessitates the escaping character (e.g. a backslash) in the string, which itself usually needs to be escaped, and so on.
- Some languages have a function for getting the "source code representation" of a string (i.e. adds quotation marks, etc.); in these languages, this can be used to circumvent the quoting problem.
- Another solution is to construct the quote character from its character code, without having to write the quote character itself. Then the character is inserted into the string at the appropriate places. The ASCII code for double-quote is 34, and for single-quote is 39.
- Newlines in the program may have to be reproduced as newlines in the string, which usually requires some kind of escape sequence (e.g. ""). This causes the same problem as above, where the escaping character needs to itself be escaped, etc.
- If the language has a way of getting the "source code representation", it usually handles the escaping of characters, so this is not a problem.
- Some languages allow you to have a string literal that spans multiple lines, which embeds the newlines into the string without escaping.
- Write the entire program on one line, for free-form languages (as you can see for some of the solutions here, they run off the edge of the screen), thus removing the need for newlines. However, this may be unacceptable as some languages require a newline at the end of the file; and otherwise it is still generally good style to have a newline at the end of a file. (The task is not clear on whether a newline is required at the end of the file.) Some languages have a print statement that appends a newline; which solves the newline-at-the-end issue; but others do not.

**Success Criteria:**
- Program produces correct output for test cases
- Implementation matches Rosetta Code specification
