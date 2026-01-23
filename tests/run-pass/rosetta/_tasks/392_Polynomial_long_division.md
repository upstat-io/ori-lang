# Polynomial long division

**Problem:** In algebra, polynomial long division is an algorithm for dividing a polynomial by another polynomial of the same or lower degree. Let us suppose a polynomial is represented by a vector, x (i.e., an ordered collection of coefficients) so that the ith element keeps the coefficient of x^i, and the multiplication by a monomial is a shift of the vector's elements "towards right" (injecting ones from left) followed by a multiplication of each element by the coefficient of the monomial.

**Requirements:**
- Error handling (for allocations or for wrong inputs) is not mandatory.
- Conventions can be different; in particular, note that if the first coefficient in the vector is the highest power of x for the polynomial represented by the vector, then the algorithm becomes simpler.
- Polynomial derivative

**Success Criteria:**
- Program produces correct output for test cases
- Implementation matches Rosetta Code specification
