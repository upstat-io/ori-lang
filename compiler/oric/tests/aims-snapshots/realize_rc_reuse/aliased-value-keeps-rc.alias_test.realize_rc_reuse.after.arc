fn @alias_test() -> () [entry: bb0]
  bb0:
    %0: str [FatVal] = "hello"
    %1: () [Scalar] = ()
    RcInc %0 [FatPtr]
    %2: str [FatVal] = %0
    %3: () [Scalar] = Apply @ori_print(%2 [borrow])
    %4: () [Scalar] = ()
    %5: str [FatVal] = %0
    RcDec %2 [FatPtr]
    %6: () [Scalar] = Apply @ori_print(%5 [borrow])
    %7: () [Scalar] = ()
    %8: () [Scalar] = ()
    RcDec %5 [FatPtr]
    Return %8
