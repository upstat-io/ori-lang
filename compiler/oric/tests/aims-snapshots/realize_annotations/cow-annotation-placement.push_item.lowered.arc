fn @push_item(%0: [int] [own], %1: int [own]) -> [int] [entry: bb0]
  bb0:
    %2: [int] [RcPtr] = %0
    %3: int [Scalar] = %1
    %4: [int] [RcPtr] = Invoke @push(%2 [own], %3 [own]) normal bb1 unwind bb2
  bb1:
    Return %4
  bb2:
    Resume
