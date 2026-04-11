fn @__lambda_make_greeter_0(%0: str [own]) -> () [entry: bb0]
  captures: 1
  bb0:
    %1: str [FatVal] = %0
    %2: () [Scalar] = Apply @ori_print(%1 [own])
    %3: () [Scalar] = ()
    Return %3
