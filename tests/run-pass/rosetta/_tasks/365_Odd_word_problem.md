# Odd word problem

**Problem:** Write a program that solves the odd word problem with the restrictions given below. Description: You are promised an input stream consisting of English letters and punctuations. It is guaranteed that: A stream with six words: what,is,the;meaning,of:life. The task is to reverse the letters in every other word while leaving punctuations intact, producing: what,si,the;gninaem,of:efil.

**Requirements:**
- the words (sequence of consecutive letters) are delimited by one and only one punctuation,
- the stream will begin with a word,
- the words will be at least one letter long, and
- a full stop (a period, [.]) appears after, and only after, the last word.
- Only I/O allowed is reading or writing one character at a time, which means: no reading in a string, no peeking ahead, no pushing characters back into the stream, and no storing characters in a global variable for later use;
- You are not to explicitly save characters in a collection data structure, such as arrays, strings, hash tables, etc, for later reversal;
- You are allowed to use recursions, closures, continuations, threads, co-routines, etc., even if their use implies the storage of multiple characters.

**Success Criteria:**
- Program produces correct output for test cases
- Implementation matches Rosetta Code specification
