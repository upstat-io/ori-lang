fn @sum_to(%0: int [own]) -> int [entry: bb0]
  bb0:
    %1: int [Scalar] = %0
    %2: int [Scalar] = 0
    %3: bool [Scalar] = %1 <= %2
    Branch %3 ? bb1 : bb2
  bb1:
    %4: int [Scalar] = 0
    Jump bb3(%4)
  bb2:
    %5: int [Scalar] = %0
    %6: int [Scalar] = %0
    %7: int [Scalar] = 1
    %8: int [Scalar] = %6 - %7
    %9: int [Scalar] = Invoke @sum_to(%8 [own]) normal bb4 unwind bb5
  bb3: (%11: int)
    Return %11
  bb4:
    %10: int [Scalar] = %5 + %9
    Jump bb3(%10)
  bb5:
    Resume
