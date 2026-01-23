# Numeric error propagation

**Problem:** If f, a, and b are values with uncertainties σf, σa, and σb, and c is a constant; then if f is derived from a, b, and c in the following ways, then σf can be calculated as follows: ;Addition/Subtraction ;Multiplication/Division ;Exponentiation This implementation of error propagation does not address issues of dependent and independent values. It is assumed that a and b are independent and so the formula for multiplication should not be applied to a*a for example.

**Requirements:**
- If f = a c, or f = c a then σf = σa
- If f = a b then σf2 = σa2 + σb2
- If f = ca or f = ac then σf = |cσa|
- If f = ab or f = a / b then σf2 = f2( (σa / a)2 + (σb / b)2)
- If f = ac then σf = |fc(σa / a)|
- Add an uncertain number type to your language that can support addition, subtraction, multiplication, division, and exponentiation between numbers with an associated error term together with 'normal' floating point numbers without an associated error term. Implement enough functionality to perform the following calculations.
- Given coordinates and their errors:x1 = 100 1.1y1 = 50 1.2x2 = 200 2.2y2 = 100 2.3 if point p1 is located at (x1, y1) and p2 is at (x2, y2); calculate the distance between the two points using the classic Pythagorean formula: d = (x1 - x2)² + (y1 - y2)²
- Print and display both d and its error.
- A Guide to Error Propagation B. Keeney, 2005.
- Propagation of uncertainty Wikipedia.

**Success Criteria:**
- Program produces correct output for test cases
- Implementation matches Rosetta Code specification
