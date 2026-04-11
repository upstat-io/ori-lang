fn @replicate(%0: int [own], %1: str [borrow]) -> [str] [entry: bb0]
  bb0:
    %2: int [Scalar] = %0
    %3: int [Scalar] = 0
    %4: bool [Scalar] = %2 <= %3
    Branch %4 ? bb1 : bb2
  bb1:
    %5: [str] [RcPtr] = Construct List()
    Jump bb3(%5)
  bb2:
    %6: str [FatVal] = %1
    %7: [str] [RcPtr] = Construct List(%6)
    %8: int [Scalar] = %0
    %9: int [Scalar] = 1
    %10: int [Scalar] = %8 - %9
    %11: str [FatVal] = %1
    %12: [str] [RcPtr] = Invoke @replicate(%10 [own], %11 [own]) normal bb4 unwind bb5
  bb3: (%14: [str])
    Return %14
  bb4:
    %13: [str] [RcPtr] = Invoke @concat(%7 [own], %12 [own]) normal bb6 unwind bb7
  bb5:
    Resume
  bb6:
    Jump bb3(%13)
  bb7:
    Resume
