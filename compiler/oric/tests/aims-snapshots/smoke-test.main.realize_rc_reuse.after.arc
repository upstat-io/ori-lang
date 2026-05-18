fn @main() -> () [entry: bb0]
  bb0:
    burden_inc %0
    %0: str [FatVal] = "hello"
    %1: () [Scalar] = Apply @ori_print(%0 [borrow])
    RcDec %0 [FatPtr]
    burden_dec %0
    %2: () [Scalar] = ()
    Return %2
