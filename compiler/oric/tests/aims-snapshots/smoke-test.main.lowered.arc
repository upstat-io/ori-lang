fn @main() -> () [entry: bb0]
  bb0:
    %0: str [FatVal] = "hello"
    %1: () [Scalar] = Apply @ori_print(%0 [own])
    %2: () [Scalar] = ()
    Return %2
