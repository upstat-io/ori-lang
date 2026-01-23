# Fractran

**Problem:** FRACTRAN is a Turing-complete esoteric programming language invented by the mathematician John Horton Conway. A FRACTRAN program is an ordered list of positive fractions P = (f_1, f_2, , f_m), together with an initial positive integer input n. The program is run by updating the integer n as follows: Conway gave a program for primes in FRACTRAN: 17/91, 78/85, 19/51, 23/38, 29/33, 77/29, 95/23, 77/19, 1/17, 11/13, 13/11, 15/14, 15/2, 55/1 Starting with n=2, this FRACTRAN program will change n to 1

**Requirements:**
- for the first fraction, f_i, in the list for which nf_i is an integer, replace n with nf_i ;
- repeat this rule until no fraction in the list produces an integer when multiplied by n, then halt.
- J. H. Conway (1987). Fractran: A Simple Universal Programming Language for Arithmetic. In: Open Problems in Communication and Computation, pages 4–26. Springer.
- J. H. Conway (2010). "FRACTRAN: A simple universal programming language for arithmetic". In Jeffrey C. Lagarias. The Ultimate Challenge: the 3x+1 problem. American Mathematical Society. pp. 249–264. ISBN 978-0-8218-4940-8. Zbl 1216.68068.
- Number Pathology: Fractran by Mark C. Chu-Carroll; October 27, 2006.

**Success Criteria:**
- Program produces correct output for test cases
- Implementation matches Rosetta Code specification
