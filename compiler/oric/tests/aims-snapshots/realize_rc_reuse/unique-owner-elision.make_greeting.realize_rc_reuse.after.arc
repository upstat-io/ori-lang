fn @make_greeting() -> str [entry: bb0]
  bb0:
    burden_inc %0
    %0: str [FatVal] = "hello world"
    burden_dec %0
    Return %0
