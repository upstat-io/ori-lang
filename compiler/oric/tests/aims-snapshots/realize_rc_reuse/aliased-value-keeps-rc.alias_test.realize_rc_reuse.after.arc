fn @alias_test() -> () [entry: bb0]
  bb0:
    burden_inc %0
    %0: str [FatVal] = "hello"
    %1: () [Scalar] = ()
    burden_inc %2
    RcInc %0 [FatPtr]
    %2: str [FatVal] = %0
    %3: () [Scalar] = Apply @ori_print(%2 [borrow])
    RcDec %2 [FatPtr]
    burden_dec %2
    %4: () [Scalar] = ()
    burden_inc %5
    %5: str [FatVal] = %0
    burden_dec %0
    %6: () [Scalar] = Apply @ori_print(%5 [borrow])
    RcDec %5 [FatPtr]
    burden_dec %5
    %7: () [Scalar] = ()
    %8: () [Scalar] = ()
    Return %8
