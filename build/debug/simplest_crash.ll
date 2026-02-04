; ModuleID = 'simplest_crash'
source_filename = "simplest_crash"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-f80:128-n8:16:32:64-S128"
target triple = "x86_64-pc-linux-gnu"

declare void @ori_print(ptr)

declare void @ori_print_int(i64)

declare void @ori_print_float(double)

declare void @ori_print_bool(i1)

declare void @ori_panic(ptr)

declare void @ori_panic_cstr(ptr)

declare void @ori_assert(i1)

declare void @ori_assert_eq_int(i64, i64)

declare void @ori_assert_eq_bool(i1, i1)

declare void @ori_assert_eq_str(ptr, ptr)

declare ptr @ori_list_new(i64, i64)

declare void @ori_list_free(ptr, i64)

declare i64 @ori_list_len(ptr)

declare i32 @ori_compare_int(i64, i64)

declare i64 @ori_min_int(i64, i64)

declare i64 @ori_max_int(i64, i64)

declare { i64, ptr } @ori_str_concat(ptr, ptr)

declare i1 @ori_str_eq(ptr, ptr)

declare i1 @ori_str_ne(ptr, ptr)

declare { i64, ptr } @ori_str_from_int(i64)

declare { i64, ptr } @ori_str_from_bool(i1)

declare { i64, ptr } @ori_str_from_float(double)

declare ptr @ori_closure_box(i64)

define void @crash() {
entry:
  %default = alloca { i8, i64 }, align 8
  %outer = alloca { i8, i64 }, align 8
  store { i8, i64 } { i8 1, i64 0 }, ptr %outer, align 4
  store { i8, i64 } { i8 1, i64 99 }, ptr %default, align 4
  %outer1 = load { i8, i64 }, ptr %outer, align 4
  ret void
}
