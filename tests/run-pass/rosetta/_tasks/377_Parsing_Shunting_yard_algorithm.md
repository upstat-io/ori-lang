# Parsing/Shunting-yard algorithm

**Problem:** Given the operator characteristics and input from the Shunting-yard algorithm page and tables, use the algorithm to show the changes in the operator stack and RPN output as each individual token is processed. 3 + 4 * 2 / ( 1 - 5 ) ^ 2 ^ 3 {| class="wikitable" ! operator !! precedence !! associativity !! operation |- || align="center" | ^ || 4 || right || exponentiation |- || align="center" | * || 3 || left || multiplication |- || align="center" | / || 3 || left || division |- || align="center" |

**Requirements:**
- Assume an input of a correct, space separated, string of tokens representing an infix expression
- Test with the input string:
- Operator precedence is given in this table:
- Parsing/RPN to infix conversion.

**Success Criteria:**
- Generate a space separated output string representing the RPN
- print and display the output here.
- Parsing/RPN calculator algorithm for a method of calculating a final value from this output RPN expression.
