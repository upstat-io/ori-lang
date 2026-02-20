//! AOT Trait and Method Codegen Tests
//!
//! End-to-end tests verifying that trait methods, impl methods, and built-in
//! method dispatch produce correct native code through the LLVM backend.
//!
//! Covers roadmap Section 3 items:
//! - 3.0: Core library traits (Len, `IsEmpty`, Option, Result, Comparable, Eq)
//! - 3.1: Trait declarations (default methods)
//! - 3.2: Trait implementations (inherent impl, trait impl, method resolution)
//! - 3.14: Comparable/Hashable for compound types (Option, Result, Tuple, List)
//! - 3.21: Operator traits (user-defined +, -, *, /, %, //, &, |, ^, <<, >>)

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

// 3.0.1: Len Trait — .len() codegen

#[test]
fn test_aot_list_len_basic() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = [10, 20, 30];
    let n = xs.len();
    if n == 3 then 0 else 1
}
"#,
        "list_len_basic",
    );
}

#[test]
fn test_aot_list_len_empty() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs: [int] = [];
    let n = xs.len();
    if n == 0 then 0 else 1
}
"#,
        "list_len_empty",
    );
}

#[test]
fn test_aot_list_len_single() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = [42];
    if xs.len() == 1 then 0 else 1
}
"#,
        "list_len_single",
    );
}

#[test]
fn test_aot_string_len() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s = "hello";
    if s.len() == 5 then 0 else 1
}
"#,
        "string_len",
    );
}

#[test]
fn test_aot_string_len_empty() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s = "";
    if s.len() == 0 then 0 else 1
}
"#,
        "string_len_empty",
    );
}

// 3.0.2: IsEmpty Trait — .is_empty() codegen

#[test]
fn test_aot_list_is_empty_true() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs: [int] = [];
    if xs.is_empty() then 0 else 1
}
"#,
        "list_is_empty_true",
    );
}

#[test]
fn test_aot_list_is_empty_false() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = [1, 2];
    if xs.is_empty() then 1 else 0
}
"#,
        "list_is_empty_false",
    );
}

#[test]
fn test_aot_string_is_empty_true() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s = "";
    if s.is_empty() then 0 else 1
}
"#,
        "string_is_empty_true",
    );
}

#[test]
fn test_aot_string_is_empty_false() {
    assert_aot_success(
        r#"
@main () -> int = {
    let s = "hi";
    if s.is_empty() then 1 else 0
}
"#,
        "string_is_empty_false",
    );
}

// 3.0.3: Option Methods — .is_some(), .is_none(), .unwrap(), .unwrap_or() codegen

#[test]
fn test_aot_option_is_some_true() {
    assert_aot_success(
        r#"
@get () -> Option<int> = Some(42)

@main () -> int = {
    let o = get();
    if o.is_some() then 0 else 1
}
"#,
        "option_is_some_true",
    );
}

#[test]
fn test_aot_option_is_some_false() {
    assert_aot_success(
        r#"
@get () -> Option<int> = None

@main () -> int = {
    let o = get();
    if o.is_some() then 1 else 0
}
"#,
        "option_is_some_false",
    );
}

#[test]
fn test_aot_option_is_none_true() {
    assert_aot_success(
        r#"
@get () -> Option<int> = None

@main () -> int = {
    let o = get();
    if o.is_none() then 0 else 1
}
"#,
        "option_is_none_true",
    );
}

#[test]
fn test_aot_option_is_none_false() {
    assert_aot_success(
        r#"
@get () -> Option<int> = Some(7)

@main () -> int = {
    let o = get();
    if o.is_none() then 1 else 0
}
"#,
        "option_is_none_false",
    );
}

#[test]
fn test_aot_option_unwrap_some() {
    assert_aot_success(
        r#"
@get () -> Option<int> = Some(42)

@main () -> int = {
    let o = get();
    let v = o.unwrap();
    if v == 42 then 0 else 1
}
"#,
        "option_unwrap_some",
    );
}

#[test]
fn test_aot_option_unwrap_or_some() {
    assert_aot_success(
        r#"
@get () -> Option<int> = Some(42)

@main () -> int = {
    let o = get();
    let v = o.unwrap_or(default: 0);
    if v == 42 then 0 else 1
}
"#,
        "option_unwrap_or_some",
    );
}

#[test]
fn test_aot_option_unwrap_or_none() {
    assert_aot_success(
        r#"
@get () -> Option<int> = None

@main () -> int = {
    let o = get();
    let v = o.unwrap_or(default: 99);
    if v == 99 then 0 else 1
}
"#,
        "option_unwrap_or_none",
    );
}

// 3.0.4: Result Methods — .is_ok(), .is_err(), .unwrap() codegen

#[test]
fn test_aot_result_is_ok_true() {
    assert_aot_success(
        r#"
@get () -> Result<int, str> = Ok(42)

@main () -> int = {
    let r = get();
    if r.is_ok() then 0 else 1
}
"#,
        "result_is_ok_true",
    );
}

#[test]
fn test_aot_result_is_ok_false() {
    assert_aot_success(
        r#"
@get () -> Result<int, str> = Err("bad")

@main () -> int = {
    let r = get();
    if r.is_ok() then 1 else 0
}
"#,
        "result_is_ok_false",
    );
}

#[test]
fn test_aot_result_is_err_true() {
    assert_aot_success(
        r#"
@get () -> Result<int, str> = Err("bad")

@main () -> int = {
    let r = get();
    if r.is_err() then 0 else 1
}
"#,
        "result_is_err_true",
    );
}

#[test]
fn test_aot_result_is_err_false() {
    assert_aot_success(
        r#"
@get () -> Result<int, str> = Ok(42)

@main () -> int = {
    let r = get();
    if r.is_err() then 1 else 0
}
"#,
        "result_is_err_false",
    );
}

#[test]
fn test_aot_result_unwrap_ok() {
    assert_aot_success(
        r#"
@get () -> Result<int, str> = Ok(42)

@main () -> int = {
    let r = get();
    let v = r.unwrap();
    if v == 42 then 0 else 1
}
"#,
        "result_unwrap_ok",
    );
}

// 3.0.5: Comparable Trait — .compare() codegen

#[test]
fn test_aot_int_compare_less() {
    assert_aot_success(
        r#"
@main () -> int = {
    let ord = 1.compare(other: 5);
    if ord.is_less() then 0 else 1
}
"#,
        "int_compare_less",
    );
}

#[test]
fn test_aot_int_compare_equal() {
    assert_aot_success(
        r#"
@main () -> int = {
    let ord = 7.compare(other: 7);
    if ord.is_equal() then 0 else 1
}
"#,
        "int_compare_equal",
    );
}

#[test]
fn test_aot_int_compare_greater() {
    assert_aot_success(
        r#"
@main () -> int = {
    let ord = 10.compare(other: 3);
    if ord.is_greater() then 0 else 1
}
"#,
        "int_compare_greater",
    );
}

#[test]
fn test_aot_ordering_reverse() {
    assert_aot_success(
        r#"
@main () -> int = {
    let ord = 1.compare(other: 5);
    let rev = ord.reverse();
    if rev.is_greater() then 0 else 1
}
"#,
        "ordering_reverse",
    );
}

#[test]
fn test_aot_ordering_predicates() {
    assert_aot_success(
        r#"
@main () -> int = {
    let less = 1.compare(other: 2);
    let equal = 3.compare(other: 3);
    let greater = 5.compare(other: 1);
    let ok1 = less.is_less() && !less.is_equal() && !less.is_greater();
    let ok2 = !equal.is_less() && equal.is_equal() && !equal.is_greater();
    let ok3 = !greater.is_less() && !greater.is_equal() && greater.is_greater();
    if ok1 && ok2 && ok3 then 0 else 1
}
"#,
        "ordering_predicates",
    );
}

#[test]
fn test_aot_ordering_is_less_or_equal() {
    assert_aot_success(
        r#"
@main () -> int = {
    let less = 1.compare(other: 2);
    let equal = 3.compare(other: 3);
    let greater = 5.compare(other: 1);
    let ok1 = less.is_less_or_equal();
    let ok2 = equal.is_less_or_equal();
    let ok3 = !greater.is_less_or_equal();
    if ok1 && ok2 && ok3 then 0 else 1
}
"#,
        "ordering_is_less_or_equal",
    );
}

#[test]
fn test_aot_ordering_is_greater_or_equal() {
    assert_aot_success(
        r#"
@main () -> int = {
    let less = 1.compare(other: 2);
    let equal = 3.compare(other: 3);
    let greater = 5.compare(other: 1);
    let ok1 = !less.is_greater_or_equal();
    let ok2 = equal.is_greater_or_equal();
    let ok3 = greater.is_greater_or_equal();
    if ok1 && ok2 && ok3 then 0 else 1
}
"#,
        "ordering_is_greater_or_equal",
    );
}

// 3.0.6: Eq Trait — == and != codegen (explicit coverage)

#[test]
fn test_aot_eq_int() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = 42;
    let b = 42;
    let c = 99;
    let eq = a == b;
    let ne = a != c;
    if eq && ne then 0 else 1
}
"#,
        "eq_int",
    );
}

#[test]
fn test_aot_eq_bool() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = true;
    let b = true;
    let c = false;
    let eq = a == b;
    let ne = a != c;
    if eq && ne then 0 else 1
}
"#,
        "eq_bool",
    );
}

#[test]
fn test_aot_eq_string() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = "hello";
    let b = "hello";
    let c = "world";
    let eq = a == b;
    let ne = a != c;
    if eq && ne then 0 else 1
}
"#,
        "eq_string",
    );
}

// 3.2: Trait Implementations — Inherent impl codegen

#[test]
fn test_aot_inherent_impl_method() {
    assert_aot_success(
        r#"
type Point = { x: int, y: int }

impl Point {
    @get_x (self) -> int = self.x
    @get_y (self) -> int = self.y
    @sum (self) -> int = self.x + self.y
}

@main () -> int = {
    let p = Point { x: 10, y: 20 };
    let ok1 = p.get_x() == 10;
    let ok2 = p.get_y() == 20;
    let ok3 = p.sum() == 30;
    if ok1 && ok2 && ok3 then 0 else 1
}
"#,
        "inherent_impl_method",
    );
}

#[test]
fn test_aot_inherent_impl_with_params() {
    assert_aot_success(
        r#"
type Counter = { value: int }

impl Counter {
    @add (self, n: int) -> int = self.value + n
    @is_above (self, threshold: int) -> bool = self.value > threshold
}

@main () -> int = {
    let c = Counter { value: 10 };
    let ok1 = c.add(n: 5) == 15;
    let ok2 = c.is_above(threshold: 5);
    let ok3 = !c.is_above(threshold: 15);
    if ok1 && ok2 && ok3 then 0 else 1
}
"#,
        "inherent_impl_with_params",
    );
}

// 3.2: Trait Implementations — Trait impl codegen

#[test]
fn test_aot_trait_impl_method() {
    assert_aot_success(
        r#"
trait Describable {
    @describe (self) -> str
}

type Widget = { name: str }

impl Describable for Widget {
    @describe (self) -> str = self.name
}

@main () -> int = {
    let w = Widget { name: "button" };
    let d = w.describe();
    if d == "button" then 0 else 1
}
"#,
        "trait_impl_method",
    );
}

#[test]
fn test_aot_trait_impl_multiple_methods() {
    assert_aot_success(
        r#"
trait Calculator {
    @add (self, n: int) -> int
    @double (self) -> int
}

type Num = { value: int }

impl Calculator for Num {
    @add (self, n: int) -> int = self.value + n
    @double (self) -> int = self.value * 2
}

@main () -> int = {
    let n = Num { value: 5 };
    let ok1 = n.add(n: 3) == 8;
    let ok2 = n.double() == 10;
    if ok1 && ok2 then 0 else 1
}
"#,
        "trait_impl_multiple_methods",
    );
}

// 3.1: Trait Declarations — Default method codegen

#[test]
fn test_aot_trait_default_method() {
    assert_aot_success(
        r#"
trait Summarizable {
    @name (self) -> str
    @summary (self) -> str = "Item: " + self.name()
}

type Item = { label: str }

impl Summarizable for Item {
    @name (self) -> str = self.label
}

@main () -> int = {
    let item = Item { label: "widget" };
    let s = item.summary();
    if s == "Item: widget" then 0 else 1
}
"#,
        "trait_default_method",
    );
}

// 3.2: Method resolution — inherent methods take priority over trait methods

#[test]
fn test_aot_method_resolution_inherent_over_trait() {
    assert_aot_success(
        r#"
trait Greetable {
    @greet (self) -> str
}

type Person = { name: str }

impl Person {
    @greet (self) -> str = "Hi, I'm " + self.name
}

impl Greetable for Person {
    @greet (self) -> str = "Hello from " + self.name
}

@main () -> int = {
    let p = Person { name: "Alice" };
    let g = p.greet();
    if g == "Hi, I'm Alice" then 0 else 1
}
"#,
        "method_resolution_inherent_over_trait",
    );
}

// 3.2: User-defined impl method dispatch — struct field access in methods

#[test]
fn test_aot_impl_method_field_access() {
    assert_aot_success(
        r#"
type Rect = { width: int, height: int }

impl Rect {
    @area (self) -> int = self.width * self.height
    @perimeter (self) -> int = 2 * (self.width + self.height)
    @is_square (self) -> bool = self.width == self.height
}

@main () -> int = {
    let r = Rect { width: 3, height: 4 };
    let ok1 = r.area() == 12;
    let ok2 = r.perimeter() == 14;
    let ok3 = !r.is_square();
    let sq = Rect { width: 5, height: 5 };
    let ok4 = sq.is_square();
    if ok1 && ok2 && ok3 && ok4 then 0 else 1
}
"#,
        "impl_method_field_access",
    );
}

// 3.2: Multiple impl blocks on same type

#[test]
fn test_aot_multiple_impl_blocks() {
    assert_aot_success(
        r#"
trait Printable {
    @to_str (self) -> str
}

type Color = { r: int, g: int, b: int }

impl Color {
    @brightness (self) -> int = (self.r + self.g + self.b) / 3
}

impl Printable for Color {
    @to_str (self) -> str = "color"
}

@main () -> int = {
    let c = Color { r: 100, g: 150, b: 200 };
    let ok1 = c.brightness() == 150;
    let ok2 = c.to_str() == "color";
    if ok1 && ok2 then 0 else 1
}
"#,
        "multiple_impl_blocks",
    );
}

// -----------------------------------------------------------------------
// 3.21: Operator Traits — user-defined operator dispatch
// -----------------------------------------------------------------------

#[test]
fn test_aot_operator_trait_add() {
    assert_aot_success(
        r#"
type Point = { x: int, y: int }

impl Add for Point {
    type Output = Point
    @add (self, rhs: Point) -> Point = Point {
        x: self.x + rhs.x,
        y: self.y + rhs.y,
    }
}

@main () -> int = {
    let a = Point { x: 1, y: 2 };
    let b = Point { x: 3, y: 4 };
    let c = a + b;
    if c.x == 4 && c.y == 6 then 0 else 1
}
"#,
        "operator_trait_add",
    );
}

#[test]
fn test_aot_operator_trait_sub() {
    assert_aot_success(
        r#"
type Point = { x: int, y: int }

impl Sub for Point {
    type Output = Point
    @subtract (self, rhs: Point) -> Point = Point {
        x: self.x - rhs.x,
        y: self.y - rhs.y,
    }
}

@main () -> int = {
    let a = Point { x: 5, y: 8 };
    let b = Point { x: 2, y: 3 };
    let c = a - b;
    if c.x == 3 && c.y == 5 then 0 else 1
}
"#,
        "operator_trait_sub",
    );
}

#[test]
fn test_aot_operator_trait_neg() {
    assert_aot_success(
        r#"
type Point = { x: int, y: int }

impl Neg for Point {
    type Output = Point
    @negate (self) -> Point = Point {
        x: -self.x,
        y: -self.y,
    }
}

@main () -> int = {
    let a = Point { x: 3, y: -7 };
    let b = -a;
    if b.x == -3 && b.y == 7 then 0 else 1
}
"#,
        "operator_trait_neg",
    );
}

#[test]
fn test_aot_operator_trait_mul_mixed() {
    assert_aot_success(
        r#"
type Vec2 = { x: int, y: int }

impl Mul<int> for Vec2 {
    type Output = Vec2
    @multiply (self, rhs: int) -> Vec2 = Vec2 {
        x: self.x * rhs,
        y: self.y * rhs,
    }
}

@main () -> int = {
    let v = Vec2 { x: 2, y: 3 };
    let scaled = v * 5;
    if scaled.x == 10 && scaled.y == 15 then 0 else 1
}
"#,
        "operator_trait_mul_mixed",
    );
}

#[test]
fn test_aot_operator_trait_chained() {
    assert_aot_success(
        r#"
type Point = { x: int, y: int }

impl Add for Point {
    type Output = Point
    @add (self, rhs: Point) -> Point = Point {
        x: self.x + rhs.x,
        y: self.y + rhs.y,
    }
}

impl Sub for Point {
    type Output = Point
    @subtract (self, rhs: Point) -> Point = Point {
        x: self.x - rhs.x,
        y: self.y - rhs.y,
    }
}

@main () -> int = {
    let a = Point { x: 1, y: 2 };
    let b = Point { x: 3, y: 4 };
    let c = Point { x: 10, y: 10 };
    let result = c - (a + b);
    if result.x == 6 && result.y == 4 then 0 else 1
}
"#,
        "operator_trait_chained",
    );
}

#[test]
fn test_aot_operator_trait_bitwise() {
    assert_aot_success(
        r#"
type Mask = { bits: int }

impl BitAnd for Mask {
    type Output = Mask
    @bit_and (self, rhs: Mask) -> Mask = Mask { bits: self.bits & rhs.bits }
}

impl BitOr for Mask {
    type Output = Mask
    @bit_or (self, rhs: Mask) -> Mask = Mask { bits: self.bits | rhs.bits }
}

@main () -> int = {
    let a = Mask { bits: 0b1100 };
    let b = Mask { bits: 0b1010 };
    let and_result = a & b;
    let or_result = a | b;
    if and_result.bits == 0b1000 && or_result.bits == 0b1110 then 0 else 1
}
"#,
        "operator_trait_bitwise",
    );
}

#[test]
fn test_aot_operator_trait_not() {
    assert_aot_success(
        r#"
type Toggle = { on: bool }

impl Not for Toggle {
    type Output = Toggle
    @not (self) -> Toggle = Toggle { on: !self.on }
}

@main () -> int = {
    let t = Toggle { on: true };
    let f = !t;
    if f.on == false then 0 else 1
}
"#,
        "operator_trait_not",
    );
}

// =========================================================================
// 3.14: Comparable/Hashable compound type methods
// =========================================================================

// -- String methods --

#[test]
fn test_aot_str_compare() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = "apple";
    let b = "banana";
    let c = "apple";
    let r1 = a.compare(b).is_less();
    let r2 = a.compare(c).is_equal();
    let r3 = b.compare(a).is_greater();
    if r1 && r2 && r3 then 0 else 1
}
"#,
        "str_compare",
    );
}

#[test]
fn test_aot_str_equals() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = "hello";
    let b = "hello";
    let c = "world";
    let r1 = a.equals(b);
    let r2 = !a.equals(c);
    if r1 && r2 then 0 else 1
}
"#,
        "str_equals",
    );
}

#[test]
fn test_aot_str_hash() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = "hello";
    let b = "hello";
    let c = "world";
    let h1 = a.hash();
    let h2 = b.hash();
    let h3 = c.hash();
    // Same strings produce same hash
    if h1 == h2 && h1 != h3 then 0 else 1
}
"#,
        "str_hash",
    );
}

// -- Bool hash --

#[test]
fn test_aot_bool_hash() {
    assert_aot_success(
        r#"
@main () -> int = {
    let t = true;
    let f = false;
    let ht = t.hash();
    let hf = f.hash();
    // true.hash() = 1, false.hash() = 0
    if ht == 1 && hf == 0 then 0 else 1
}
"#,
        "bool_hash",
    );
}

// -- Ordering compare --

#[test]
fn test_aot_ordering_compare() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = 1.compare(2);
    let b = 1.compare(2);
    let c = 3.compare(2);
    // Less.compare(Less) = Equal
    let r1 = a.compare(b).is_equal();
    // Less.compare(Greater) = Less (0 < 2)
    let r2 = a.compare(c).is_less();
    if r1 && r2 then 0 else 1
}
"#,
        "ordering_compare",
    );
}

// -- Float hash --

#[test]
fn test_aot_float_hash() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = 3.14;
    let b = 3.14;
    let c = 2.71;
    let h1 = a.hash();
    let h2 = b.hash();
    let h3 = c.hash();
    if h1 == h2 && h1 != h3 then 0 else 1
}
"#,
        "float_hash",
    );
}

// -- hash_combine --

#[test]
fn test_aot_hash_combine() {
    assert_aot_success(
        r#"
@main () -> int = {
    let h1 = hash_combine(0, 42);
    let h2 = hash_combine(0, 42);
    let h3 = hash_combine(0, 99);
    // Deterministic
    if h1 == h2 && h1 != h3 then 0 else 1
}
"#,
        "hash_combine",
    );
}

// -- Option compare --

#[test]
fn test_aot_option_compare() {
    assert_aot_success(
        r#"
@main () -> int = {
    let none: Option<int> = None;
    let some1 = Some(10);
    let some2 = Some(20);
    let some3 = Some(10);
    // None < Some
    let r1 = none.compare(some1).is_less();
    // Some(10) < Some(20)
    let r2 = some1.compare(some2).is_less();
    // Some(10) == Some(10)
    let r3 = some1.compare(some3).is_equal();
    // Some > None
    let r4 = some1.compare(none).is_greater();
    if r1 && r2 && r3 && r4 then 0 else 1
}
"#,
        "option_compare",
    );
}

// -- Option equals --

#[test]
fn test_aot_option_equals() {
    assert_aot_success(
        r#"
@main () -> int = {
    let none1: Option<int> = None;
    let none2: Option<int> = None;
    let some1 = Some(42);
    let some2 = Some(42);
    let some3 = Some(99);
    let r1 = none1.equals(none2);
    let r2 = some1.equals(some2);
    let r3 = !some1.equals(some3);
    let r4 = !none1.equals(some1);
    if r1 && r2 && r3 && r4 then 0 else 1
}
"#,
        "option_equals",
    );
}

// -- Option hash --

#[test]
fn test_aot_option_hash() {
    assert_aot_success(
        r#"
@main () -> int = {
    let none: Option<int> = None;
    let some1 = Some(42);
    let some2 = Some(42);
    let some3 = Some(99);
    let h_none = none.hash();
    let h1 = some1.hash();
    let h2 = some2.hash();
    let h3 = some3.hash();
    // None.hash() == 0
    let r1 = h_none == 0;
    // Same value → same hash
    let r2 = h1 == h2;
    // Different value → different hash (with overwhelming probability)
    let r3 = h1 != h3;
    if r1 && r2 && r3 then 0 else 1
}
"#,
        "option_hash",
    );
}

// -- Result compare --

#[test]
fn test_aot_result_compare() {
    assert_aot_success(
        r#"
@main () -> int = {
    let ok1: Result<int, int> = Ok(10);
    let ok2: Result<int, int> = Ok(20);
    let err1: Result<int, int> = Err(5);
    let err2: Result<int, int> = Err(15);
    // Ok < Err
    let r1 = ok1.compare(err1).is_less();
    // Ok(10) < Ok(20)
    let r2 = ok1.compare(ok2).is_less();
    // Err(5) < Err(15)
    let r3 = err1.compare(err2).is_less();
    // Err > Ok
    let r4 = err1.compare(ok1).is_greater();
    if r1 && r2 && r3 && r4 then 0 else 1
}
"#,
        "result_compare",
    );
}

// -- Result equals --

#[test]
fn test_aot_result_equals() {
    assert_aot_success(
        r#"
@main () -> int = {
    let ok1: Result<int, int> = Ok(42);
    let ok2: Result<int, int> = Ok(42);
    let ok3: Result<int, int> = Ok(99);
    let err1: Result<int, int> = Err(1);
    let err2: Result<int, int> = Err(1);
    let r1 = ok1.equals(ok2);
    let r2 = !ok1.equals(ok3);
    let r3 = err1.equals(err2);
    let r4 = !ok1.equals(err1);
    if r1 && r2 && r3 && r4 then 0 else 1
}
"#,
        "result_equals",
    );
}

// -- Result hash --

#[test]
fn test_aot_result_hash() {
    assert_aot_success(
        r#"
@main () -> int = {
    let ok1: Result<int, int> = Ok(42);
    let ok2: Result<int, int> = Ok(42);
    let err1: Result<int, int> = Err(42);
    let h1 = ok1.hash();
    let h2 = ok2.hash();
    let h3 = err1.hash();
    // Same variant+value → same hash
    let r1 = h1 == h2;
    // Ok(42) vs Err(42) → different hash (different seed)
    let r2 = h1 != h3;
    if r1 && r2 then 0 else 1
}
"#,
        "result_hash",
    );
}

// -- Tuple compare --

#[test]
fn test_aot_tuple_compare() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = (1, 2);
    let b = (1, 3);
    let c = (1, 2);
    let d = (2, 0);
    // (1,2) < (1,3) — lexicographic on second field
    let r1 = a.compare(b).is_less();
    // (1,2) == (1,2)
    let r2 = a.compare(c).is_equal();
    // (2,0) > (1,3) — first field decides
    let r3 = d.compare(b).is_greater();
    if r1 && r2 && r3 then 0 else 1
}
"#,
        "tuple_compare",
    );
}

// -- Tuple equals --

#[test]
fn test_aot_tuple_equals() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = (1, true);
    let b = (1, true);
    let c = (1, false);
    let r1 = a.equals(b);
    let r2 = !a.equals(c);
    if r1 && r2 then 0 else 1
}
"#,
        "tuple_equals",
    );
}

// -- Tuple hash --

#[test]
fn test_aot_tuple_hash() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = (1, 2, 3);
    let b = (1, 2, 3);
    let c = (3, 2, 1);
    let h1 = a.hash();
    let h2 = b.hash();
    let h3 = c.hash();
    // Same tuple → same hash
    let r1 = h1 == h2;
    // Different tuple → different hash
    let r2 = h1 != h3;
    if r1 && r2 then 0 else 1
}
"#,
        "tuple_hash",
    );
}

// -- Primitive equals methods --

#[test]
fn test_aot_int_equals() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = 42;
    let b = 42;
    let c = 99;
    let r1 = a.equals(b);
    let r2 = !a.equals(c);
    if r1 && r2 then 0 else 1
}
"#,
        "int_equals",
    );
}

#[test]
fn test_aot_byte_compare() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = byte(10);
    let b = byte(20);
    let c = byte(10);
    let r1 = a.compare(b).is_less();
    let r2 = a.compare(c).is_equal();
    if r1 && r2 then 0 else 1
}
"#,
        "byte_compare",
    );
}

#[test]
fn test_aot_char_hash() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = 'A';
    let b = 'A';
    let c = 'Z';
    let h1 = a.hash();
    let h2 = b.hash();
    let h3 = c.hash();
    if h1 == h2 && h1 != h3 then 0 else 1
}
"#,
        "char_hash",
    );
}

// =========================================================================
// 3.14: Hash contract edge cases (hygiene fixes)
// =========================================================================

// Float ±0.0 hash contract: -0.0 == 0.0 → hash must match

#[test]
fn test_aot_float_hash_neg_zero() {
    assert_aot_success(
        r#"
@main () -> int = {
    let pos = 0.0;
    let neg = -0.0;
    // -0.0 == 0.0 must be true
    let eq = pos == neg;
    // Their hashes must also match
    let h1 = pos.hash();
    let h2 = neg.hash();
    if eq && h1 == h2 then 0 else 1
}
"#,
        "float_hash_neg_zero",
    );
}

// Byte hash: values ≥ 128 must use unsigned extension

#[test]
fn test_aot_byte_hash_high_value() {
    assert_aot_success(
        r#"
@main () -> int = {
    let b = byte(200);
    let h = b.hash();
    // byte(200) should hash to 200 (unsigned), not -56 (signed)
    if h == 200 then 0 else 1
}
"#,
        "byte_hash_high_value",
    );
}

// String hash quality: different strings of same length must hash differently

#[test]
fn test_aot_str_hash_same_length_different_content() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = "abc";
    let b = "xyz";
    let h1 = a.hash();
    let h2 = b.hash();
    // Same-length but different content must produce different hashes
    if h1 != h2 then 0 else 1
}
"#,
        "str_hash_same_length_different",
    );
}

// Nested Option: Option<Option<int>> compare/equals/hash

#[test]
fn test_aot_nested_option_equals() {
    assert_aot_success(
        r#"
@wrap (x: Option<int>) -> Option<Option<int>> = Some(x)
@wrap_none () -> Option<Option<int>> = None

@main () -> int = {
    let a = wrap(x: Some(42));
    let b = wrap(x: Some(42));
    let c = wrap(x: Some(99));
    let d = wrap(x: None);
    let e = wrap_none();
    // Same value → equals
    let r1 = a.equals(b);
    // Different inner value → not equals
    let r2 = !a.equals(c);
    // Some(Some(42)) != Some(None)
    let r3 = !a.equals(d);
    // Some(None) != None
    let r4 = !d.equals(e);
    // None == None
    let r5 = e.equals(wrap_none());
    if r1 && r2 && r3 && r4 && r5 then 0 else 1
}
"#,
        "nested_option_equals",
    );
}

#[test]
fn test_aot_nested_option_compare() {
    assert_aot_success(
        r#"
@wrap (x: Option<int>) -> Option<Option<int>> = Some(x)
@wrap_none () -> Option<Option<int>> = None

@main () -> int = {
    let a = wrap(x: Some(10));
    let b = wrap(x: Some(20));
    let c = wrap_none();
    // Some(Some(10)) < Some(Some(20))
    let r1 = a.compare(b).is_less();
    // None < Some(anything)
    let r2 = c.compare(a).is_less();
    // Some(anything) > None
    let r3 = a.compare(c).is_greater();
    if r1 && r2 && r3 then 0 else 1
}
"#,
        "nested_option_compare",
    );
}

#[test]
fn test_aot_nested_option_hash() {
    assert_aot_success(
        r#"
@wrap (x: Option<int>) -> Option<Option<int>> = Some(x)

@main () -> int = {
    let a = wrap(x: Some(42));
    let b = wrap(x: Some(42));
    let c = wrap(x: Some(99));
    let h1 = a.hash();
    let h2 = b.hash();
    let h3 = c.hash();
    // Same value → same hash
    let r1 = h1 == h2;
    // Different value → different hash
    let r2 = h1 != h3;
    if r1 && r2 then 0 else 1
}
"#,
        "nested_option_hash",
    );
}

// Tuple inside Option: Option<(int, int)> compare/equals

#[test]
fn test_aot_option_tuple_equals() {
    assert_aot_success(
        r#"
@wrap (t: (int, int)) -> Option<(int, int)> = Some(t)

@main () -> int = {
    let a = wrap(t: (1, 2));
    let b = wrap(t: (1, 2));
    let c = wrap(t: (3, 4));
    let r1 = a.equals(b);
    let r2 = !a.equals(c);
    if r1 && r2 then 0 else 1
}
"#,
        "option_tuple_equals",
    );
}

// -- List compare --

#[test]
fn test_aot_list_compare() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = [1, 2, 3];
    let b = [1, 2, 4];
    let c = [1, 2, 3];
    let d = [1, 2];
    // [1,2,3] < [1,2,4] — third element decides
    let r1 = a.compare(b).is_less();
    // [1,2,3] == [1,2,3]
    let r2 = a.compare(c).is_equal();
    // [1,2,4] > [1,2,3] — third element decides
    let r3 = b.compare(a).is_greater();
    // [1,2] < [1,2,3] — shorter list is Less
    let r4 = d.compare(a).is_less();
    // [1,2,3] > [1,2] — longer list is Greater
    let r5 = a.compare(d).is_greater();
    if r1 && r2 && r3 && r4 && r5 then 0 else 1
}
"#,
        "list_compare",
    );
}

#[test]
fn test_aot_list_compare_empty() {
    assert_aot_success(
        r#"
@main () -> int = {
    let empty: [int] = [];
    let one = [1];
    // [] == []
    let r1 = empty.compare(empty).is_equal();
    // [] < [1]
    let r2 = empty.compare(one).is_less();
    // [1] > []
    let r3 = one.compare(empty).is_greater();
    if r1 && r2 && r3 then 0 else 1
}
"#,
        "list_compare_empty",
    );
}

// -- List equals --

#[test]
fn test_aot_list_equals() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = [1, 2, 3];
    let b = [1, 2, 3];
    let c = [1, 2, 4];
    let d = [1, 2];
    let r1 = a.equals(b);
    let r2 = !a.equals(c);
    let r3 = !a.equals(d);
    if r1 && r2 && r3 then 0 else 1
}
"#,
        "list_equals",
    );
}

#[test]
fn test_aot_list_equals_empty() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a: [int] = [];
    let b: [int] = [];
    let c = [1];
    let r1 = a.equals(b);
    let r2 = !a.equals(c);
    if r1 && r2 then 0 else 1
}
"#,
        "list_equals_empty",
    );
}

// -- List hash --

#[test]
fn test_aot_list_hash() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a = [1, 2, 3];
    let b = [1, 2, 3];
    let c = [3, 2, 1];
    let h1 = a.hash();
    let h2 = b.hash();
    let h3 = c.hash();
    // Same list → same hash
    let r1 = h1 == h2;
    // Different order → different hash
    let r2 = h1 != h3;
    if r1 && r2 then 0 else 1
}
"#,
        "list_hash",
    );
}

#[test]
fn test_aot_list_hash_empty() {
    assert_aot_success(
        r#"
@main () -> int = {
    let a: [int] = [];
    let b: [int] = [];
    let h1 = a.hash();
    let h2 = b.hash();
    // Empty lists have same hash
    let r1 = h1 == h2;
    // Empty list hash is 0 (initial seed)
    let r2 = h1 == 0;
    if r1 && r2 then 0 else 1
}
"#,
        "list_hash_empty",
    );
}

// 3.17: Into Trait — .into() codegen

#[test]
fn test_aot_int_into_float() {
    assert_aot_success(
        r#"
@main () -> int = {
    let n = 42;
    let f: float = n.into();
    if f == 42.0 then 0 else 1
}
"#,
        "int_into_float",
    );
}

#[test]
fn test_aot_int_into_float_negative() {
    assert_aot_success(
        r#"
@main () -> int = {
    let n = -100;
    let f: float = n.into();
    if f == -100.0 then 0 else 1
}
"#,
        "int_into_float_neg",
    );
}

#[test]
fn test_aot_int_into_float_zero() {
    assert_aot_success(
        r#"
@main () -> int = {
    let n = 0;
    let f: float = n.into();
    if f == 0.0 then 0 else 1
}
"#,
        "int_into_float_zero",
    );
}
