# Operator Rules

Formal typing and evaluation rules for Ori operators.

## Legend

```
NOTATION
────────
T, U, E         type variables
v, v1, v2       values
e, e1, e2       expressions
env             type environment
|-              "entails" (type judgment)
->              type-level transformation
=>              evaluation (reduces to)
─────           inference rule separator (premises above, conclusion below)
[cond]          side condition
x               type product (left x right)

TYPE RULES
──────────
    premise1    premise2
    ────────────────────    RULE-NAME
         conclusion

READ AS: "if premise1 and premise2 hold, then conclusion holds"

EVALUATION RULES
────────────────
pattern => result [condition]

READ AS: "pattern evaluates to result when condition holds"

ASSOCIATIVITY
─────────────
assoc=left      left-to-right grouping: a op b op c = (a op b) op c
assoc=right     right-to-left grouping: a op b op c = a op (b op c)

MAINTENANCE
───────────
- Add new operators: copy existing block, modify rules
- Modify behavior: update relevant => rules
- Type changes: update -> rules and inference rules
- Keep in sync with: grammar.ebnf, ori_typeck/src/operators.rs, ori_eval/src/interpreter/mod.rs
```

---

## Pipe `|>`

```
assoc=left
prec=16 (lowest binary)

TYPE RULES
──────────
    e1 : T    f : (P: T, ...) -> U    [P is single unspecified param of f]
    ─────────────────────────────────────────────────────────────────────    PIPE-FILL
                             e1 |> f(...) : U

    e1 : T    method : (self: T, ...) -> U
    ──────────────────────────────────────    PIPE-METHOD
              e1 |> .method(...) : U

    e1 : T    g : (T) -> U
    ────────────────────────    PIPE-LAMBDA
         e1 |> g : U

UNSPECIFIED PARAMETER
─────────────────────
A parameter is "unspecified" when:
  (a) not provided in the call arguments, AND
  (b) has no default value

Parameters with defaults are treated as filled for pipe resolution.

COMPILE ERRORS
──────────────
Zero unspecified params  => "all parameters already specified; nothing for pipe to fill"
2+ unspecified params    => "ambiguous pipe target; specify all parameters except one"

DESUGARING
──────────
e1 |> f(a: v)            => { let $__pipe = e1; f(<unspecified>: __pipe, a: v) }
e1 |> .method(a: v)      => { let $__pipe = e1; __pipe.method(a: v) }
e1 |> (x -> expr)        => { let $__pipe = e1; (x -> expr)(__pipe) }
e1 |> f(a: v)?           => { let $__pipe = e1; f(<unspecified>: __pipe, a: v)? }

LEFT-TO-RIGHT: a |> f |> g |> h = h(g(f(a)))
```

---

## Coalesce `??`

```
assoc=right
prec=15

TYPE RULES
──────────
e1 : Option<T>    e2 : T
────────────────────────    COALESCE-UNWRAP
      e1 ?? e2 : T

e1 : Option<T>    e2 : Option<T>
────────────────────────────────    COALESCE-CHAIN
      e1 ?? e2 : Option<T>

e1 : Result<T,E>    e2 : T
──────────────────────────    COALESCE-RESULT-UNWRAP
      e1 ?? e2 : T

e1 : Result<T,E>    e2 : Result<T,E>
────────────────────────────────────    COALESCE-RESULT-CHAIN
      e1 ?? e2 : Result<T,E>

e1 : Never    e2 : T    [Never unifies with Option<T>]
────────────────────────────────────────────────────    COALESCE-NEVER-LEFT
                  e1 ?? e2 : T

e1 : Option<T>    e2 : Never
────────────────────────────    COALESCE-NEVER-RIGHT
      e1 ?? e2 : T

EVALUATION
──────────
Some(v) ?? e2 => v         [type(e1) != type(e1 ?? e2)]
Some(v) ?? e2 => Some(v)   [type(e1) = type(e1 ?? e2)]
None ?? e2 => eval(e2)
Ok(v) ?? e2 => v           [type(e1) != type(e1 ?? e2)]
Ok(v) ?? e2 => Ok(v)       [type(e1) = type(e1 ?? e2)]
Err(_) ?? e2 => eval(e2)

SHORT-CIRCUIT: e2 not evaluated when e1 is Some/Ok
```

---

## Arithmetic `+` `-` `*` `/` `%` `div`

```
assoc=left
prec=4 (* / % div @), prec=5 (+ -)

TYPE RULES
──────────
e1 : int    e2 : int
────────────────────    ARITH-INT
    e1 op e2 : int

e1 : float    e2 : float
────────────────────────    ARITH-FLOAT
     e1 op e2 : float

e1 : str    e2 : str
────────────────────    CONCAT
    e1 + e2 : str

e1 : Duration    e2 : Duration
──────────────────────────────    DURATION-ADD-SUB
     e1 +|- e2 : Duration

e1 : Duration    e2 : int
─────────────────────────    DURATION-MUL-DIV
    e1 *|/ e2 : Duration

e1 : int    e2 : Duration
─────────────────────────    DURATION-MUL-REV
     e1 * e2 : Duration

e1 : Duration    e2 : Duration
──────────────────────────────    DURATION-MOD
      e1 % e2 : Duration

e1 : Size    e2 : Size
──────────────────────    SIZE-ADD-SUB
    e1 +|- e2 : Size

e1 : Size    e2 : int
─────────────────────    SIZE-MUL-DIV
    e1 *|/ e2 : Size

e1 : int    e2 : Size
─────────────────────    SIZE-MUL-REV
     e1 * e2 : Size

e1 : Size    e2 : Size
──────────────────────    SIZE-MOD
     e1 % e2 : Size

EVALUATION
──────────
n1 + n2 => sum           [overflow -> panic]
n1 - n2 => diff          [overflow -> panic]
n1 * n2 => product       [overflow -> panic]
n1 / n2 => quotient      [n2 = 0 -> panic, truncates toward zero]
n1 % n2 => remainder     [n2 = 0 -> panic]
n1 div n2 => floor_quot  [n2 = 0 -> panic, floor toward -inf]
s1 + s2 => concat
```

---

## Power `**`

```
assoc=right
prec=2 (tighter than unary, looser than postfix)

TYPE RULES
──────────
e1 : int    e2 : int
────────────────────    POW-INT
    e1 ** e2 : int

e1 : float    e2 : float
──────────────────────────    POW-FLOAT
     e1 ** e2 : float

e1 : float    e2 : int
─────────────────────────    POW-FLOAT-INT
     e1 ** e2 : float

e1 : int    e2 : float
─────────────────────────    POW-INT-FLOAT
     e1 ** e2 : float

EVALUATION
──────────
n1 ** n2 => int_power    [n1 : int, n2 : int, n2 >= 0]
n1 ** n2 => panic        [n1 : int, n2 : int, n2 < 0, "negative exponent on integer"]
n1 ** 0 => 1             [for all n1, including 0 ** 0]
f1 ** f2 => libm_pow     [delegates to libm pow()]

OVERFLOW
────────
int ** int follows standard overflow behavior (panic in debug)

TRAIT DISPATCH
──────────────
** -> Pow -> power(self, rhs:)
```

---

## Comparison `==` `!=` `<` `<=` `>` `>=`

```
assoc=left
prec=8 (< <= > >=), prec=9 (== !=)

TYPE RULES
──────────
e1 : T    e2 : T    T : Eq
──────────────────────────    EQ
     e1 ==|!= e2 : bool

e1 : T    e2 : T    T : Comparable
──────────────────────────────────    ORD
      e1 <|<=|>|>= e2 : bool

EVALUATION
──────────
v == v => true
v1 == v2 => false        [v1 != v2]
v != v => false
v1 != v2 => true         [v1 != v2]
v1 < v2 => compare(v1, v2) = Less
v1 <= v2 => compare(v1, v2) != Greater
v1 > v2 => compare(v1, v2) = Greater
v1 >= v2 => compare(v1, v2) != Less
```

---

## Logical `&&` `||`

```
assoc=left
prec=13 (&&), prec=14 (||)

TYPE RULES
──────────
e1 : bool    e2 : bool
──────────────────────    AND
     e1 && e2 : bool

e1 : bool    e2 : bool
──────────────────────    OR
     e1 || e2 : bool

EVALUATION
──────────
false && e2 => false     [e2 not evaluated]
true && e2 => eval(e2)
true || e2 => true       [e2 not evaluated]
false || e2 => eval(e2)
```

---

## Bitwise `&` `|` `^` `<<` `>>`

```
assoc=left
prec=10 (&), prec=11 (^), prec=12 (|), prec=6 (<< >>)

TYPE RULES
──────────
e1 : int    e2 : int
────────────────────────    BITWISE-INT
  e1 &|^|<<|>> e2 : int

e1 : byte    e2 : byte
──────────────────────────    BITWISE-BYTE
    e1 &|^ e2 : byte

e1 : byte    e2 : int
─────────────────────────    SHIFT-BYTE
   e1 <<|>> e2 : byte

EVALUATION
──────────
n1 & n2 => bitwise_and
n1 | n2 => bitwise_or
n1 ^ n2 => bitwise_xor
n1 << n2 => shift_left   [n2 < 0 -> panic, n2 >= width -> panic]
n1 >> n2 => shift_right  [n2 < 0 -> panic, n2 >= width -> panic]

CONSTRAINTS
───────────
int width = 64 bits (valid shift: 0..63)
byte width = 8 bits (valid shift: 0..7)
```

---

## Range `..` `..=`

```
assoc=left
prec=7

TYPE RULES
──────────
e1 : int    e2 : int
────────────────────────    RANGE
   e1 ..|..= e2 : Range<int>

e1 : int
────────────────────    RANGE-OPEN
  e1 .. : Range<int>

e1 : int    e2 : int    e3 : int
────────────────────────────────    RANGE-STEP
   e1 ..|..= e2 by e3 : Range<int>
```

---

## Unary `-` `!` `~`

```
prec=3 (between power and multiplicative)

TYPE RULES
──────────
e : int
───────────    NEG-INT
  -e : int

e : float
─────────────    NEG-FLOAT
  -e : float

e : Duration
────────────────    NEG-DURATION
  -e : Duration

e : bool
────────────    NOT
  !e : bool

e : int
───────────    BITNOT-INT
  ~e : int

e : byte
────────────    BITNOT-BYTE
  ~e : byte

CONSTRAINTS
───────────
-Size -> compile error (Size cannot be negative)
```

---

## Type Conversion `as` `as?`

```
prec=1 (postfix)

TYPE RULES
──────────
e : T    T converts to U (infallible)
──────────────────────────────────────    AS-INFALLIBLE
            e as U : U

e : T    T converts to U (fallible)
────────────────────────────────────    AS-FALLIBLE
         e as? U : Option<U>

INFALLIBLE CONVERSIONS
──────────────────────
int -> float
byte -> int
char -> int

FALLIBLE CONVERSIONS
────────────────────
str -> int      (parse)
str -> float    (parse)
int -> byte     (range check)
float -> int    (truncate, range check)
```

---

## Try `?`

```
prec=1 (postfix)

TYPE RULES
──────────
e : Option<T>    enclosing returns Option<U>
────────────────────────────────────────────    TRY-OPTION
                   e? : T

e : Result<T,E>    enclosing returns Result<U,E>
────────────────────────────────────────────────    TRY-RESULT
                    e? : T

EVALUATION
──────────
Some(v)? => v
None? => return None
Ok(v)? => v
Err(e)? => return Err(e)
```

---

## Never

```
UNIFICATION
───────────
unify(Never, T) = Ok    for all T
unify(T, Never) = Ok    for all T

COERCION
────────
e : Never
─────────    NEVER-COERCE
  e : T      for all T

SOURCES
───────
panic(msg:) : Never
todo() : Never
todo(reason:) : Never
unreachable() : Never
unreachable(reason:) : Never
break : Never               [in loop]
continue : Never            [in loop]
loop { e } : Never          [no break in e]
e? : Never                  [early return path when e is None/Err]
```

---

## Precedence Table

```
PREC  OPERATORS              ASSOC   DESCRIPTION
────  ─────────              ─────   ───────────
1     . [] () ? as as?       left    postfix
2     **                     right   power
3     ! - ~                  right   unary
4     * / % div @            left    multiplicative
5     + -                    left    additive
6     << >>                  left    shift
7     .. ..= [by]            left    range (by is step modifier)
8     < > <= >=              left    comparison
9     == !=                  left    equality
10    &                      left    bitwise and
11    ^                      left    bitwise xor
12    |                      left    bitwise or
13    &&                     left    logical and
14    ||                     left    logical or
15    ??                     RIGHT   coalesce
```

---

## Trait Dispatch

```
OPERATOR -> TRAIT -> METHOD
───────────────────────────
+    -> Add      -> add(self, other:)
-    -> Sub      -> subtract(self, other:)
*    -> Mul      -> multiply(self, other:)
/    -> Div      -> divide(self, other:)
div  -> FloorDiv -> floor_divide(self, other:)
%    -> Rem      -> remainder(self, other:)
**   -> Pow      -> power(self, rhs:)
@    -> MatMul   -> matrix_multiply(self, rhs:)
-    -> Neg      -> negate(self)
!    -> Not      -> not(self)
~    -> BitNot   -> bit_not(self)
&    -> BitAnd   -> bit_and(self, other:)
|    -> BitOr    -> bit_or(self, other:)
^    -> BitXor   -> bit_xor(self, other:)
<<   -> Shl      -> shift_left(self, rhs:)
>>   -> Shr      -> shift_right(self, rhs:)
==   -> Eq       -> equals(self, other:)
<    -> Comparable -> compare(self, other:)

RESOLUTION ORDER
────────────────
1. Primitive type -> direct evaluation
2. User type -> trait method lookup
```

---

## Compound Assignment

```
DESUGARING
──────────
x op= y  =>  x = x op y    [parser-level rewrite]

SUPPORTED OPERATORS (TRAIT-BASED)
─────────────────────────────────
+=   desugars via  Add
-=   desugars via  Sub
*=   desugars via  Mul
/=   desugars via  Div
%=   desugars via  Rem
**=  desugars via  Pow
@=   desugars via  MatMul
&=   desugars via  BitAnd
|=   desugars via  BitOr
^=   desugars via  BitXor
<<=  desugars via  Shl
>>=  desugars via  Shr

SUPPORTED OPERATORS (LOGICAL)
─────────────────────────────
&&=  desugars to  x = x && y    [bool-only, short-circuit preserved]
||=  desugars to  x = x || y    [bool-only, short-circuit preserved]

CONSTRAINTS
───────────
- Left-hand side must be a mutable binding (no $ prefix)
- Compound assignment is a statement, not an expression
- Target expression is duplicated in AST (pure: no side effects)
```
