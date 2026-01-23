# Compiler/syntax analyzer

**Problem:** A Syntax analyzer transforms a token stream (from the Lexical analyzer) into a Syntax tree, based on a grammar. Take the output from the Lexical analyzer task, and convert it to an Abstract Syntax Tree (AST), based on the grammar below. The output should be in a flattened format. The program should read input from a file and/or stdin, and write output to a file and/or stdout.

**Requirements:**
- . If the language being used has a parser module/library/class, it would be great
- if two versions of the solution are provided: One without the parser module, and one
- The simple programming language to be analyzed is more or less a (very tiny) subset of
- C. The formal grammar in
- Extended Backus-Naur Form (EBNF):
- The resulting AST should be formulated as a Binary Tree.
- Example - given the simple program (below), stored in a file called while.t, create the list of tokens, using one of the Lexical analyzer solutions
- lex < while.t > while.lex

**Success Criteria:**
- Task completed according to Rosetta Code specification
