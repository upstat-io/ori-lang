fn @pick(%0: bool [own]) -> int [entry: bb0]
  bb0:
    %1: bool [Scalar] = %0
    %5: int [Scalar] = 1
    %6: int [Scalar] = 2
    %7: int [Scalar] = Select %1 ? %5 : %6
    %4: int [Scalar] = %7
    Return %4
