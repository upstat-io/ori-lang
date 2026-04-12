fn @count_steps(%0: int [own]) -> int [entry: bb0]
  bb0:
    %1: int [Scalar] = 0
    %2: () [Scalar] = ()
    %3: int [Scalar] = 0
    %4: int [Scalar] = %0
    %5: int [Scalar] = 1
    %6: int [Scalar] = 0
    %7: range<int> [Scalar] = Construct Tuple(%3, %4, %5, %6)
    %13: int [Scalar] = Project %7.0
    %14: int [Scalar] = Project %7.1
    %15: int [Scalar] = Project %7.2
    Jump bb1(%13, %1)
  bb1: (%8: int, %9: int)
    %16: bool [Scalar] = %8 < %14
    Branch %16 ? bb2 : bb5
  bb2:
    %17: int [Scalar] = %9
    %18: int [Scalar] = 1
    %19: int [Scalar] = %17 + %18
    %20: int [Scalar] = %19
    %21: () [Scalar] = ()
    %22: () [Scalar] = ()
    Jump bb3(%20)
  bb3: (%10: int)
    %23: int [Scalar] = %8 + %15
    Jump bb1(%23, %10)
  bb4: (%11: (), %12: int)
    %25: int [Scalar] = %12
    Return %25
  bb5:
    %24: () [Scalar] = ()
    Jump bb4(%24, %9)
