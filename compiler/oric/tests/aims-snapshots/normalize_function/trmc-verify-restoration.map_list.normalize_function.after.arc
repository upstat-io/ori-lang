fn @map_list(%0: [int] [borrow], %1: (int) -> int [borrow]) -> [int] [entry: bb0]
  bb0:
    %2: [int] [RcPtr] = %0
    %3: bool [Scalar] = Invoke @is_empty(%2 [own]) normal bb1 unwind bb2
  bb1:
    Branch %3 ? bb3 : bb4
  bb2:
    Resume
  bb3:
    %4: [int] [RcPtr] = Construct List()
    Jump bb5(%4)
  bb4:
    %5: [int] [RcPtr] = %0
    %6: int [Scalar] = Project %5.0
    %7: int [Scalar] = 0
    %8: int [Scalar] = Apply @__index(%5 [own], %7 [own])
    %9: () [Scalar] = ()
    %10: [int] [RcPtr] = %0
    %11: int [Scalar] = 1
    %12: [int] [RcPtr] = Invoke @skip(%10 [own], %11 [own]) normal bb6 unwind bb7
  bb5: (%23: [int])
    Return %23
  bb6:
    %13: <error> [Scalar] = Invoke @collect(%12 [own]) normal bb8 unwind bb9
  bb7:
    Resume
  bb8:
    %14: () [Scalar] = ()
    %15: int [Scalar] = %8
    %16: (int) -> int [FatVal] = %1
    %17: int [Scalar] = ApplyIndirect %16(%15)
    %18: [int] [RcPtr] = Construct List(%17)
    %19: <error> [Scalar] = %13
    %20: (int) -> int [FatVal] = %1
    %21: [int] [RcPtr] = Invoke @map_list(%19 [own], %20 [own]) normal bb10 unwind bb11
  bb9:
    Resume
  bb10:
    %22: [int] [RcPtr] = Invoke @concat(%18 [own], %21 [own]) normal bb12 unwind bb13
  bb11:
    Resume
  bb12:
    Jump bb5(%22)
  bb13:
    Resume
