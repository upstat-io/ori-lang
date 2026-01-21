# Appendix A: Grammar Reference

This appendix provides the complete grammar for Sigil in a semi-formal notation.

---

## Notation

```
name        = definition          // rule
name        = a | b               // alternation
name        = a b                 // sequence
name        = [a]                 // optional
name        = {a}                 // zero or more
name        = (a)                 // grouping
"keyword"                         // literal keyword
'c'                               // literal character
TERMINAL                          // terminal symbol
```

---

## Program Structure

```
program     = {import} {item}

item        = function
            | type_def
            | config
            | test
            | impl_block

import      = "use" module_path [import_list]
import_list = "{" IDENT {"," IDENT} [","] "}"
            | "as" IDENT

module_path = IDENT {"." IDENT}
```

---

## Functions

```
function    = [visibility] "@" IDENT [generics] params "->" type [uses_clause] "=" expr

visibility  = "pub"

generics    = "<" generic_param {"," generic_param} ">"
generic_param = IDENT [":" bounds]

params      = "(" [param {"," param}] ")"
param       = IDENT ":" type

uses_clause = "uses" IDENT {"," IDENT}

bounds      = trait_bound {"+" trait_bound}
trait_bound = type_path
```

---

## Types

```
type_def    = [visibility] [derive] "type" IDENT [generics] "=" type_body

derive      = "#[derive(" IDENT {"," IDENT} ")]"

type_body   = struct_type
            | sum_type
            | newtype

struct_type = "{" [field {"," field}] [","] "}"
field       = IDENT ":" type

sum_type    = variant {"|" variant}
variant     = IDENT [variant_data]
variant_data = "(" [field {"," field}] ")"

newtype     = type
```

---

## Type Expressions

```
type        = type_path [type_args]
            | list_type
            | map_type
            | tuple_type
            | function_type
            | "dyn" type

type_path   = IDENT {"." IDENT}
type_args   = "<" type {"," type} ">"

list_type   = "[" type "]"
map_type    = "{" type ":" type "}"
tuple_type  = "(" type {"," type} ")"
function_type = "(" [type {"," type}] ")" "->" type
```

---

## Config Variables

```
config      = [visibility] "$" IDENT "=" literal
```

---

## Tests

```
test        = "@" IDENT "tests" "@" IDENT params "->" "void" "=" expr
```

---

## Impl Blocks

```
impl_block  = inherent_impl | trait_impl

// Inherent implementation - methods directly on a type
inherent_impl = "impl" [generics] type_path [where_clause] "{" {method} "}"

// Trait implementation - implementing a trait for a type
trait_impl  = "impl" [generics] type_path "for" type [where_clause] "{" {method} "}"

where_clause = "where" constraint {"," constraint}
constraint  = IDENT ":" bounds

method      = "@" IDENT params "->" type "=" expr
```

**Examples:**

```sigil
// Inherent impl - methods on Point
impl Point {
    @new (x: int, y: int) -> Point = Point { x, y }
    @distance (self) -> float = sqrt(...)
}

// Trait impl - Point implements Printable
impl Printable for Point {
    @to_string (self) -> str = "(" + str(self.x) + ", " + str(self.y) + ")"
}
```

---

## Traits

```
trait_def   = [visibility] "trait" IDENT [generics] [trait_bounds] "{" {trait_item} "}"

trait_bounds = ":" bounds

trait_item  = method_sig
            | assoc_type
            | default_method

method_sig  = "@" IDENT params "->" type
default_method = "@" IDENT params "->" type "=" expr
assoc_type  = "type" IDENT
```

---

## Expressions

```
expr        = for_expr
            | with_expr
            | pattern_expr
            | if_expr
            | lambda
            | binary_expr

// For expression - two forms
for_expr    = for_imperative | for_pattern

// Imperative form: side effects or building lists
for_imperative = "for" for_binding {"," for_binding} [for_guard] (do_clause | yield_clause)
for_binding = IDENT "in" expr
for_guard   = "if" expr
do_clause   = "do" expr
yield_clause = "yield" expr

// Pattern form: early exit with Ok/Err
for_pattern = "for" "(" named_args ")"
// Properties: .over, .map, .match, .default

// Loop expression - infinite loop with break/continue
loop_expr   = "loop" "(" expr ")"
break_expr  = "break"
continue_expr = "continue"

// with is a data pattern, see data_pattern below

pattern_expr = run_expr | try_expr | match_expr | data_pattern

// Sequential execution with bindings
run_expr    = "run" "(" [binding {"," binding}] "," expr ")"
binding     = "let" ["mut"] IDENT "=" expr

// Error propagation with bindings (supports ? operator)
try_expr    = "try" "(" [binding {"," binding}] "," expr ")"

// Pattern matching (see Match Expression section)
match_expr  = "match" "(" expr "," match_arms ")"

// Data and resilience patterns use named properties exclusively
data_pattern = data_pattern_name "(" named_args ")"
data_pattern_name = "map" | "filter" | "fold" | "recurse" | "collect" | "find"
                  | "parallel" | "retry" | "cache" | "validate" | "timeout" | "with"

named_args  = named_arg {"," named_arg}
named_arg   = "." IDENT ":" expr

if_expr     = "if" expr "then" expr {"else" "if" expr "then" expr} "else" expr

lambda      = lambda_params "->" expr
lambda_params = IDENT
              | "(" [IDENT {"," IDENT}] ")"
```

---

## Binary Expressions

```
binary_expr = or_expr

or_expr     = and_expr {"||" and_expr}
and_expr    = eq_expr {"&&" eq_expr}
eq_expr     = cmp_expr {("==" | "!=") cmp_expr}
cmp_expr    = range_expr {("<" | ">" | "<=" | ">=") range_expr}
range_expr  = add_expr [(".." | "..=") add_expr]
add_expr    = mul_expr {("+" | "-") mul_expr}
mul_expr    = unary_expr {("*" | "/" | "%" | "div") unary_expr}

unary_expr  = ["!" | "-"] postfix_expr

postfix_expr = primary {postfix_op}
postfix_op  = "." IDENT [call_args]
            | "[" expr "]"
            | call_args
            | ".await"
            | "?"

call_args   = "(" [expr {"," expr}] ")"
```

---

## Primary Expressions

```
primary     = literal
            | IDENT
            | "self"
            | "Self"
            | "(" expr ")"
            | list_literal
            | map_literal
            | struct_literal

list_literal = "[" [expr {"," expr}] [","] "]"
map_literal = "{" [map_entry {"," map_entry}] [","] "}"
map_entry   = expr ":" expr

struct_literal = type_path "{" [field_init {"," field_init}] [","] "}"
field_init  = IDENT [":" expr]
```

---

## Match Expression

```
match_expr  = "match" "(" expr "," match_arms ")"
match_arms  = match_arm {"," match_arm} [","]
match_arm   = guarded_pattern "->" expr

// Guard syntax: pattern followed by .match(condition)
guarded_pattern = pattern ["." "match" "(" expr ")"]

// Note: The guard expression must evaluate to bool.
// Variables bound in the pattern are in scope within the guard.
```

---

## Patterns

```
pattern     = literal_pattern
            | range_pattern
            | binding_pattern
            | wildcard_pattern
            | variant_pattern
            | struct_pattern
            | list_pattern
            | or_pattern
            | at_pattern

literal_pattern = literal
range_pattern = [literal] ".." [literal]    // e.g., 1..10, ..0, 1..
              | [literal] "..=" literal     // e.g., 1..=10, ..=0
binding_pattern = IDENT
wildcard_pattern = "_"
variant_pattern = type_path ["(" [pattern {"," pattern}] ")"]
struct_pattern = "{" [field_pattern {"," field_pattern}] [".." ] "}"
field_pattern = IDENT [":" pattern]
list_pattern = "[" [list_pattern_elem {"," list_pattern_elem}] "]"
list_pattern_elem = pattern | ".." [IDENT]
or_pattern  = pattern "|" pattern
at_pattern  = IDENT "@" pattern
```

---

## Literals

```
literal     = INT_LITERAL
            | FLOAT_LITERAL
            | STRING_LITERAL
            | BOOL_LITERAL
            | duration_literal

BOOL_LITERAL = "true" | "false"

duration_literal = INT_LITERAL duration_unit
duration_unit = "ms" | "s" | "m" | "h"
```

---

## Lexical Elements

```
IDENT       = (LETTER | '_') {LETTER | DIGIT | '_'}
INT_LITERAL = DIGIT {DIGIT | '_'}
FLOAT_LITERAL = DIGIT {DIGIT} '.' DIGIT {DIGIT} [EXPONENT]
EXPONENT    = ('e' | 'E') ['+' | '-'] DIGIT {DIGIT}
STRING_LITERAL = '"' {STRING_CHAR} '"'
STRING_CHAR = <any char except '"' or '\'> | ESCAPE
ESCAPE      = '\' ('"' | '\' | 'n' | 't' | 'r')

LETTER      = 'a'..'z' | 'A'..'Z'
DIGIT       = '0'..'9'
```

---

## Comments

```
comment     = "//" {<any char except newline>} NEWLINE
doc_comment = "//" doc_marker {<any char except newline>} NEWLINE
doc_marker  = "#" | "@param" | "@field" | "!" | ">"
```

---

## Keywords

Reserved words that cannot be used as identifiers:

```
async       do          else        false       for
if          impl        in          match       pub
self        Self        then        trait       true
type        use         uses        void        where
with        yield
```

Context-sensitive keywords (can be identifiers outside pattern context):

```
cache       collect     filter      find        fold
map         parallel    recurse     retry       run
timeout     try         validate
```

---

## Operator Precedence

From highest to lowest:

1. Postfix: `.`, `[]`, `()`, `.await`
2. Unary: `!`, `-`
3. Multiplicative: `*`, `/`, `%`, `div`
4. Additive: `+`, `-`
5. Range: `..`, `..=`
6. Comparison: `<`, `>`, `<=`, `>=`
7. Equality: `==`, `!=`
8. Logical AND: `&&`
9. Logical OR: `||`
10. Coalesce: `??`

All binary operators are left-associative.

---

## See Also

- [Basic Syntax](../02-syntax/01-basic-syntax.md)
- [Expressions](../02-syntax/02-expressions.md)
- [Capabilities](../14-capabilities/index.md) — `uses` clause and `with`...`in` expression
