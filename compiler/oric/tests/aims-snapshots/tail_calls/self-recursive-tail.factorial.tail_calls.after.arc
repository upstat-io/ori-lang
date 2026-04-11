fn @factorial(%0: int [own], %1: int [own]) -> int [entry: bb6]
  bb0: (%14: int, %15: int)
    %0: int [Scalar] = %14
    %1: int [Scalar] = %15
    %2: int [Scalar] = %0
    %3: int [Scalar] = 1
    %4: bool [Scalar] = %2 <= %3
    Branch %4 ? bb1 : bb2
  bb1:
    %5: int [Scalar] = %1
    Jump bb3(%5)
  bb2:
    %6: int [Scalar] = %0
    %7: int [Scalar] = 1
    %8: int [Scalar] = %6 - %7
    %9: int [Scalar] = %1
    %10: int [Scalar] = %0
    %11: int [Scalar] = %9 * %10
    Jump bb0(%8, %11)
  bb3: (%13: int)
    Return %13
  bb4:
    Jump bb3(%12)
  bb5:
    Resume
  bb6:
    Jump bb0(%0, %1)
