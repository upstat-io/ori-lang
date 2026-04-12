fn @sequential() -> int [entry: bb0]
  bb0:
    %0: int [Scalar] = 1
    %1: () [Scalar] = ()
    %2: int [Scalar] = 2
    %3: () [Scalar] = ()
    %4: int [Scalar] = %0
    %5: int [Scalar] = %2
    %6: int [Scalar] = %4 + %5
    %7: () [Scalar] = ()
    %8: int [Scalar] = %6
    Return %8
