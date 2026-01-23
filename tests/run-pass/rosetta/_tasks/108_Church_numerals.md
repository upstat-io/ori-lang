# Church numerals

**Problem:** Task In the Church encoding of natural numbers, the number N is encoded by a function that applies its first argument N times to its second argument. Church zero always returns the identity function, regardless of its first argument. In other words, the first argument is not applied to the second argument at all.

**Requirements:**
- Church one applies its first argument f just once to its second argument x, yielding f(x)
- Church two applies its first argument f twice to its second argument x, yielding f(f(x))
- Arithmetic operations on natural numbers can be similarly represented as functions on Church numerals.
- In your language define:
- Church Zero,
- a Church successor function (a function on a Church numeral which returns the next Church numeral in the series),
- functions for Addition, Multiplication and Exponentiation over Church numerals,
- a function to convert integers to corresponding Church numerals,

**Success Criteria:**
- Task completed according to Rosetta Code specification
