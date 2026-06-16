fn @main() -> () [entry: bb0]
  bb0:
    %0: str [FatVal] = "hello"
    RcInc %0 [FatPtr]
    %1: () [Scalar] = Apply @ori_print(%0 [borrow])
    %2: () [Scalar] = ()
    RcDec %0 [FatPtr]
    RcDec %0 [FatPtr]
    Return %2
