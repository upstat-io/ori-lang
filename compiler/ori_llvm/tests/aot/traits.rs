//! AOT Trait and Method Codegen Tests
//!
//! End-to-end tests verifying that trait methods, impl methods, and built-in
//! method dispatch produce correct native code through the LLVM backend.
//!
//! Covers roadmap Section 3 items:
//! - 3.0: Core library traits (Len, `IsEmpty`, Option, Result, Comparable, Eq)
//! - 3.1: Trait declarations (default methods)
//! - 3.2: Trait implementations (inherent impl, trait impl, method resolution)

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
@main () -> int = run(
    let xs = [10, 20, 30],
    let n = xs.len(),
    if n == 3 then 0 else 1
)
"#,
        "list_len_basic",
    );
}

#[test]
fn test_aot_list_len_empty() {
    assert_aot_success(
        r#"
@main () -> int = run(
    let xs: [int] = [],
    let n = xs.len(),
    if n == 0 then 0 else 1
)
"#,
        "list_len_empty",
    );
}

#[test]
fn test_aot_list_len_single() {
    assert_aot_success(
        r#"
@main () -> int = run(
    let xs = [42],
    if xs.len() == 1 then 0 else 1
)
"#,
        "list_len_single",
    );
}

#[test]
fn test_aot_string_len() {
    assert_aot_success(
        r#"
@main () -> int = run(
    let s = "hello",
    if s.len() == 5 then 0 else 1
)
"#,
        "string_len",
    );
}

#[test]
fn test_aot_string_len_empty() {
    assert_aot_success(
        r#"
@main () -> int = run(
    let s = "",
    if s.len() == 0 then 0 else 1
)
"#,
        "string_len_empty",
    );
}

// 3.0.2: IsEmpty Trait — .is_empty() codegen

#[test]
fn test_aot_list_is_empty_true() {
    assert_aot_success(
        r#"
@main () -> int = run(
    let xs: [int] = [],
    if xs.is_empty() then 0 else 1
)
"#,
        "list_is_empty_true",
    );
}

#[test]
fn test_aot_list_is_empty_false() {
    assert_aot_success(
        r#"
@main () -> int = run(
    let xs = [1, 2],
    if xs.is_empty() then 1 else 0
)
"#,
        "list_is_empty_false",
    );
}

#[test]
fn test_aot_string_is_empty_true() {
    assert_aot_success(
        r#"
@main () -> int = run(
    let s = "",
    if s.is_empty() then 0 else 1
)
"#,
        "string_is_empty_true",
    );
}

#[test]
fn test_aot_string_is_empty_false() {
    assert_aot_success(
        r#"
@main () -> int = run(
    let s = "hi",
    if s.is_empty() then 1 else 0
)
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

@main () -> int = run(
    let o = get(),
    if o.is_some() then 0 else 1
)
"#,
        "option_is_some_true",
    );
}

#[test]
fn test_aot_option_is_some_false() {
    assert_aot_success(
        r#"
@get () -> Option<int> = None

@main () -> int = run(
    let o = get(),
    if o.is_some() then 1 else 0
)
"#,
        "option_is_some_false",
    );
}

#[test]
fn test_aot_option_is_none_true() {
    assert_aot_success(
        r#"
@get () -> Option<int> = None

@main () -> int = run(
    let o = get(),
    if o.is_none() then 0 else 1
)
"#,
        "option_is_none_true",
    );
}

#[test]
fn test_aot_option_is_none_false() {
    assert_aot_success(
        r#"
@get () -> Option<int> = Some(7)

@main () -> int = run(
    let o = get(),
    if o.is_none() then 1 else 0
)
"#,
        "option_is_none_false",
    );
}

#[test]
fn test_aot_option_unwrap_some() {
    assert_aot_success(
        r#"
@get () -> Option<int> = Some(42)

@main () -> int = run(
    let o = get(),
    let v = o.unwrap(),
    if v == 42 then 0 else 1
)
"#,
        "option_unwrap_some",
    );
}

#[test]
#[ignore = "LLVM codegen: .unwrap_or() not in Option method dispatch table"]
fn test_aot_option_unwrap_or_some() {
    assert_aot_success(
        r#"
@get () -> Option<int> = Some(42)

@main () -> int = run(
    let o = get(),
    let v = o.unwrap_or(default: 0),
    if v == 42 then 0 else 1
)
"#,
        "option_unwrap_or_some",
    );
}

#[test]
#[ignore = "LLVM codegen: .unwrap_or() not in Option method dispatch table"]
fn test_aot_option_unwrap_or_none() {
    assert_aot_success(
        r#"
@get () -> Option<int> = None

@main () -> int = run(
    let o = get(),
    let v = o.unwrap_or(default: 99),
    if v == 99 then 0 else 1
)
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

@main () -> int = run(
    let r = get(),
    if r.is_ok() then 0 else 1
)
"#,
        "result_is_ok_true",
    );
}

#[test]
fn test_aot_result_is_ok_false() {
    assert_aot_success(
        r#"
@get () -> Result<int, str> = Err("bad")

@main () -> int = run(
    let r = get(),
    if r.is_ok() then 1 else 0
)
"#,
        "result_is_ok_false",
    );
}

#[test]
fn test_aot_result_is_err_true() {
    assert_aot_success(
        r#"
@get () -> Result<int, str> = Err("bad")

@main () -> int = run(
    let r = get(),
    if r.is_err() then 0 else 1
)
"#,
        "result_is_err_true",
    );
}

#[test]
fn test_aot_result_is_err_false() {
    assert_aot_success(
        r#"
@get () -> Result<int, str> = Ok(42)

@main () -> int = run(
    let r = get(),
    if r.is_err() then 1 else 0
)
"#,
        "result_is_err_false",
    );
}

#[test]
fn test_aot_result_unwrap_ok() {
    assert_aot_success(
        r#"
@get () -> Result<int, str> = Ok(42)

@main () -> int = run(
    let r = get(),
    let v = r.unwrap(),
    if v == 42 then 0 else 1
)
"#,
        "result_unwrap_ok",
    );
}

// 3.0.5: Comparable Trait — .compare() codegen

#[test]
#[ignore = "LLVM codegen: .compare() return type not resolved as Ordering (Idx::ERROR)"]
fn test_aot_int_compare_less() {
    assert_aot_success(
        r#"
@main () -> int = run(
    let ord = 1.compare(other: 5),
    if ord.is_less() then 0 else 1
)
"#,
        "int_compare_less",
    );
}

#[test]
#[ignore = "LLVM codegen: .compare() return type not resolved as Ordering (Idx::ERROR)"]
fn test_aot_int_compare_equal() {
    assert_aot_success(
        r#"
@main () -> int = run(
    let ord = 7.compare(other: 7),
    if ord.is_equal() then 0 else 1
)
"#,
        "int_compare_equal",
    );
}

#[test]
#[ignore = "LLVM codegen: .compare() return type not resolved as Ordering (Idx::ERROR)"]
fn test_aot_int_compare_greater() {
    assert_aot_success(
        r#"
@main () -> int = run(
    let ord = 10.compare(other: 3),
    if ord.is_greater() then 0 else 1
)
"#,
        "int_compare_greater",
    );
}

#[test]
#[ignore = "LLVM codegen: .compare() return type not resolved as Ordering (Idx::ERROR)"]
fn test_aot_ordering_reverse() {
    assert_aot_success(
        r#"
@main () -> int = run(
    let ord = 1.compare(other: 5),
    let rev = ord.reverse(),
    if rev.is_greater() then 0 else 1
)
"#,
        "ordering_reverse",
    );
}

#[test]
#[ignore = "LLVM codegen: .compare() return type not resolved as Ordering (Idx::ERROR)"]
fn test_aot_ordering_predicates() {
    assert_aot_success(
        r#"
@main () -> int = run(
    let less = 1.compare(other: 2),
    let equal = 3.compare(other: 3),
    let greater = 5.compare(other: 1),
    let ok1 = less.is_less() && !less.is_equal() && !less.is_greater(),
    let ok2 = !equal.is_less() && equal.is_equal() && !equal.is_greater(),
    let ok3 = !greater.is_less() && !greater.is_equal() && greater.is_greater(),
    if ok1 && ok2 && ok3 then 0 else 1
)
"#,
        "ordering_predicates",
    );
}

#[test]
#[ignore = "LLVM codegen: .compare() return type not resolved as Ordering (Idx::ERROR)"]
fn test_aot_ordering_is_less_or_equal() {
    assert_aot_success(
        r#"
@main () -> int = run(
    let less = 1.compare(other: 2),
    let equal = 3.compare(other: 3),
    let greater = 5.compare(other: 1),
    let ok1 = less.is_less_or_equal(),
    let ok2 = equal.is_less_or_equal(),
    let ok3 = !greater.is_less_or_equal(),
    if ok1 && ok2 && ok3 then 0 else 1
)
"#,
        "ordering_is_less_or_equal",
    );
}

#[test]
#[ignore = "LLVM codegen: .compare() return type not resolved as Ordering (Idx::ERROR)"]
fn test_aot_ordering_is_greater_or_equal() {
    assert_aot_success(
        r#"
@main () -> int = run(
    let less = 1.compare(other: 2),
    let equal = 3.compare(other: 3),
    let greater = 5.compare(other: 1),
    let ok1 = !less.is_greater_or_equal(),
    let ok2 = equal.is_greater_or_equal(),
    let ok3 = greater.is_greater_or_equal(),
    if ok1 && ok2 && ok3 then 0 else 1
)
"#,
        "ordering_is_greater_or_equal",
    );
}

// 3.0.6: Eq Trait — == and != codegen (explicit coverage)

#[test]
fn test_aot_eq_int() {
    assert_aot_success(
        r#"
@main () -> int = run(
    let a = 42,
    let b = 42,
    let c = 99,
    let eq = a == b,
    let ne = a != c,
    if eq && ne then 0 else 1
)
"#,
        "eq_int",
    );
}

#[test]
fn test_aot_eq_bool() {
    assert_aot_success(
        r#"
@main () -> int = run(
    let a = true,
    let b = true,
    let c = false,
    let eq = a == b,
    let ne = a != c,
    if eq && ne then 0 else 1
)
"#,
        "eq_bool",
    );
}

#[test]
fn test_aot_eq_string() {
    assert_aot_success(
        r#"
@main () -> int = run(
    let a = "hello",
    let b = "hello",
    let c = "world",
    let eq = a == b,
    let ne = a != c,
    if eq && ne then 0 else 1
)
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

@main () -> int = run(
    let p = Point { x: 10, y: 20 },
    let ok1 = p.get_x() == 10,
    let ok2 = p.get_y() == 20,
    let ok3 = p.sum() == 30,
    if ok1 && ok2 && ok3 then 0 else 1
)
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

@main () -> int = run(
    let c = Counter { value: 10 },
    let ok1 = c.add(n: 5) == 15,
    let ok2 = c.is_above(threshold: 5),
    let ok3 = !c.is_above(threshold: 15),
    if ok1 && ok2 && ok3 then 0 else 1
)
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

@main () -> int = run(
    let w = Widget { name: "button" },
    let d = w.describe(),
    if d == "button" then 0 else 1
)
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

@main () -> int = run(
    let n = Num { value: 5 },
    let ok1 = n.add(n: 3) == 8,
    let ok2 = n.double() == 10,
    if ok1 && ok2 then 0 else 1
)
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

@main () -> int = run(
    let item = Item { label: "widget" },
    let s = item.summary(),
    if s == "Item: widget" then 0 else 1
)
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

@main () -> int = run(
    let p = Person { name: "Alice" },
    let g = p.greet(),
    if g == "Hi, I'm Alice" then 0 else 1
)
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

@main () -> int = run(
    let r = Rect { width: 3, height: 4 },
    let ok1 = r.area() == 12,
    let ok2 = r.perimeter() == 14,
    let ok3 = !r.is_square(),
    let sq = Rect { width: 5, height: 5 },
    let ok4 = sq.is_square(),
    if ok1 && ok2 && ok3 && ok4 then 0 else 1
)
"#,
        "impl_method_field_access",
    );
}

// 3.2: Multiple impl blocks on same type

#[test]
#[ignore = "LLVM codegen: trait impl self passed as pointer, extract_value expects struct"]
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

@main () -> int = run(
    let c = Color { r: 100, g: 150, b: 200 },
    let ok1 = c.brightness() == 150,
    let ok2 = c.to_str() == "color",
    if ok1 && ok2 then 0 else 1
)
"#,
        "multiple_impl_blocks",
    );
}
