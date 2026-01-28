# Parser v2: Formal Grammar

## Notation

This grammar uses Extended Backus-Naur Form (EBNF) with the following conventions:

| Notation | Meaning |
|----------|---------|
| `A B` | Sequence: A followed by B |
| `A \| B` | Alternative: A or B |
| `A?` | Optional: zero or one A |
| `A*` | Repetition: zero or more A |
| `A+` | Repetition: one or more A |
| `(A B)` | Grouping |
| `'x'` | Terminal: literal character or keyword |
| `"xyz"` | Terminal: literal string |
| `/* ... */` | Comment |
| `UPPER` | Token from lexer |
| `lower` | Non-terminal (grammar rule) |

## Lexical Grammar

### Whitespace and Comments

```ebnf
WHITESPACE      = ' ' | '\t' | '\r'
NEWLINE         = '\n'
LINE_COMMENT    = '//' (!NEWLINE)* NEWLINE
```

### Keywords

```ebnf
KEYWORD         = 'async' | 'break' | 'continue' | 'do' | 'else' | 'false'
                | 'for' | 'if' | 'impl' | 'in' | 'let' | 'loop' | 'match'
                | 'mut' | 'pub' | 'self' | 'Self' | 'then' | 'trait' | 'true'
                | 'type' | 'use' | 'uses' | 'void' | 'where' | 'with' | 'yield'
                | 'extension' | 'extend'
```

### Soft Keywords (Context-Sensitive)

```ebnf
/* These are only keywords in specific contexts */
SOFT_KEYWORD    = 'run' | 'try' | 'match' | 'recurse' | 'parallel'
                | 'spawn' | 'timeout' | 'cache' | 'with' | 'for' | 'catch'
```

### Reserved Built-in Names

```ebnf
/* Reserved in call position only */
BUILTIN_NAME    = 'int' | 'float' | 'str' | 'byte' | 'len' | 'is_empty'
                | 'is_some' | 'is_none' | 'is_ok' | 'is_err'
                | 'assert' | 'assert_eq' | 'assert_ne'
                | 'assert_some' | 'assert_none' | 'assert_ok' | 'assert_err'
                | 'assert_panics' | 'assert_panics_with'
                | 'compare' | 'min' | 'max' | 'print' | 'panic'
```

### Identifiers

```ebnf
IDENT           = IDENT_START IDENT_CONTINUE*
IDENT_START     = 'a'..'z' | 'A'..'Z' | '_'
IDENT_CONTINUE  = IDENT_START | '0'..'9'

/* Qualified names */
UPPER_IDENT     = 'A'..'Z' IDENT_CONTINUE*
LOWER_IDENT     = ('a'..'z' | '_') IDENT_CONTINUE*
```

### Literals

```ebnf
INT_LIT         = DECIMAL_LIT | HEX_LIT
DECIMAL_LIT     = DIGIT (DIGIT | '_')*
HEX_LIT         = '0' ('x' | 'X') HEX_DIGIT (HEX_DIGIT | '_')*
DIGIT           = '0'..'9'
HEX_DIGIT       = DIGIT | 'a'..'f' | 'A'..'F'

FLOAT_LIT       = DECIMAL_LIT '.' DECIMAL_LIT EXPONENT?
                | DECIMAL_LIT EXPONENT
EXPONENT        = ('e' | 'E') ('+' | '-')? DECIMAL_LIT

STRING_LIT      = '"' STRING_CHAR* '"'
STRING_CHAR     = !('\\' | '"' | NEWLINE) | STRING_ESCAPE
STRING_ESCAPE   = '\\' ('\\' | '"' | 'n' | 't' | 'r')

CHAR_LIT        = '\'' CHAR_CHAR '\''
CHAR_CHAR       = !(NEWLINE | '\\' | '\'') | CHAR_ESCAPE
CHAR_ESCAPE     = '\\' ('\\' | '\'' | 'n' | 't' | 'r' | '0')

BOOL_LIT        = 'true' | 'false'

DURATION_LIT    = DECIMAL_LIT DURATION_UNIT
DURATION_UNIT   = 'ms' | 's' | 'm' | 'h'

SIZE_LIT        = DECIMAL_LIT SIZE_UNIT
SIZE_UNIT       = 'b' | 'kb' | 'mb' | 'gb'
```

### Operators and Punctuation

```ebnf
/* Operators (by precedence, highest first) */
OP_DOT          = '.'
OP_QUESTION     = '?'
OP_NOT          = '!'
OP_NEG          = '-'      /* unary context */
OP_BITNOT       = '~'
OP_MUL          = '*'
OP_DIV          = '/'
OP_MOD          = '%'
OP_FLOORDIV     = 'div'
OP_ADD          = '+'
OP_SUB          = '-'      /* binary context */
OP_SHL          = '<<'
OP_SHR          = '>>'
OP_RANGE        = '..'
OP_RANGE_INC    = '..='
OP_LT           = '<'
OP_GT           = '>'
OP_LE           = '<='
OP_GE           = '>='
OP_EQ           = '=='
OP_NE           = '!='
OP_BITAND       = '&'
OP_BITXOR       = '^'
OP_BITOR        = '|'
OP_AND          = '&&'
OP_OR           = '||'
OP_COALESCE     = '??'
OP_ARROW        = '->'

/* Punctuation */
LPAREN          = '('
RPAREN          = ')'
LBRACKET        = '['
RBRACKET        = ']'
LBRACE          = '{'
RBRACE          = '}'
COMMA           = ','
COLON           = ':'
SEMICOLON       = ';'
AT              = '@'
DOLLAR          = '$'
HASH            = '#'
EQUALS          = '='
DOUBLE_COLON    = '::'
```

## Syntactic Grammar

### Module Structure

```ebnf
module          = import* item*

import          = use_import | extension_import

use_import      = 'use' import_path '{' import_list '}' NEWLINE
                | 'use' import_path 'as' IDENT NEWLINE
                | 'pub' 'use' import_path '{' import_list '}' NEWLINE

extension_import = 'extension' import_path '{' extension_list '}' NEWLINE

import_path     = relative_path | module_path
relative_path   = STRING_LIT                    /* './foo' or '../bar' */
module_path     = IDENT ('.' IDENT)*            /* std.math */

import_list     = import_item (',' import_item)* ','?
import_item     = DOUBLE_COLON? IDENT ('as' IDENT)?

extension_list  = extension_item (',' extension_item)* ','?
extension_item  = UPPER_IDENT '.' IDENT
```

### Items

```ebnf
item            = attribute* visibility? item_kind

visibility      = 'pub'

item_kind       = function_def
                | test_def
                | type_def
                | trait_def
                | impl_def
                | extend_def
                | config_def

attribute       = '#' '[' attr_name attr_args? ']'
attr_name       = IDENT
attr_args       = '(' attr_arg_list? ')'
attr_arg_list   = attr_arg (',' attr_arg)* ','?
attr_arg        = IDENT ':' literal
                | STRING_LIT
```

### Functions

```ebnf
function_def    = '@' LOWER_IDENT generics? params return_type?
                  uses_clause? where_clause? '=' expr

test_def        = '@' LOWER_IDENT test_targets? params return_type? '=' expr
test_targets    = ('tests' '@' LOWER_IDENT)+

params          = '(' param_list? ')'
param_list      = param (',' param)* ','?
param           = IDENT ':' type

return_type     = '->' type

uses_clause     = 'uses' capability_list
capability_list = capability (',' capability)*
capability      = UPPER_IDENT

where_clause    = 'where' where_bound (',' where_bound)*
where_bound     = IDENT ':' type_bound
```

### Config Variables

```ebnf
config_def      = '$' LOWER_IDENT ('(' param_list? ')' return_type)? '=' expr
```

### Type Definitions

```ebnf
type_def        = 'type' UPPER_IDENT generics? '=' type_body

type_body       = struct_body                   /* { field: Type } */
                | sum_body                      /* A | B(field: Type) */
                | newtype_body                  /* ExistingType */

struct_body     = '{' field_list? '}'
field_list      = field (',' field)* ','?
field           = IDENT ':' type

sum_body        = variant ('|' variant)*
variant         = UPPER_IDENT variant_fields?
variant_fields  = '(' field_list? ')'

newtype_body    = type                          /* Must be a type, not a variant */
```

### Traits

```ebnf
trait_def       = 'trait' UPPER_IDENT generics? trait_bounds? trait_body

trait_bounds    = ':' type_bound ('+' type_bound)*

trait_body      = '{' trait_member* '}'

trait_member    = trait_method | trait_type

trait_method    = '@' LOWER_IDENT params return_type? ('=' expr)?

trait_type      = 'type' UPPER_IDENT
```

### Implementations

```ebnf
impl_def        = 'impl' generics? impl_target impl_body

impl_target     = type                          /* inherent impl */
                | type_bound 'for' type         /* trait impl */

impl_body       = '{' impl_member* '}'

impl_member     = impl_method | impl_type

impl_method     = '@' LOWER_IDENT params return_type? '=' expr

impl_type       = 'type' UPPER_IDENT '=' type
```

### Extend Blocks

```ebnf
extend_def      = 'extend' UPPER_IDENT where_clause? extend_body
extend_body     = '{' impl_method* '}'
```

### Generics

```ebnf
generics        = '<' generic_param_list '>'
generic_param_list = generic_param (',' generic_param)* ','?
generic_param   = IDENT (':' type_bound)?

type_bound      = type ('+' type)*
```

## Expressions

### Expression Precedence (Highest to Lowest)

```ebnf
/* Precedence 1: Primary */
primary_expr    = literal
                | IDENT
                | UPPER_IDENT                   /* variant constructor */
                | 'self'
                | 'Self'
                | grouped_expr
                | list_literal
                | map_literal
                | struct_literal
                | control_expr

grouped_expr    = '(' expr ')'
                | '(' ')'                       /* unit */
                | '(' expr ',' expr_list ')'   /* tuple */

list_literal    = '[' expr_list? ']'
map_literal     = '{' map_entry_list? '}'
map_entry_list  = map_entry (',' map_entry)* ','?
map_entry       = expr ':' expr

struct_literal  = UPPER_IDENT '{' struct_field_list? '}'
struct_field_list = struct_field (',' struct_field)* ','?
struct_field    = IDENT ':' expr
                | IDENT                         /* shorthand */

/* Precedence 2: Postfix */
postfix_expr    = primary_expr postfix_op*
postfix_op      = '.' IDENT                     /* field access */
                | '.' IDENT call_args           /* method call */
                | call_args                     /* function call */
                | '[' expr ']'                  /* indexing */
                | '?'                           /* error propagation */

call_args       = '(' arg_list? ')'
arg_list        = arg (',' arg)* ','?
arg             = IDENT ':' expr                /* named argument */
                | expr                          /* positional (for lambdas/conversions) */

/* Precedence 3: Unary */
unary_expr      = unary_op* postfix_expr
unary_op        = '!' | '-' | '~'

/* Precedence 4-14: Binary (see operator table) */
binary_expr     = unary_expr (binary_op unary_expr)*

binary_op       = '*' | '/' | '%' | 'div'       /* Precedence 4 */
                | '+' | '-'                     /* Precedence 5 */
                | '<<' | '>>'                   /* Precedence 6 */
                | '..' | '..='                  /* Precedence 7 */
                | '<' | '>' | '<=' | '>='       /* Precedence 8 */
                | '==' | '!='                   /* Precedence 9 */
                | '&'                           /* Precedence 10 */
                | '^'                           /* Precedence 11 */
                | '|'                           /* Precedence 12 */
                | '&&'                          /* Precedence 13 */
                | '||'                          /* Precedence 14 */
                | '??'                          /* Precedence 15 */

/* Full expression */
expr            = binary_expr
                | lambda_expr
```

### Control Expressions

```ebnf
control_expr    = if_expr
                | loop_expr
                | for_expr
                | pattern_expr
                | with_expr
                | let_expr

if_expr         = 'if' expr 'then' expr ('else' expr)?

loop_expr       = 'loop' label? '(' expr ')'
label           = ':' IDENT

for_expr        = 'for' label? IDENT 'in' expr for_body
for_body        = 'do' expr
                | 'yield' expr
                | 'if' expr 'yield' expr

let_expr        = 'let' 'mut'? pattern (':' type)? '=' expr

with_expr       = 'with' capability_provision 'in' expr
capability_provision = UPPER_IDENT '=' expr
```

### Pattern Expressions (function_seq / function_exp)

```ebnf
pattern_expr    = run_expr
                | try_expr
                | match_expr
                | recurse_expr
                | parallel_expr
                | spawn_expr
                | timeout_expr
                | cache_expr
                | with_pattern_expr
                | for_pattern_expr
                | catch_expr
                | builtin_call

/* function_seq patterns */
run_expr        = 'run' '(' seq_binding_list ')'
try_expr        = 'try' '(' seq_binding_list ')'

seq_binding_list = seq_binding (',' seq_binding)* ','?
seq_binding     = let_expr
                | expr '?'                      /* try propagation */
                | expr

match_expr      = 'match' '(' expr ',' match_arm_list ')'
match_arm_list  = match_arm (',' match_arm)* ','?
match_arm       = pattern '->' expr

/* function_exp patterns */
recurse_expr    = 'recurse' '(' named_arg_list ')'
parallel_expr   = 'parallel' '(' named_arg_list ')'
spawn_expr      = 'spawn' '(' named_arg_list ')'
timeout_expr    = 'timeout' '(' named_arg_list ')'
cache_expr      = 'cache' '(' named_arg_list ')'
with_pattern_expr = 'with' '(' named_arg_list ')'
for_pattern_expr = 'for' '(' named_arg_list ')'
catch_expr      = 'catch' '(' named_arg_list ')'

named_arg_list  = named_arg (',' named_arg)* ','?
named_arg       = IDENT ':' expr

/* function_val (type conversions) */
builtin_call    = BUILTIN_NAME '(' expr ')'
```

### Lambda Expressions

```ebnf
lambda_expr     = lambda_params '->' lambda_body
                | typed_lambda

lambda_params   = IDENT                         /* single param */
                | '(' param_list? ')'           /* multiple params */

lambda_body     = expr

typed_lambda    = '(' param_list ')' '->' type '=' expr
```

## Patterns

```ebnf
pattern         = literal_pattern
                | binding_pattern
                | wildcard_pattern
                | variant_pattern
                | struct_pattern
                | tuple_pattern
                | list_pattern
                | range_pattern
                | or_pattern
                | at_pattern
                | guard_pattern

literal_pattern = INT_LIT | FLOAT_LIT | STRING_LIT | CHAR_LIT | BOOL_LIT

binding_pattern = IDENT

wildcard_pattern = '_'

variant_pattern = UPPER_IDENT variant_pattern_fields?
variant_pattern_fields = '(' pattern_list? ')'

struct_pattern  = '{' struct_pattern_field_list? '}'
struct_pattern_field_list = struct_pattern_field (',' struct_pattern_field)* ','?
struct_pattern_field = IDENT ':' pattern
                     | IDENT                    /* shorthand */

tuple_pattern   = '(' pattern ',' pattern_list ')'

list_pattern    = '[' list_pattern_elements? ']'
list_pattern_elements = pattern_list (',' '..' IDENT?)?
                      | '..' IDENT?

range_pattern   = literal '..' literal
                | literal '..=' literal

or_pattern      = pattern '|' pattern

at_pattern      = IDENT '@' pattern

guard_pattern   = pattern '.' 'match' '(' expr ')'

pattern_list    = pattern (',' pattern)* ','?
```

## Types

```ebnf
type            = primitive_type
                | named_type
                | generic_type
                | list_type
                | map_type
                | set_type
                | tuple_type
                | function_type
                | unit_type
                | never_type
                | dyn_type

primitive_type  = 'int' | 'float' | 'bool' | 'str' | 'char' | 'byte'

named_type      = type_path
type_path       = UPPER_IDENT ('.' UPPER_IDENT)*

generic_type    = type_path '<' type_arg_list '>'
type_arg_list   = type (',' type)* ','?

list_type       = '[' type ']'

map_type        = '{' type ':' type '}'

set_type        = 'Set' '<' type '>'

tuple_type      = '(' type ',' type_list ')'
type_list       = type (',' type)* ','?

function_type   = '(' type_list? ')' '->' type

unit_type       = '(' ')'

never_type      = 'Never'

dyn_type        = 'dyn' type_bound
```

## Operator Precedence Table

| Precedence | Operators | Associativity | Description |
|------------|-----------|---------------|-------------|
| 1 (highest) | `.` `[]` `()` `?` | Left | Access, call, propagate |
| 2 | `!` `-` `~` | Right (prefix) | Unary not, negate, bitnot |
| 3 | `*` `/` `%` `div` | Left | Multiplicative |
| 4 | `+` `-` | Left | Additive |
| 5 | `<<` `>>` | Left | Shift |
| 6 | `..` `..=` | None | Range |
| 7 | `<` `>` `<=` `>=` | Left | Comparison |
| 8 | `==` `!=` | Left | Equality |
| 9 | `&` | Left | Bitwise AND |
| 10 | `^` | Left | Bitwise XOR |
| 11 | `\|` | Left | Bitwise OR |
| 12 | `&&` | Left | Logical AND |
| 13 | `||` | Left | Logical OR |
| 14 (lowest) | `??` | Left | Coalesce |

## Grammar Notes

### Disambiguation Rules

1. **Lambda vs Tuple**: `(x)` alone is grouped expression; `(x) -> ...` is lambda
2. **Struct Literal vs Block**: In `if` condition, `{` does not start struct literal
3. **Range vs Method Call**: `1..2.method()` parses as `(1..2).method()`
4. **Soft Keywords**: `run`, `try`, `match`, etc. only keywords when followed by `(`

### Indentation Sensitivity

While not enforced by grammar, the formatter and style guide require:
- 4-space indentation
- Continuation lines indented from parent
- `run`/`try` contents on separate lines

### Reserved for Future

- `async` (marker for async functions)
- `spawn` (concurrent execution)
- `parallel` (parallel execution)
