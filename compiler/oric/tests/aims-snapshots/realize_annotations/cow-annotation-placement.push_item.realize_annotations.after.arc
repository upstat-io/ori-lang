fn @push_item(%0: [int] [borrow], %1: int [own]) -> [int] [entry: bb0]
  bb0:
    %2: [int] [RcPtr] = %0
    %3: int [Scalar] = %1
    %4: [int] [RcPtr] = Apply @push(%2 [own], %3 [own])
    Return %4
