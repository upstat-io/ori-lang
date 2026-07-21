-- Ackermann (call-heavy macro kernel). Equal-work port of ackermann.ori / ackermann.py.
-- Run under LuaJIT with -joff (strict-interpreter north-star comparator).
local function ackermann(m, n)
  if m == 0 then return n + 1 end
  if n == 0 then return ackermann(m - 1, 1) end
  return ackermann(m - 1, ackermann(m, n - 1))
end
print(ackermann(3, 6))
