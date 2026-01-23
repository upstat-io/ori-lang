# Cholesky decomposition

**Problem:** Every symmetric, positive definite matrix A can be decomposed into a product of a unique lower triangular matrix L and its transpose: A = L L T }} L } is called the Cholesky factor of A } , and can be interpreted as a generalized square root of A } , as described in Cholesky decomposition.

**Requirements:**
- In a 3x3 example, we have to solve the following system of equations:
- A{aaa aa aa } {l00 l0 ll }{lll 0ll 00l} LL {lllll ll+lll+ll lll+lll+l+l}}}}
- We can see that for the diagonal elements (
- there is a calculation pattern:
- or in general:
- For the elements below the diagonal (
- ) there is also a calculation pattern:
- ={ }(a-ll)}}

**Success Criteria:**
- Task completed according to Rosetta Code specification
