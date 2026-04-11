fn @make_greeter(%0: str [own]) -> () -> () [entry: bb0]
  bb0:
    %1: () -> () [FatVal] = PartialApply @__lambda_make_greeter_0(%0)
    Return %1
