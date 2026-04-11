fn @is_even(%0: int [own]) -> bool [entry: bb0]
  bb0:
    %1: int [Scalar] = %0
    %2: int [Scalar] = 0
    %3: bool [Scalar] = %1 == %2
    Branch %3 ? bb1 : bb2
  bb1:
    %4: bool [Scalar] = true
    Jump bb3(%4)
  bb2:
    %5: int [Scalar] = %0
    %6: int [Scalar] = 1
    %7: int [Scalar] = %5 - %6
    %8: bool [Scalar] = Invoke @is_odd(%7 [borrow]) normal bb4 unwind bb5
  bb3: (%9: bool)
    Return %9
  bb4:
    Jump bb3(%8)
  bb5:
    Resume
